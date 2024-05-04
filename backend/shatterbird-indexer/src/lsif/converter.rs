use either::Either;
use eyre::{eyre, OptionExt};
use futures::future::join_all;
use std::hash::Hash;
use std::ops::Deref;

use futures::FutureExt;
use rayon::prelude::*;
use scc::{Bag, HashMap, HashSet};
use tracing::{debug, debug_span, info, info_span, instrument, trace, warn, Level};

use crate::lsif::RootMapping;
use lsp_types::lsif;
use radix_trie::{Trie, TrieCommon};
use scc::hash_map::Entry;
use shatterbird_storage::model::lang::{EdgeData, EdgeDataMultiIn, EdgeInfo, Item, VertexInfo};
use shatterbird_storage::model::{Commit, Edge, FileContent, Line, Node, Range, Vertex};
use shatterbird_storage::{Id, Model, Storage};

use super::graph::{DocumentRef, EdgeRef, Graph, VertexRef};
use super::lsif_ext::{EdgeDataRef, EdgeExtensions};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct LineKey {
    file: lsif::Id,
    line_no: u64,
}

#[derive(Debug)]
struct FileWithPath {
    node: Node,
    path: Vec<Id<Node>>,
}

pub struct Converter<'g, 's> {
    storage: &'s Storage,
    graph: &'g Graph<'g>,
    roots: Trie<String, Id<Commit>>,
    files: HashMap<lsif::Id, FileWithPath>,
    lines: HashMap<LineKey, Line>,
    ranges: HashMap<lsif::Id, Range>,
    vertices: HashMap<lsif::Id, Option<Vertex>>,
    edges: HashMap<lsif::Id, Either<Id<Edge>, Edge>>,
}

impl<'g, 's> Converter<'g, 's> {
    pub fn new(storage: &'s Storage, graph: &'g Graph, roots: Vec<RootMapping>) -> Self {
        Converter {
            storage,
            graph,
            roots: roots.into_iter().map(|x| (x.dir, x.node)).collect(),
            files: HashMap::new(),
            lines: HashMap::new(),
            ranges: HashMap::new(),
            vertices: HashMap::new(),
            edges: HashMap::new(),
        }
    }

    #[instrument(skip_all, err)]
    pub async fn load(&self) -> eyre::Result<()> {
        // Mostly IO-bound part
        let tasks = self
            .graph
            .documents()
            .into_par_iter()
            .map(|doc| {
                let doc_id = doc.entry().id.clone();
                self.load_doc(doc).map(|x| (doc_id, x))
            })
            .collect_vec_list();
        let docs = join_all(tasks.into_iter().flatten())
            .await
            .into_iter()
            .filter_map(|(doc_id, x)| match x {
                Ok(None) => None,
                Ok(Some(x)) => Some(Ok((doc_id, x))),
                Err(e) => Some(Err(e)),
            })
            .collect::<Result<Vec<_>, _>>()?;

        // CPU-bound part
        docs.into_par_iter()
            .map(|(id, v)| self.load_children(&id, v))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(())
    }

    #[instrument(skip_all, err)]
    pub async fn save(self) -> eyre::Result<()> {
        tokio::try_join!(
            async {
                let _span = info_span!("saving ranges").entered();
                let mut ranges = Vec::new();
                let mut next = self.ranges.first_entry_async().await;
                while let Some(curr) = next {
                    ranges.push(curr.get().clone());
                    next = curr.next_async().await
                }
                info!("saving {} ranges", ranges.len());
                self.storage.insert_many(ranges.iter()).await
            },
            async {
                let _span = info_span!("saving vertices").entered();
                let mut vertices = Vec::new();
                let mut next = self.vertices.first_entry_async().await;
                while let Some(curr) = next {
                    if let Some(vertex) = curr.get().as_ref() {
                        vertices.push(vertex.clone());
                    }
                    next = curr.next_async().await
                }
                info!("saving {} vertices", vertices.len());
                self.storage.insert_many(vertices.iter()).await
            },
            async {
                let _span = info_span!("saving edges").entered();
                let mut edges = Vec::new();
                let mut next = self.edges.first_entry_async().await;
                while let Some(curr) = next {
                    if let Either::Right(edge) = curr.get().as_ref() {
                        edges.push(edge.clone());
                    }
                    next = curr.next_async().await
                }
                info!("saving {} edges", edges.len());
                self.storage.access().insert_many(edges, None).await?;
                Ok(())
            }
        )?;
        Ok(())
    }

