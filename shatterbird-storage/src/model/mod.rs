mod files;
mod history;
mod id;
pub mod lang;

pub use files::{BlobFile, FileContent, Line, Node, Range};
pub use history::Snapshot;
pub use id::Id;
pub use lang::{Edge, Vertex};

use serde::de::DeserializeOwned;
use serde::Serialize;

trait Private {}

#[allow(private_bounds)]
pub trait Model
where
    Self: Private + Serialize + DeserializeOwned + Unpin + Send + Sync,
{
    const COLLECTION: &'static str;
    fn id(&self) -> Id<Self>;
}
macro_rules! model {
    ($name:ty: $collection:literal) => {
        impl Private for $name {}
        impl Model for $name {
            const COLLECTION: &'static str = $collection;

            fn id(&self) -> Id<Self> {
                self._id
            }
        }
    };
    ($($name:ty: $collection:literal),* $(,)?) => {
        $(
            model!($name : $collection);
        )*
    };
}

model!(
    Range: "spans",
    Vertex: "results",
    Edge: "edges",
    Node: "nodes",
    Line: "lines",
    BlobFile: "blobs",
    Snapshot: "snapshots"
);
