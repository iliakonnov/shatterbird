use super::{Id, Range};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub _id: Id<Self>,
    pub data: VertexData,
}

// See https://docs.rs/lsp-types/latest/lsp_types/lsif/enum.Vertex.html
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VertexData {
    MetaData {},
    Project {},
    Range {
        range: Id<Range>,
    },
    Moniker(lsp_types::Moniker),
}
