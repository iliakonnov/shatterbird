use futures::future::join_all;
use std::hash::Hash;

use futures::TryFutureExt;
use rayon::prelude::*;
use scc::{Bag, HashMap};
use tokio::io::AsyncReadExt;
use tracing::{debug, debug_span, info, info_span, instrument, Level, trace};

use lsp_types::lsif;
use shatterbird_storage::model::lang::{EdgeData, EdgeDataMultiIn, EdgeInfo, Item, VertexInfo};
use shatterbird_storage::model::{Edge, FileContent, Line, Node, Range, Vertex};
use shatterbird_storage::{Id, Model, Storage};

use super::graph::{DocumentRef, EdgeRef, Graph, VertexRef};
use super::lsif_ext::{EdgeDataRef, EdgeExtensions};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct LineKey {
    file: lsif::Id,
    line_no: u64,
}

pub struct Converter<'a> {
    graph: &'a Graph<'a>,
    files: HashMap<lsif::Id, Node>,
    lines: HashMap<LineKey, Line>,
    ranges: HashMap<lsif::Id, Range>,
    vertices: HashMap<lsif::Id, Vertex>,
    edges: Bag<Edge>,
}

impl<'a> Converter<'a> {
    pub fn new(graph: &'a Graph) -> Self {
        Converter {
            graph,
            files: HashMap::new(),
            lines: HashMap::new(),
            ranges: HashMap::new(),
            vertices: HashMap::new(),
            edges: Bag::new(),
        }
    }

    #[instrument(skip_all, err)]
    pub async fn load(&self) -> eyre::Result<()> {
        // Mostly IO-bound part
        let tasks = self.graph.documents().into_par_iter().map(|doc| {
            let doc_id = doc.entry().id.clone();
            self.load_doc(doc).map_ok(move |v| (doc_id, v))
        }).collect_vec_list();
        let docs = join_all(tasks.into_iter().flatten())
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        // CPU-bound part
        docs.into_par_iter()
            .map(|(id, v)| self.load_children(&id, v))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(())
    }

    #[instrument(skip_all, err)]
    pub async fn save(self, storage: &Storage) -> eyre::Result<()> {
        tokio::try_join!(
            async {
                let _span = info_span!("saving files").entered();
                let mut files = Vec::new();
                let mut next = self.files.first_entry_async().await;
                while let Some(curr) = next {
                    files.push(curr.get().clone());
                    next = curr.next_async().await
                }
                info!("saving {} files", files.len());
                storage.insert_many(files.iter()).await
            },
            async {
                let _span = info_span!("saving lines").entered();
                let mut lines = Vec::new();
                let mut next = self.lines.first_entry_async().await;
                while let Some(curr) = next {
                    lines.push(curr.get().clone());
                    next = curr.next_async().await
                }
                info!("saving {} lines", lines.len());
                storage.insert_many(lines.iter()).await
            },
            async {
                let _span = info_span!("saving vertices").entered();
                let mut vertices = Vec::new();
                let mut next = self.vertices.first_entry_async().await;
                while let Some(curr) = next {
                    vertices.push(curr.get().clone());
                    next = curr.next_async().await
                }
                info!("saving {} vertices", vertices.len());
                storage.insert_many(vertices.iter()).await
            },
            async {
                let _span = info_span!("saving edges").entered();
                info!("saving {} edges", self.edges.len());
                storage
                    .access()
                    .insert_many(self.edges.into_iter(), None)
                    .await?;
                Ok(())
            }
        )?;
        Ok(())
    }

    #[instrument(level = Level::DEBUG, skip_all, ret, err, fields(doc_id = ?doc.entry().id, uri = doc.document().uri.to_string()))]
    async fn load_doc(&self, doc: DocumentRef<'_>) -> eyre::Result<Id<Vertex>> {
        debug!("loading doc {:?}", doc.entry());
        let doc_id = doc.entry().id.clone();
        let doc = doc.document();
        eyre::ensure!(doc.uri.scheme().to_ascii_lowercase() == "file");

        let mut file = tokio::fs::File::open(doc.uri.path()).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;

        let lines = {
            let mut lines = Vec::new();
            for (line_no, line) in content.split('\n').enumerate() {
                lines.push(self.load_line(&doc_id, line, line_no as u64));
            }
            join_all(lines)
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
        };
        _ = self
            .files
            .insert_async(
                doc_id.clone(),
                Node {
                    _id: Id::new(),
                    oid: gix::ObjectId::empty_blob(gix::hash::Kind::Sha1),  // TODO: Reuse objects from Git
                    created_at: Default::default(),
                    content: FileContent::Text {
                        size: content.bytes().len() as u64,
                        lines,
                    },
                },
            )
            .await;

        let vertex_id = Id::new();
        _ = self
            .vertices
            .insert_async(
                doc_id.clone(),
                Vertex {
                    _id: vertex_id,
                    data: VertexInfo::Document(doc.clone()),
                },
            )
            .await;

        self.graph.outgoing_from(&doc_id)
            .into_par_iter()
            .filter_map(|edge| if let lsif::Edge::Contains(data) = edge.edge() { Some (data)} else { None })
            .flat_map(|data| &data.in_vs)
            .filter_map(|vertex| self.graph.vertex(vertex))
            .filter_map(|vertex| if let lsif::Vertex::Range { range, ..} = vertex.vertex() { Some((vertex, range)) } else { None })
            .map(|(vertex, range)| self.load_range(&doc_id, vertex, range))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(vertex_id)
    }

