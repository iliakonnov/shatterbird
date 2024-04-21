use mongodb::bson::doc;
use mongodb::{Client, Collection, Database};
use tracing::{info, instrument};
use futures::stream::{StreamExt, TryStreamExt};

pub use model::{Id, Model};

pub mod model;

pub struct Storage {
    client: Client,
    database: Database
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
    
    pub async fn get_all<T: Model>(&self, ids: &[Id<T>]) -> eyre::Result<Vec<T>> {
        let cursor = self.access().find(doc! {"_id": {"$in": ids}}, None).await?;
        Ok(cursor.try_collect().await?)
    }

    pub async fn get<T: Model>(&self, id: Id<T>) -> eyre::Result<Option<T>> {
        Ok(self.access().find_one(doc! {"_id": id}, None).await?)
    }
    
    pub async fn insert_one<T: Model>(&self, model: &T) -> eyre::Result<()> {
        self.access::<T>().insert_one(model, None).await?;
        Ok(())
    }

    pub async fn insert_many<'a, T: Model + 'a>(&self, models: impl Iterator<Item=&'a T>) -> eyre::Result<()> {
        self.access::<T>().insert_many(models, None).await?;
        Ok(())
    }
}
