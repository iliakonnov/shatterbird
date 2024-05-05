use axum::Router;

use axum::http::StatusCode;
use std::sync::Arc;

use crate::state::ServerState;
use shatterbird_storage::Storage;
use tracing::instrument;

use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, registry::Registry};

mod filesystem;
mod language_server;
mod settings;
mod state;
pub mod utils;

#[tokio::main]
#[instrument]
async fn main() -> eyre::Result<()> {
    Registry::default()
        .with(ErrorLayer::default())
        .with(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_filter(tracing_subscriber::EnvFilter::from_default_env()),
        )
        .init();
    color_eyre::install()?;
    settings::init()?;

    let settings = settings::get()?;
    let state = Arc::new(ServerState {
        storage: Storage::connect(&settings.db_url).await?,
    });

    let listener = tokio::net::TcpListener::bind(&settings.addr).await?;
    let router = Router::new()
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .nest("/api/fs", filesystem::router())
        .nest("/api/lsp", language_server::router())
        .fallback(|| async { (StatusCode::NOT_FOUND, "unknown route") })
        .with_state(state)
        .layer(tower_http::cors::CorsLayer::permissive());
    axum::serve(listener, router).await?;

    Ok(())
}
