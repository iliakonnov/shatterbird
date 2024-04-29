use mongo_model::Model;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Model, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
#[serde(bound(serialize = "T: Serialize"))]
#[mongo_model(collection = "test")]
struct Test<T: Model> {
    #[serde(rename="_id")]
    id: mongo_model::Id<Self>,
    foo: T,
}
