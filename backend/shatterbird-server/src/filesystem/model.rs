use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use shatterbird_storage::model::{BlobFile, FileContent, Line, Node};
use shatterbird_storage::{ts, Id};
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum EitherNode {
    Full(FullNode),
    Short(NodeInfo),
    NotFound(String),
}

/// Хранит полную информацию об узле файлового дерева
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FullNode {
    #[serde(flatten)]
    pub info: NodeInfo,

    #[ts(inline)]
    pub content: ExpandedFileContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ExpandedFileContent {
    Symlink {
        /// Ссылка на целевой файл
        target: String,
    },
    Directory {
        /// Дочерние узлы этой директории вместе с их именами
        #[ts(inline)]
        children: HashMap<String, NodeInfo>,
    },
    Text {
        /// Размер в байтах
        size: u64,

        /// Строки файла
        #[ts(inline)]
        lines: Vec<Line>,
    },
    Blob {
        /// Размер в байтах
        size: u64,

        /// Идентификатор этого файла в базе данных
        #[ts(as = "ts::Id<BlobFile>")]
        content: Id<BlobFile>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NodeInfo {
    /// Идентифкатор этого узла в базе данных
    #[ts(as = "ts::Id<Node>")]
    pub _id: Id<Node>,

    /// Тип узла
    #[ts(inline)]
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
