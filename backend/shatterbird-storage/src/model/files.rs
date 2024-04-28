use super::{Id, Snapshot};
use crate::ts;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Line {
    #[ts(as = "ts::Id<Self>")]
    pub _id: Id<Self>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Range {
    #[ts(as = "ts::Id<Self>")]
    pub _id: Id<Self>,
    #[ts(as = "ts::Id<Line>")]
    pub line_id: Id<Line>,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BlobFile {
    #[ts(as = "ts::Id<Self>")]
    pub _id: Id<Self>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum FileContent {
    Symlink {
        target: String,
    },
    Directory {
        #[ts(as = "HashMap<String, ts::Id<Node>>")]
        children: HashMap<String, Id<Node>>,
    },
    Text {
        size: u64,
        #[ts(as = "Vec<ts::Id<Line>>")]
        lines: Vec<Id<Line>>,
    },
    Blob {
        size: u64,
        #[ts(as = "ts::Id<BlobFile>")]
        content: Id<BlobFile>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Node {
    #[ts(as = "ts::Id<Self>")]
    pub _id: Id<Self>,

    #[ts(as = "String")]
    #[serde(with = "crate::serializers::gix_hash")]
    pub oid: gix_hash::ObjectId,

    pub content: FileContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Commit {
    #[ts(as = "ts::Id<Self>")]
    pub _id: Id<Self>,
    
    #[ts(as = "String")]
    #[serde(with = "crate::serializers::gix_hash")]
    pub oid: gix_hash::ObjectId,
    
    #[ts(as = "ts::Id<Node>")]
    pub root: Id<Node>,
    
    #[ts(as = "Vec<ts::Id<Commit>>")]
    pub parents: Vec<Id<Commit>>,
}
