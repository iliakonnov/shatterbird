//! https://code.visualstudio.com/api/references/vscode-api#FileSystemProvider

use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::Router;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use shatterbird_storage::model::{BlobFile, Commit, FileContent, Id, Node};

use crate::filesystem::model::{EitherNode, FullNode, NodeInfo};
use crate::state::AppState;
use crate::utils::{AppResult, May404};
use crate::ServerState;

mod model;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/tree/:commit", get(get_commit))
        .route("/tree/:commit/*uri", get(by_path))
        .route("/nodes/:id", get(by_id))
        .route("/blobs/:id", get(get_blob))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct IsShort {
    #[serde(default)]
    short: bool,
}

async fn get_node(state: &ServerState, id: Id<Node>, is_short: bool) -> AppResult<EitherNode> {
    let node = match state.storage.get(id).await? {
        Some(x) => x,
        None => return Ok(EitherNode::NotFound("unknown id".to_string())),
    };
    let info = NodeInfo {
        _id: node._id,
        kind: (&node.content).into(),
    };
    if is_short {
        return Ok(EitherNode::Short(info));
    }

    let text = match &node.content {
        FileContent::Text { lines, .. } => {
            let lines = state.storage.get_all(&lines[..]).await?;
            Some(lines.into_iter().map(|ln| ln.text).collect())
        }
        _ => None,
    };

    Ok(EitherNode::Full(FullNode {
        info,
        content: node.content,
        text,
    }))
}

#[axum::debug_handler(state = Arc<ServerState>)]
async fn by_path(
    State(state): AppState,
    Path((commit, path)): Path<(String, String)>,
    Query(is_short): Query<IsShort>,
) -> AppResult<EitherNode> {
    let commit = state
        .storage
        .get::<Commit>(Id::from(ObjectId::from_str(&commit)?))
        .await?;
    let root = match commit {
        Some(x) => x.root,
        None => return Ok(EitherNode::NotFound("unknown commit".to_string())),
    };

    let mut next = root;
    for stem in path.trim_matches('/').split('/') {
        if stem.is_empty() {
            continue;
        }
        let curr = match state.storage.get(next).await? {
            Some(x) => x,
            None => return Ok(EitherNode::NotFound(format!("can't find node {}", next))),
        };
        let children = match curr.content {
            FileContent::Directory { children, .. } => children,
            _ => {
                return Ok(EitherNode::NotFound(format!(
                    "node {} not a directory",
                    next
                )))
            }
        };
        next = match children.get(stem) {
            Some(x) => *x,
            None => {
                return Ok(EitherNode::NotFound(format!(
                    "node {} does not have child {:?}",
                    next, stem
                )))
            }
        };
    }

    get_node(&state, next, is_short.short).await
}

#[axum::debug_handler(state = Arc<ServerState>)]
async fn get_commit(
    State(state): AppState,
    Path(commit): Path<String>,
    Query(is_short): Query<IsShort>,
) -> AppResult<EitherNode> {
    let commit = state
        .storage
        .get::<Commit>(Id::from(ObjectId::from_str(&commit)?))
        .await?;
    let root = match commit {
        Some(x) => x.root,
        None => return Ok(EitherNode::NotFound("unknown commit".to_string())),
    };
    get_node(&state, root, is_short.short).await
}

#[axum::debug_handler(state = Arc<ServerState>)]
async fn by_id(
    State(state): AppState,
    Path(id): Path<Id<Node>>,
    Query(is_short): Query<IsShort>,
) -> AppResult<EitherNode> {
    get_node(&state, id, is_short.short).await
}

#[axum::debug_handler(state = Arc<ServerState>)]
async fn get_blob(
    State(state): AppState,
    Path(id): Path<Id<BlobFile>>,
) -> AppResult<May404<Vec<u8>>> {
    let blob = state.storage.get(id).await?;
    Ok(May404(blob.map(|x| x.data)))
}