    #[instrument(skip(self), err)]
    fn load_children(&self, doc_id: &lsif::Id, vertex_id: Id<Vertex>) -> eyre::Result<()> {
        debug!("loading children of {:?} (aka {})", doc_id, vertex_id);
        self.graph
            .outgoing_from(doc_id)
            .into_par_iter()
            .map(|e| self.load_edge(vertex_id, e))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(())
    }

    #[instrument(level = Level::DEBUG, skip_all, ret, err, fields(out_v = %out_v, edge_id = ?edge.entry().id))]
    fn load_edge(&self, out_v: Id<Vertex>, edge: EdgeRef<'_>) -> eyre::Result<Option<Id<Edge>>> {
        trace!("loading edge {:?}", edge.entry().id);
        let in_vs = edge.edge()
            .edge_data()
            .par_each()
            .filter(|data| !self.vertices.contains(data.in_v))
            .collect_vec_list();
        if in_vs.par_iter().map(|x| x.len()).sum::<usize>() == 0 {
            return Ok(None);
        }
        let in_vs = in_vs
            .into_par_iter()
            .flatten()
            .map(|data| { self.visit_edge(data) })
            .collect::<Result<Vec<_>, _>>()?
            .par_iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        if in_vs.is_empty() {
            return Err(eyre::eyre!(
                "no incoming vertices found for edge {:?}",
                edge.entry()
            ));
        }

        let edge_data = EdgeData {
            in_v: in_vs.first().copied().unwrap_or_default(),
            out_v,
        };
        let edge_data_multi = EdgeDataMultiIn { in_vs, out_v };

        let id = Id::new();
        self.edges.push(Edge {
            _id: id,
            data: match edge.edge() {
                lsif::Edge::Contains(_x) => EdgeInfo::Contains(edge_data_multi),
                lsif::Edge::Moniker(_x) => EdgeInfo::Moniker(edge_data),
                lsif::Edge::NextMoniker(_x) => EdgeInfo::NextMoniker(edge_data),
                lsif::Edge::Next(_x) => EdgeInfo::Next(edge_data),
                lsif::Edge::PackageInformation(_x) => EdgeInfo::PackageInformation(edge_data),
                lsif::Edge::Item(x) => EdgeInfo::Item(Item {
                    document: match self.vertices.get(&x.document) {
                        Some(x) => x.get().id(),
                        None => {
                            return Err(eyre::eyre!(
                                "{:?} references document {:?} which is not yet loaded",
                                edge.entry(),
                                x.document
                            ))
                        }
                    },
                    property: x.property.clone(),
                    edge_data: edge_data_multi,
                }),
                lsif::Edge::Definition(_x) => EdgeInfo::Definition(edge_data),
                lsif::Edge::Declaration(_x) => EdgeInfo::Declaration(edge_data),
                lsif::Edge::Hover(_x) => EdgeInfo::Hover(edge_data),
                lsif::Edge::References(_x) => EdgeInfo::References(edge_data),
                lsif::Edge::Implementation(_x) => EdgeInfo::Implementation(edge_data),
                lsif::Edge::TypeDefinition(_x) => EdgeInfo::TypeDefinition(edge_data),
                lsif::Edge::FoldingRange(_x) => EdgeInfo::FoldingRange(edge_data),
                lsif::Edge::DocumentLink(_x) => EdgeInfo::DocumentLink(edge_data),
                lsif::Edge::DocumentSymbol(_x) => EdgeInfo::DocumentSymbol(edge_data),
                lsif::Edge::Diagnostic(_x) => EdgeInfo::Diagnostic(edge_data),
            },
        });

        Ok(Some(id))
    }

