mod id;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub use id::Id;
pub use mongo_model_derive::Model;
pub use {bson, serde};

pub trait Model
where
    Self: Serialize + DeserializeOwned + Unpin + Send + Sync,
{
    const COLLECTION: &'static str;
    fn id(&self) -> Id<Self>;
}

pub trait ModelFilter
where
    Self: Serialize,
{
    type Model: Model;

    fn build(self) -> Option<bson::Document>;
}
