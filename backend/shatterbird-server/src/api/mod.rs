use std::sync::Arc;

use axum::routing::get;
use axum::Router;

use crate::ServerState;

mod hello;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new().route("/", get(hello::hello))
}
