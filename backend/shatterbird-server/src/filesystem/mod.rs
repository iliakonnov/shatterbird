//! https://code.visualstudio.com/api/references/vscode-api#FileSystemProvider

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use futures::TryStreamExt;
use log::warn;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use shatterbird_storage::model::{BlobFile, Commit, FileContent, Node};
use shatterbird_storage::Id;

use crate::filesystem::model::{EitherNode, ExpandedFileContent, FullNode, NodeInfo};
use crate::state::AppState;
use crate::utils::{AppResult, May404};
use crate::ServerState;

mod model;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/commits", get(list_commits))
        .route("/commits/by-id/:commit", get(get_commit_by_id))
        .route("/commits/by-oid/:oid", get(get_commit_by_git))
        .route("/tree/:commit", get(get_commit_root))
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
        _id: node.id,
        kind: (&node.content).into(),
    };
    if is_short {
        return Ok(EitherNode::Short(info));
    }

    let content = match node.content {
        FileContent::Symlink { target } => ExpandedFileContent::Symlink { target },
        FileContent::Blob { size, content } => ExpandedFileContent::Blob { size, content },
        FileContent::Directory { children } => {
            let children_ids: Vec<_> = children.values().copied().collect();
            let children_nodes = state
                .storage
                .find::<Node>(doc! {"_id": {"$in": children_ids}}, None)
                .await?
                .into_iter()
                .map(|c| (c.id, c))
                .collect::<HashMap<_, _>>();
            ExpandedFileContent::Directory {
                children: children
                    .into_iter()
                    .filter_map(|(k, id)| {
                        children_nodes
                            .get(&id)
                            .map(|node| NodeInfo {
                                _id: node.id,
                                kind: (&node.content).into(),
                            })
                            .map(|node| (k, node))
                    })
                    .collect(),
            }
        }
        FileContent::Text { size, lines } => {
            let lines = state
                .storage
                .find(doc! {"_id": {"$in": lines}}, None)
                .await?;
            ExpandedFileContent::Text { size, lines }
        }
    };

    Ok(EitherNode::Full(FullNode { info, content }))
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
async fn get_commit_root(
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

#[axum::debug_handler(state = Arc<ServerState>)]
async fn list_commits(State(state): AppState) -> AppResult<Json<Vec<Commit>>> {
    let cursor = state.storage.access::<Commit>().find(None, None).await?;
    let commits: Vec<Commit> = cursor.try_collect().await?;
    Ok(Json(commits))
}

#[axum::debug_handler(state = Arc<ServerState>)]
async fn get_commit_by_id(
    State(state): AppState,
    Path(commit): Path<String>,
) -> AppResult<May404<Json<Commit>>> {
    Ok(May404(
        state
            .storage
            .get::<Commit>(Id::from(ObjectId::from_str(&commit)?))
            .await?
            .map(Json),
    ))
}

#[axum::debug_handler(state = Arc<ServerState>)]
async fn get_commit_by_git(
    State(state): AppState,
    Path(commit): Path<String>,
) -> AppResult<May404<Json<Commit>>> {
    let oid = match gix_hash::ObjectId::from_str(&commit) {
        Ok(x) => x,
        Err(e) => {
            warn!("invalid commit id {}: {}", commit, e);
            return Ok(May404(None));
        }
    };
    Ok(May404(
        state.storage.get_by_oid::<Commit>(oid).await?.map(Json),
    ))
}
