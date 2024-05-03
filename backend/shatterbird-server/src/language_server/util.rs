use eyre::{eyre, OptionExt};
use lsp_types::Url;
use mongodb::bson::doc;
use tracing::{debug, info_span, instrument};

use shatterbird_storage::model::lang::EdgeInfo;
use shatterbird_storage::model::{Commit, Edge, FileContent, Line, Node, Range, Vertex};
use shatterbird_storage::{Id, Storage};

use crate::language_server::error::LspError;

#[instrument(skip_all, fields(uri = %uri))]
pub async fn resolve_url(storage: &Storage, uri: &Url) -> Result<Node, LspError> {
    let splitted = uri
        .path()
        .split('/')
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();
    if splitted.is_empty() {
        return Err(LspError::not_found(uri, "empty path"));
    }
    let commit = splitted[0]
        .parse()
        .map_err(|x| LspError::bad_request(eyre!("invalid commit: {x}")))?;
    let commit: Commit = storage
        .get_by_oid(commit)
        .await?
        .ok_or_else(|| LspError::not_found(uri, format!("no such commit: {}", commit)))?;
    let mut curr = commit.root;
    for &component in splitted[1..].iter() {
        let node = match storage.get(curr).await? {
            Some(x) => x,
            None => return Err(LspError::Internal(eyre!("can't find {}", curr))),
        };
        let children = match node.content {
            FileContent::Directory { children } => children,
            _ => {
                return Err(LspError::not_found(
                    uri,
                    format!("{} is not a directory", curr),
                ))?
            }
        };
        curr = match children.get(component) {
            Some(x) => *x,
            None => {
                return Err(LspError::not_found(
                    uri,
                    format!("no child named {} in {}", component, curr),
                ))
            }
        };
    }
    storage
        .get(curr)
        .await?
        .ok_or_else(|| LspError::Internal(eyre!("can't find {}", curr)))
}

#[derive(Debug)]
pub struct ResolvedPosition {
    pub node: Id<Node>,
    pub line: Id<Line>,
    pub position: u32,
    pub related: Vec<Id<Vertex>>,
    pub found: Vec<Vertex>,
}

#[instrument(skip_all, fields(uri = %position.text_document.uri, edge=edge, position = ?position.position))]
pub async fn find(
    storage: &Storage,
    edge: &'static str,
    position: &lsp_types::TextDocumentPositionParams,
) -> Result<ResolvedPosition, LspError> {
    let node = resolve_url(&storage, &position.text_document.uri).await?;
    let lines = match &node.content {
        FileContent::Text { lines, .. } => lines,
        _ => return Err(LspError::BadRequest(eyre!("not a text file"))),
    };
    let line = lines
        .get(position.position.line as usize)
        .copied()
        .ok_or_else(|| LspError::BadRequest(eyre!("invalid line number")))?;
    let line = storage
        .get(line)
        .await?
        .ok_or_else(|| LspError::Internal(eyre!("can't find {}", line)))?;
    let position = position.position.character;

    let mut ranges = storage
        .find::<Range>(
            doc! {
                "line_id": { "$eq": line.id },
                "start": { "$lte": position },
                "end": { "$gt": position },
            },
            None,
        )
        .await?;

    ranges.sort_unstable_by_key(|r| r.end - r.start);
    let mut result = ResolvedPosition {
        node: node.id,
        line: line.id,
        position,
        found: Vec::new(),
        related: Vec::new(),
    };

    for range in ranges {
        let mut vertex: Vertex = storage
            .find_one(
                doc! {
                    "data.vertex": { "$eq": "Range" },
                    "data.range": { "$eq": range.id },
                },
                None,
            )
            .await?
            .ok_or_eyre(eyre!("no matching vertex found for {}", range.id))?;
        'inner: loop {
            let span = info_span!("vertex", vertex_id = %vertex.id);

            result.related.push(vertex.id);
            let outgoing: Vec<Edge> = storage
                .find(
                    doc! {
                        "data.edge": { "$eq": edge },
                        "data.out_v": { "$eq": vertex.id }
                    },
                    None,
                )
                .await?;
            if !outgoing.is_empty() {
                result.found = storage
                    .find(
                        doc! {
                            "_id": {
                                "$in": outgoing.iter().flat_map(|e| e.data.in_vs()).collect::<Vec<_>>()
                            }
                        },
                        None,
                    )
                    .await?;
                return Ok(result);
            }

            let next: Option<Edge> = storage
                .find_one(
                    doc! {
                        "data.edge": { "$eq": "Next" },
                        "data.out_v": { "$eq": vertex.id }
                    },
                    None,
                )
                .await?;
            let next = match next {
                None => break 'inner,
                Some(Edge {
                    data: EdgeInfo::Next(edge),
                    ..
                }) => edge,
                Some(x) => return Err(eyre!("unexpected edge: {:?}", x).into()),
            };
            vertex = storage
                .get(next.in_v)
                .await?
                .ok_or_eyre(eyre!("can't find next vertex {}", next.in_v))?;
        }
    }

    Ok(result)
}
