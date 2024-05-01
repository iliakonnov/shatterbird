use eyre::eyre;
use gix_hash::ObjectId;
use lsp_types::Url;
use mongodb::bson::doc;
use tracing::{debug, instrument};

use shatterbird_storage::model::{Commit, FileContent, Line, Node, Range};
use shatterbird_storage::{filter, Storage};

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
    pub node: Node,
    pub line: Line,
    pub ranges: Vec<Range>,
    pub position: u32,
}

#[instrument(skip_all, fields(uri = %position.text_document.uri, position = ?position.position))]
pub async fn resolve_position(
    storage: &Storage,
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
    let ranges = storage
        .find_all(filter! {
            Range { line_id == line.id },
            Range { start <= position },
            Range { end > position },
        })
        .await?;
    Ok(ResolvedPosition {
        node,
        line,
        ranges,
        position,
    })
}
