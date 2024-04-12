use super::{Id, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    pub _id: Id<Self>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub _id: Id<Self>,
    pub line_id: Id<Line>,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobFile {
    pub _id: Id<Self>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileContent {
    Symlink { target: String },
    Directory { children: HashMap<String, Id<Node>> },
    Text { size: u64, lines: Vec<Id<Line>> },
    Blob { size: u64, content: Id<BlobFile> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub _id: Id<Self>,
    pub created_at: Id<Snapshot>,
    pub content: FileContent,
}
