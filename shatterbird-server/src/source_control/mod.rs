use crate::state::ServerState;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
}
