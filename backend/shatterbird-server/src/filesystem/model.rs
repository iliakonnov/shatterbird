use std::collections::HashMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use shatterbird_storage::ts;
use shatterbird_storage::model::{BlobFile, FileContent, Id, Line, Node};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum EitherNode {
    Full(FullNode),
    Short(NodeInfo),
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FullNode {
    #[serde(flatten)]
    pub info: NodeInfo,
    pub content: ExpandedFileContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ExpandedFileContent {
    Symlink {
        target: String,
    },
    Directory {
        children: HashMap<String, NodeInfo>,
    },
    Text {
        size: u64,
        lines: Vec<Line>,
    },
    Blob {
        size: u64,
        #[ts(as = "ts::Id<BlobFile>")]
        content: Id<BlobFile>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NodeInfo {
    #[ts(as="ts::Id<Node>")]
    pub _id: Id<Node>,
    pub kind: ContentKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ContentKind {
    Symlink,
    Directory,
    Text,
    Blob,
}

impl IntoResponse for EitherNode {
    fn into_response(self) -> Response {
        match self {
            EitherNode::Full(x) => Json(x).into_response(),
            EitherNode::Short(x) => Json(x).into_response(),
            EitherNode::NotFound(msg) => (StatusCode::NOT_FOUND, msg).into_response(),
        }
    }
}

impl From<&FileContent> for ContentKind {
    fn from(value: &FileContent) -> Self {
        match value {
            FileContent::Symlink { .. } => Self::Symlink,
            FileContent::Directory { .. } => Self::Directory,
            FileContent::Text { .. } => Self::Text,
            FileContent::Blob { .. } => Self::Blob,
        }
    }
}
