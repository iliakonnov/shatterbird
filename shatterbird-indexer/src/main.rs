#![feature(exclusive_wrapper)]

use std::io::Write;
use crate::converter::Converter;
use crate::graph::Graph;
use bumpalo::Bump;
use futures::SinkExt;
use tracing::info;
use tracing_error::ErrorLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, Registry};

mod converter;
mod graph;
mod lsif_ext;

#[tokio::main(flavor = "current_thread")]
async fn main() -> eyre::Result<()> {
    Registry::default()
        .with(ErrorLayer::default())
        .with(
            tracing_subscriber::fmt::layer()
                .with_filter(tracing_subscriber::EnvFilter::from_default_env()),
        )
        .init();
    color_eyre::install()?;

    let mut args = std::env::args().skip(1);
    let uri = match args.next() {
        Some(x) => x,
        None => {
            return Err(eyre::eyre!(
                "you must provide mongodb connection string as first parameter"
            ))
        }
    };
    if args.next().is_some() {
        return Err(eyre::eyre!("only one argument is expected"));
    }
    let storage = shatterbird_storage::Storage::connect(&uri).await?;

    info!("parsing graph");
    let arena = Bump::new();
    let mut graph = Graph::new(&arena);
    for line in std::io::stdin().lines() {
        let line = line?;
        let entry = serde_json::from_str(&line)?;
        graph.add(entry)
    }

    info!("converting graph");
    let converter = Converter::new(&graph);
    converter.load().await?;

    info!("saving");
    converter.save(&storage).await?;

    storage.shutdown().await?;

    Ok(())
}
