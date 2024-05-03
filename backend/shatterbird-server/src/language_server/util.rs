use eyre::{eyre, OptionExt};
use lsp_types::Url;
use mongodb::bson::doc;
use tracing::{debug, info_span, instrument};

use shatterbird_storage::model::lang::{EdgeInfo, EdgeInfoDiscriminants};
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
    pub ranges: Vec<Id<Range>>,
    pub found: Vec<Vertex>,
}

#[instrument(skip_all, fields(uri = %position.text_document.uri, edge=?edge, position = ?position.position))]
pub async fn find(
    storage: &Storage,
    edge: Option<EdgeInfoDiscriminants>,
    position: &lsp_types::TextDocumentPositionParams,
    reverse: bool,
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
        ranges: ranges.iter().map(|i| i.id).collect(),
        found: Vec::new(),
    };

    let edge: &'static str = match edge {
        Some(x) => x.into(),
        None => return Ok(result),
    };
    for range in ranges {
        let initital = storage
            .find_one::<Vertex>(
                doc! {
                    "data.vertex": { "$eq": "Range" },
                    "data.range": { "$eq": range.id },
                },
                None,
            )
            .await?
            .ok_or_eyre(eyre!("no matching vertex found for {}", range.id))?;
        let mut queue = Vec::new();
        queue.push(initital.id);
        while let Some(vertex) = queue.pop() {
            let outgoing: Vec<Edge> = storage
                .find(
                    if !reverse {
                        doc! {
                            "data.edge": { "$eq": edge },
                            "data.out_v": { "$eq": vertex }
                        }
                    } else {
                        doc! {
                            "data.edge": { "$eq": edge },
                            "$or": [
                                { "data.in_v": vertex },
                                { "data.in_vs": vertex },
                            ]
                        }
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

            let next = storage
                .find::<Edge>(
                    if !reverse {
                        doc! {
                            "data.edge": { "$eq": "Next" },
                            "data.out_v": { "$eq": vertex }
                        }
                    } else {
                        doc! {
                            "data.edge": { "$eq": "Next" },
                            "$or": [
                                {"data.in_v": vertex },
                                {"data.in_vs": vertex },
                            ]
                        }
                    },
                    None,
                )
                .await?;
            for i in next {
                match i.data {
                    EdgeInfo::Next(edge) => {
                        if !reverse {
                            queue.push(edge.in_v);
                        } else {
                            queue.push(edge.out_v);
                        }
                    }
                    _ => return Err(eyre!("unexpected edge: {:?}", i).into()),
                }
            }
        }
    }

    Ok(result)
}

pub async fn find_line_no(storage: &Storage, range: &Range) -> Result<u32, LspError> {
    let line = storage
        .get(range.line_id)
        .await?
        .ok_or_eyre(eyre!("line {} referenced, but not found", range.line_id))?;
    let doc = storage
        .find_one::<Node>(
            doc! {"content.Text.lines": { "$elemMatch": { "$eq": line.id }} },
            None,
        )
        .await?
        .ok_or_eyre(eyre!("can't find file containing line {}", line.id))?;
    let line_no = match doc.content {
        FileContent::Text { lines, .. } => lines.iter().position(|&x| x == line.id).unwrap(),
        _ => {
            return Err(LspError::Internal(eyre!(
                "expected text file, found {:?}",
                doc.content
            )))
        }
    };
    Ok(line_no as _)
}
