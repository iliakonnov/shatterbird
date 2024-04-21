use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use shatterbird_storage::model::{FileContent, Id, Node};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EitherNode {
    Full(FullNode),
    Short(NodeInfo),
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullNode {
    #[serde(flatten)]
    pub info: NodeInfo,
    pub content: FileContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub _id: Id<Node>,
    pub kind: ContentKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
