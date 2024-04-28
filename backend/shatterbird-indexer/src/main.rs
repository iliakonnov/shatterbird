#![feature(exclusive_wrapper)]

use std::io::BufReader;
use std::path::PathBuf;

use clap::{arg, Parser, Subcommand};
use tracing_error::ErrorLayer;
use tracing_subscriber::{Layer, Registry};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod lsif;
mod git;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    db_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Lsif {
        #[arg(long)]
        input: PathBuf
    },
    Git {
        #[arg(long)]
        root: PathBuf
    }
}

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
    
    let args = Args::parse();
    let storage = shatterbird_storage::Storage::connect(&args.db_url).await?;
    
    match args.command {
        Command::Lsif { input } => match input.as_os_str().as_encoded_bytes() {
            b"-" => {
                let stdin = BufReader::new(std::io::stdin());
                lsif::load_lsif(&storage, stdin).await?;
            },
            _ => {
                let file = BufReader::new(std::fs::File::open(input)?);
                lsif::load_lsif(&storage, file).await?;
            }
        },
        Command::Git { root } => {
            git::index(&storage, &root).await?;
        }
    }

    storage.shutdown().await?;
    Ok(())
}
