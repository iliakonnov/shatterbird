use std::collections::HashMap;

use either::Either;
use eyre::{eyre, OptionExt, Report};
use lsp_types::Url;
use mongodb::bson::doc;
use thiserror::Error;
use tracing::{instrument, warn};

use crate::model::lang::{EdgeInfo, EdgeInfoDiscriminants};
use crate::model::{Commit, Edge, FileContent, Line, Node, Range, Vertex};
use crate::{Id, Storage};

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("file {url} not found{}", message.as_ref().map(|x| format!(": {}", x)).unwrap_or_default())]
    FileNotFound { url: Url, message: Option<String> },

    #[error("invalid commit: {0}")]
    InvalidCommit(#[from] gix_hash::decode::Error),

    #[error("other error: {0}")]
    Internal(
        #[from]
        #[source]
        Report,
    ),
}

fn not_found<T: ToString>(url: &Url, message: T) -> ResolveError {
    ResolveError::FileNotFound {
        url: url.clone(),
        message: Some(message.to_string()),
    }
}

#[instrument(skip_all, fields(uri = %uri))]
pub async fn resolve_url(storage: &Storage, uri: &Url) -> Result<Node, ResolveError> {
    let splitted = uri
        .path()
        .split('/')
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();
    if splitted.is_empty() {
        return Err(not_found(uri, "empty path"));
    }
    let commit = splitted[0].parse()?;
    let commit: Commit = storage
        .get_by_oid(commit)
        .await?
        .ok_or_else(|| not_found(uri, format!("no such commit: {}", commit)))?;
    let mut curr = commit.root;
    for &component in splitted[1..].iter() {
        let node = match storage.get(curr).await? {
            Some(x) => x,
            None => return Err(ResolveError::Internal(eyre!("can't find {}", curr))),
        };
        let children = match node.content {
            FileContent::Directory { children } => children,
            _ => return Err(not_found(uri, format!("{} is not a directory", curr)))?,
        };
        curr = match children.get(component) {
            Some(x) => *x,
            None => {
                return Err(not_found(
                    uri,
                    format!("no child named {} in {}", component, curr),
                ))
            }
        };
    }
    storage
        .get(curr)
        .await?
        .ok_or_else(|| ResolveError::Internal(eyre!("can't find {}", curr)))
}

#[derive(Debug)]
pub struct ResolvedPosition {
    pub node: Id<Node>,
    pub line: Id<Line>,
    pub position: u32,
    pub ranges: Vec<Id<Range>>,
    pub found: Vec<Vertex>,
}

#[derive(Debug, Error)]
pub enum FindError {
    #[error("failed to resolve file: {0}")]
    CantResolve(#[from] ResolveError),

    #[error("not a text file")]
    NotATextFile,

    #[error("invalid line number")]
    InvalidLineNumber,

    #[error("other error: {0}")]
    Internal(
        #[from]
        #[source]
        Report,
    ),
}

#[instrument(skip_all, fields(uri = %position.text_document.uri, edge=?edge, position = ?position.position))]
pub async fn find(
    storage: &Storage,
    edge: Option<EdgeInfoDiscriminants>,
    position: &lsp_types::TextDocumentPositionParams,
    reverse: bool,
) -> Result<ResolvedPosition, FindError> {
    let node = resolve_url(&storage, &position.text_document.uri).await?;
    let lines = match &node.content {
        FileContent::Text { lines, .. } => lines,
        _ => return Err(FindError::NotATextFile),
    };
    let line = lines
        .get(position.position.line as usize)
        .copied()
        .ok_or_else(|| FindError::InvalidLineNumber)?;
    let line = storage
        .get(line)
        .await?
        .ok_or_else(|| FindError::Internal(eyre!("can't find {}", line)))?;
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

pub async fn find_line_no(storage: &Storage, range: &Range) -> Result<u32, Report> {
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
        _ => return Err(eyre!("expected text file, found {:?}", doc.content)),
    };
    Ok(line_no as _)
}

pub async fn find_file_path(storage: &Storage, range: &Range) -> Result<Vec<String>, Report> {
    let nodes = storage
        .find::<Node>(doc! { "_id": { "$in": &range.path }}, None)
        .await?
        .into_iter()
        .map(|i| (i.id, i))
        .collect::<HashMap<_, _>>();
    let mut path = Vec::new();

    let root = range.path[0];
    let commit = storage
        .find_one::<Commit>(doc! { "root": root }, None)
        .await?
        .ok_or_eyre(eyre!("no commit for root {} is found", root))?;
    let commit_oid = commit.oid.to_hex().to_string();

    path.push(&commit_oid[..]);
    for pair in range.path.windows(2) {
        let (parent, curr) = match pair {
            &[parent, curr] => (parent, curr),
            _ => unreachable!(),
        };
        let parent = nodes
            .get(&parent)
            .ok_or_eyre(eyre!("node {} not found in database", parent))?;
        let children = match &parent.content {
            FileContent::Directory { children, .. } => children,
            _ => return Err(eyre::eyre!("node {:?} is not a directory", parent.id)),
        };
        let name = children
            .iter()
            .find(|(k, v)| **v == curr)
            .map(|(k, _)| k)
            .ok_or_eyre(eyre!("node {} not found in {}", curr, parent.id))?;
        path.push(&name[..])
    }
    Ok(path.into_iter().map(|x| x.to_owned()).collect())
}