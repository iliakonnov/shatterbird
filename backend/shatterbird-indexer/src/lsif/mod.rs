use crate::lsif::converter::Converter;
use crate::lsif::graph::Graph;
use bumpalo::Bump;
use eyre::{eyre, OptionExt};
use shatterbird_storage::model::Commit;
use shatterbird_storage::{Id, Storage};
use std::str::FromStr;
use either::Either;
use gix::ObjectId;
use tracing::{info, instrument, warn};

mod converter;
mod graph;
mod lsif_ext;

#[derive(Debug, Clone)]
pub struct RootMapping {
    pub dir: String,
    pub node: Either<Id<Commit>, ObjectId>,
}

#[instrument(skip_all)]
pub async fn load_lsif<R: std::io::BufRead>(
    storage: &Storage,
    input: R,
    roots: Vec<RootMapping>,
    save: bool,
) -> eyre::Result<()> {
    info!("parsing graph");
    let arena = Bump::new();
    let mut graph = Graph::new(&arena);
    for line in input.lines() {
        let line = line?;
        let entry = serde_json::from_str(&line)
            .map_err(|e| eyre!("failed to parse line {}: {}", line, e))?;
        graph.add(entry)
    }

    info!("converting graph");
    let converter = Converter::new(storage, &graph, roots);
    converter.load().await?;

    if save {
        info!("saving");
        converter.save().await?;
    }

    Ok(())
}

impl FromStr for RootMapping {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (dir, node) = s.rsplit_once('=').ok_or_eyre("invalid root mapping")?;
        let dir = dir.to_string();
        let node = match node.len() {
            24 => Either::Left(node.parse().map_err(|e| eyre!("failed to parse node id: {e}"))?),
            40 => Either::Right(node.parse().map_err(|e| eyre!("failed to parse node id: {e}"))?),
            _ => return Err(eyre!("invalid root id: {node}")),
        };
        Ok(RootMapping { dir, node })
    }
}