    #[instrument(level = Level::DEBUG, skip_all, ret, err, fields(doc_id = ?doc.entry().id, uri = doc.document().uri.to_string()))]
    async fn load_doc(&self, doc: DocumentRef<'_>) -> eyre::Result<Option<Id<Vertex>>> {
        debug!("loading doc {:?}", doc.entry());
        let doc_id = doc.entry().id.clone();
        let doc = doc.document();
        eyre::ensure!(doc.uri.scheme().to_ascii_lowercase() == "file");

        let path = doc.uri.path();
        let trie_node = match self.roots.get_ancestor(path) {
            Some(x) => x,
            None => {
                warn!("no root found for a document {}", path);
                return Ok(None);
            }
        };
        let prefix = trie_node
            .key()
            .expect("trie key is present when trie node is found");
        let root = trie_node
            .value()
            .copied()
            .expect("trie value is present when trie node is found");
        let suffix = path
            .strip_prefix(prefix)
            .expect("path starts with prefix since prefix is ancestor of path");

        trace!(
            "searching for {} in {} ({}) with suffix {}",
            path,
            prefix,
            root,
            suffix
        );

        let mut path = Vec::new();
        let mut curr = self
            .storage
            .get(root)
            .await?
            .ok_or_eyre(eyre!("commit {} not found in DB", root))?
            .root;
        for segment in suffix.split('/') {
            if segment.is_empty() {
                continue;
            }
            let node = self
                .storage
                .get(curr)
                .await?
                .ok_or_eyre(eyre!("node {} not found in DB", root))?;
            path.push(node.id);
            curr = match node.content {
                FileContent::Directory { children, .. } => children
                    .get(segment)
                    .copied()
                    .ok_or_eyre(eyre!("can't find segment {}", segment))?,
                _ => return Err(eyre::eyre!("node {:?} is not a directory", curr.id)),
            };
        }

        let node = self
            .storage
            .get(curr)
            .await?
            .ok_or_eyre(eyre!("file {} not found in DB", root))?;
        path.push(node.id);

        let file = match node {
            Node {
                content: FileContent::Text { .. },
                ..
            } => node,
            _ => return Err(eyre::eyre!("file {:?} is not a text document", curr.id)),
        };
        let file = FileWithPath { node: file, path };

        self.files
            .insert_async(doc_id.clone(), file)
            .await
            .expect("doc id is unique");

        let vertex_id = Id::new();
        self.vertices
            .insert_async(
                doc_id.clone(),
                Some(Vertex {
                    id: vertex_id,
                    data: VertexInfo::Document(doc.clone()),
                }),
            )
            .await
            .expect("doc_id is unique");

        self.graph
            .outgoing_from(&doc_id)
            .into_par_iter()
            .filter_map(|edge| {
                if let lsif::Edge::Contains(data) = edge.edge() {
                    Some(data)
                } else {
                    None
                }
            })
            .flat_map(|data| &data.in_vs)
            .filter_map(|vertex| self.graph.vertex(vertex))
            .filter_map(|vertex| {
                if let lsif::Vertex::Range { range, .. } = vertex.vertex() {
                    Some((vertex, range))
                } else {
                    None
                }
            })
            .map(|(vertex, range)| self.load_range(&doc_id, vertex, range))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(vertex_id))
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
        let id = {
            let entry = self.edges.entry(edge.entry().id.clone());
            let entry = match entry {
                Entry::Occupied(existing) => {
                    return Ok(Some(existing.get().as_ref().either(|x| *x, |x| x.id)))
                }
                Entry::Vacant(vacant) => vacant,
            };
            let id = Id::new();
            entry.insert_entry(Either::Left(id));
            id
        };

