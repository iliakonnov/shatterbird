use crate::api::ServerState;
use crate::utils::AppError;
use axum::extract::Path;
use eyre::eyre;
use std::sync::Arc;
use tracing::{info, instrument};

#[instrument]
#[axum::debug_handler(state = Arc<ServerState>)]
pub async fn hello() -> &'static str {
    "Hello, World!"
}

#[instrument]
#[axum::debug_handler(state = Arc<ServerState>)]
pub async fn hello_name(Path(name): Path<String>) -> Result<String, AppError> {
    info!("hello");
    if name == "asdf" {
        Err(eyre!("Nope, {}.", name).into())
    } else {
        Ok(format!("Hello, {}!", name))
    }
}
