use crate::lsif::converter::Converter;
use crate::lsif::graph::Graph;
use bumpalo::Bump;
use shatterbird_storage::Storage;
use tracing::{info, instrument};

mod converter;
mod graph;
mod lsif_ext;

#[instrument(skip_all)]
pub async fn load_lsif<R: std::io::BufRead>(storage: &Storage, input: R) -> eyre::Result<()> {
    info!("parsing graph");
    let arena = Bump::new();
    let mut graph = Graph::new(&arena);
    for line in input.lines() {
        let line = line?;
        let entry = serde_json::from_str(&line)?;
        graph.add(entry)
    }

    info!("converting graph");
    let converter = Converter::new(&graph);
    converter.load().await?;

    info!("saving");
    converter.save(storage).await?;

    Ok(())
}