        trace!("loading edge {:?}", edge.entry().id);
        let in_vs = edge
            .edge()
            .edge_data()
            .par_each()
            .map(|data| self.visit_edge(data))
            .collect::<Result<Vec<_>, _>>()?
            .par_iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        if in_vs.is_empty() {
            warn!("no incoming vertices found for edge {:?}", edge.entry().id);
            return Ok(None);
        }

        let edge_data = EdgeData {
            in_v: in_vs.first().copied().unwrap_or_default(),
            out_v,
        };
        let edge_data_multi = EdgeDataMultiIn { in_vs, out_v };

        self.edges
            .entry(edge.entry().id.clone())
            .insert_entry(Either::Right(Edge {
                id,
                data: match edge.edge() {
                    lsif::Edge::Contains(_x) => EdgeInfo::Contains(edge_data_multi),
                    lsif::Edge::Moniker(_x) => EdgeInfo::Moniker(edge_data),
                    lsif::Edge::NextMoniker(_x) => EdgeInfo::NextMoniker(edge_data),
                    lsif::Edge::Next(_x) => EdgeInfo::Next(edge_data),
                    lsif::Edge::PackageInformation(_x) => EdgeInfo::PackageInformation(edge_data),
                    lsif::Edge::Item(x) => EdgeInfo::Item(Item {
                        document: match self
                            .vertices
                            .get(&x.document)
                            .and_then(|x| x.get().as_ref().map(|x| x.id))
                        {
                            Some(x) => x,
                            None => {
                                return Err(eyre::eyre!(
                                    "{:?} references document {:?} which is not loaded",
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
            }));

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
        let entry = self.vertices.entry(v.clone());
        let mut entry = match entry {
            Entry::Occupied(existing) => return Ok(existing.get().as_ref().map(|x| x.id)),
            Entry::Vacant(vacant) => vacant.insert_entry(None),
        };
        let vertex = match self.graph.vertex(v) {
            Some(x) => x,
            None => return Err(eyre::eyre!("vertex {:?} is not found but referenced", v)),
        };
        debug!("loading vertex {:?}", vertex.entry());

        let data = match vertex.vertex().clone() {
            lsif::Vertex::MetaData(x) => VertexInfo::MetaData(x),
            lsif::Vertex::Project(x) => VertexInfo::Project(x),
            lsif::Vertex::Document(x) => VertexInfo::Document(x),
            lsif::Vertex::Range { tag, .. } => {
                let range = match self.ranges.get(&vertex.entry().id) {
                    Some(x) => x,
                    None => {
                        warn!(
                            "range {:?} is not loaded, probably some documents are missing?",
                            vertex.entry()
                        );
                        return Ok(None);
                    }
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
        entry.insert(Some(Vertex { id, data }));
        Ok(Some(id))
    }

    #[instrument(level = Level::TRACE, skip_all, ret, err, fields(doc_id = ?doc_id, vertex_id = ?vertex.entry().id, range = ?range))]
    fn load_range(
        &self,
        doc_id: &lsif::Id,
        vertex: VertexRef<'_>,
        range: &lsp_types::Range,
    ) -> eyre::Result<Id<Range>> {
        let entry = self.ranges.entry(vertex.entry().id.clone());
        let entry = match entry {
            Entry::Occupied(existing) => return Ok(existing.get().id()),
            Entry::Vacant(vacant) => vacant,
        };
        trace!("loading range {:?}", range);

        let (line_id, path) = match self.files.get(doc_id) {
            Some(x) => {
                let FileWithPath { node, path } = x.get();
                match &node.content {
                    FileContent::Text { lines, .. } => match lines.get(range.start.line as usize) {
                        Some(x) => (*x, path.clone()),
                        None => {
                            return Err(eyre::eyre!(
                                "document {:?} is not long enough to get line #{}",
                                doc_id,
                                range.start.line
                            ))
                        }
                    },
                    _ => return Err(eyre::eyre!("document {:?} is not a text document", doc_id)),
                }
            }
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
            id,
            line_id,
            start: range.start.character,
            end,
            path,
        };
        entry.insert_entry(range);
        Ok(id)
    }
}
