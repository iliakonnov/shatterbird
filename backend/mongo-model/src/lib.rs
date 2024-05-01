mod id;
pub mod filter;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub use id::Id;
pub use filter::Filter;
pub use mongo_model_derive::Model;

pub trait ModelBounds
where
    Self: Serialize + DeserializeOwned + Send + Sync + Unpin,
{
}

impl<T> ModelBounds for T where Self: Serialize + DeserializeOwned + Send + Sync + Unpin {}

pub trait Model: ModelBounds {
    const COLLECTION: &'static str;
    fn id(&self) -> Id<Self>;
}
