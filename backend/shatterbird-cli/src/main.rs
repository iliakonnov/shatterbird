mod graph;

use clap::{Parser, Subcommand};
use tracing::instrument;
use tracing_error::ErrorLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, Registry};

#[derive(Parser)]
struct Options {
    #[arg(long)]
    pub db_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Graph(graph::Graph),
}

pub struct App {
    pub storage: shatterbird_storage::Storage,
}

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

    let opts: Options = shatterbird_utils::cli::load_args();
    let app = App {
        storage: shatterbird_storage::Storage::connect(&opts.db_url).await?,
    };
    match opts.command {
        Command::Graph(graph) => graph.run(app).await,
    }
}