    #[instrument(level = Level::DEBUG, skip_all, ret, err, fields(in_v = ?data.in_v, out_v = ?data.out_v))]
    fn visit_edge(&self, data: EdgeDataRef) -> eyre::Result<Option<Id<Vertex>>> {
        let _span = debug_span!("edge item", in_v = ?data.in_v, out_v = ?data.out_v).entered();

        let vertex = match self.load_vertex(data.in_v)? {
            Some(x) => x,
            None => return Ok(None),
        };

        self.graph
            .outgoing_from(data.in_v)
            .into_par_iter()
            .map(|edge| self.load_edge(vertex, edge))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(vertex))
    }

    #[instrument(level = Level::DEBUG, skip_all, ret, err, fields(vertex_id = ?v))]
    fn load_vertex(&self, v: &lsif::Id) -> eyre::Result<Option<Id<Vertex>>> {
        let vertex = match self.graph.vertex(v) {
            Some(x) => x,
            None => return Err(eyre::eyre!("vertex {:?} is not found but referenced", v)),
        };
        debug!("loading vertex {:?}", vertex.entry());

        let data = match vertex.vertex().clone() {
            lsif::Vertex::MetaData(x) => VertexInfo::MetaData(x),
            lsif::Vertex::Project(x) => VertexInfo::Project(x),
            lsif::Vertex::Document(x) => VertexInfo::Document(x),
            lsif::Vertex::Range { range, tag } => {
                let range = match self.ranges.get(&vertex.entry().id) {
                    Some(x) => x,
                    None => return Err(eyre::eyre!("range {:?} is not loaded", vertex.entry())),
                };
                VertexInfo::Range {
                    range: range.get().id(),
                    tag,
                }
            }
            lsif::Vertex::ResultSet(x) => VertexInfo::ResultSet(x),
            lsif::Vertex::Moniker(x) => VertexInfo::Moniker(x),
            lsif::Vertex::PackageInformation(x) => VertexInfo::PackageInformation(x),
            lsif::Vertex::Event(_) => return Ok(None),
            lsif::Vertex::DefinitionResult => VertexInfo::DefinitionResult {},
            lsif::Vertex::DeclarationResult => VertexInfo::DeclarationResult {},
            lsif::Vertex::TypeDefinitionResult => VertexInfo::TypeDefinitionResult {},
            lsif::Vertex::ReferenceResult => VertexInfo::ReferenceResult {},
            lsif::Vertex::ImplementationResult => VertexInfo::ImplementationResult {},
            lsif::Vertex::FoldingRangeResult { result } => {
                VertexInfo::FoldingRangeResult { result }
            }
            lsif::Vertex::HoverResult { result } => VertexInfo::HoverResult { result },
            lsif::Vertex::DocumentSymbolResult { result } => {
                VertexInfo::DocumentSymbolResult { result }
            }
            lsif::Vertex::DocumentLinkResult { result } => VertexInfo::DocumentLinkResult {
                result: result.into_iter().map(|_x| todo!()).collect(),
            },
            lsif::Vertex::DiagnosticResult { result } => VertexInfo::DiagnosticResult {
                result: result.into_iter().map(|_x| todo!()).collect(),
            },
        };
        let id = Id::new();
        _ = self.vertices.insert(v.clone(), Vertex { _id: id, data });
        Ok(Some(id))
    }

    #[instrument(level = Level::TRACE, ret, err, skip(self, line))]
    async fn load_line(
        &self,
        doc_id: &lsif::Id,
        line: &str,
        line_no: u64,
    ) -> eyre::Result<Id<Line>> {
        trace!("loading line #{} from doc #{:?}", line_no, doc_id);

        let id = Id::new();
        let line = Line {
            _id: id,
            text: line.to_string(),
        };
        let key = LineKey {
            file: doc_id.clone(),
            line_no,
        };
        _ = self.lines.insert_async(key, line).await;
        Ok(id)
    }

    #[instrument(level = Level::TRACE, skip_all, ret, err, fields(doc_id = ?doc_id, vertex_id = ?vertex.entry().id, range = ?range))]
    fn load_range(
        &self,
        doc_id: &lsif::Id,
        vertex: VertexRef<'_>,
        range: &lsp_types::Range,
    ) -> eyre::Result<Id<Range>> {
        trace!("loading range {:?}", range);

        let line_id = match self.files.get(doc_id) {
            Some(x) => match &x.get().content {
                FileContent::Text { lines, .. } => match lines.get(range.start.line as usize) {
                    Some(x) => *x,
                    None => {
                        return Err(eyre::eyre!(
                            "document {:?} is not long enough to get line #{}",
                            doc_id,
                            range.start.line
                        ))
                    }
                },
                _ => return Err(eyre::eyre!("document {:?} is not a text document", doc_id)),
            },
            None => {
                return Err(eyre::eyre!(
                    "document {:?} not found but referenced by {:?}",
                    doc_id,
                    range
                ))
            }
        };

        let end = if range.end.line == range.start.line {
            range.end.character
        } else {
            u32::MAX
        };

        let id = Id::new();
        let range = Range {
            _id: id,
            line_id,
            start: range.start.character,
            end,
        };
        _ = self
            .ranges
            .insert(vertex.entry().id.clone(), range);
        Ok(id)
    }
}
