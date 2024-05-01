use futures::stream::TryStreamExt;
use gix_hash::ObjectId;
use mongodb::bson::{doc, Document};
use mongodb::options::{FindOneOptions, FindOptions};
use mongodb::{bson, Client, Collection, Database};
use serde::Serialize;
use tracing::{info, instrument};

pub use mongo_model::{Id, Model};

pub mod model;
pub mod serializers;
pub mod ts;

pub struct Storage {
    client: Client,
    database: Database,
}

impl Storage {
    #[instrument]
    pub async fn connect(uri: &str) -> eyre::Result<Self> {
        let client = Client::with_uri_str(uri).await?;
        client.warm_connection_pool().await;
        let database = match client.default_database() {
            Some(x) => x,
            None => return Err(eyre::eyre!("no database specified in connection string")),
        };
        info!("successfully connected to db {}", database.name());
        Ok(Storage { client, database })
    }

    pub async fn shutdown(self) -> eyre::Result<()> {
        self.client.shutdown().await;
        Ok(())
    }

    pub fn access<T: Model>(&self) -> Collection<T> {
        self.database.collection(T::COLLECTION)
    }

    pub async fn get<T: Model>(&self, id: Id<T>) -> eyre::Result<Option<T>> {
        Ok(self.access().find_one(doc! {"_id": id}, None).await?)
    }

    pub async fn find_one<T: Model>(
        &self,
        filter: impl Into<Option<Document>>,
        options: impl Into<Option<FindOneOptions>>,
    ) -> eyre::Result<Option<T>> {
        Ok(self.access().find_one(filter, None).await?)
    }

    pub async fn find<T: Model>(
        &self,
        filter: impl Into<Option<Document>>,
        options: impl Into<Option<FindOptions>>,
    ) -> eyre::Result<Vec<T>> {
        Ok(self
            .access()
            .find(filter, options)
            .await?
            .try_collect()
            .await?)
    }

    pub async fn get_by_oid<T: Model>(&self, oid: gix_hash::ObjectId) -> eyre::Result<Option<T>> {
        #[derive(Serialize)]
        struct Filter {
            #[serde(with = "crate::serializers::gix_hash")]
            oid: ObjectId,
        }
        let filter = bson::to_bson(&Filter { oid })?;
        Ok(self
            .access()
            .find_one(filter.as_document().cloned(), None)
            .await?)
    }

    pub async fn insert_one<T: Model>(&self, model: &T) -> eyre::Result<()> {
        self.access::<T>().insert_one(model, None).await?;
        Ok(())
    }

    pub async fn insert_many<'a, T: Model + 'a>(
        &self,
        models: impl Iterator<Item = &'a T>,
    ) -> eyre::Result<()> {
        self.access::<T>().insert_many(models, None).await?;
        Ok(())
    }
}
