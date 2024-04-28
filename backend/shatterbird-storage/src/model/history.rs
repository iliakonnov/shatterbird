use super::{Id, Node};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub _id: Id<Self>,
    pub root: Id<Node>,
    pub commit_id: String,
    pub parents: Vec<Id<Snapshot>>,
}
