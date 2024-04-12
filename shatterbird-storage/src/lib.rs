use mongodb::bson::doc;
use mongodb::Collection;

use crate::model::Model;
use tracing::instrument;

pub mod model;

pub struct Storage {
    client: mongodb::Client,
}

impl Storage {
    #[instrument]
    pub async fn connect(uri: &str) -> eyre::Result<Self> {
        let client = mongodb::Client::with_uri_str(uri).await?;
        client.warm_connection_pool().await;
        Ok(Storage { client })
    }

    pub fn access<T: Model>(&self) -> Collection<T> {
        self.client.database("db").collection(T::COLLECTION)
    }

    pub async fn get<T: Model>(&self, id: Id<T>) -> eyre::Result<Option<T>> {
        Ok(self.access().find_one(doc! {"_id": id.id}, None).await?)
    }
}
