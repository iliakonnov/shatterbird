use shatterbird_storage::Storage;
use std::sync::Arc;

pub type AppState = axum::extract::State<Arc<ServerState>>;
pub struct ServerState {
    pub storage: Storage,
}
