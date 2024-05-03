use crate::App;
use bson::doc;
use clap::Args;
use eyre::{eyre, OptionExt};
use futures::FutureExt;
use shatterbird_storage::model::lang::{
    EdgeInfoDiscriminants, VertexInfo, VertexInfoDiscriminants,
};
use shatterbird_storage::model::{Edge, Range, Vertex};
use shatterbird_storage::Id;
use std::collections::{HashMap, HashSet};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use tracing::instrument;

#[derive(Args)]
pub struct Graph {
    #[arg(long)]
    range_id: Id<Range>,

    #[arg(short)]
    output: PathBuf,
}

struct State<W> {
    range_id: Id<Range>,
    vertices: HashMap<Id<Vertex>, bool>,
    edges: HashSet<Id<Edge>>,
    writer: W,
}

impl Graph {
    pub async fn run(self, app: App) -> eyre::Result<()> {
        let range = app
            .storage
            .get(self.range_id)
            .await?
            .ok_or_eyre(eyre!("range {} not found", self.range_id))?;
        let initital = app
            .storage
            .find_one::<Vertex>(
                doc! {
                    "data.vertex": { "$eq": "Range" },
                    "data.range": { "$eq": range.id },
                },
                None,
            )
            .await?
            .ok_or_eyre(eyre!("no matching vertex found for {}", range.id))?;
        let mut state = State {
            range_id: self.range_id,
            vertices: HashMap::new(),
            edges: HashSet::new(),
            writer: BufWriter::new(std::fs::File::create(self.output)?),
        };

        writeln!(&mut state.writer, "strict digraph {{")?;
        state.visit_vertex(&app.storage, initital.id).await?;
        writeln!(&mut state.writer, "}}")?;
        Ok(())
    }
}

impl<W: Write> State<W> {
    #[instrument(level = tracing::Level::INFO, skip_all, fields(v=%id))]
    async fn visit_vertex(
        &mut self,
        storage: &shatterbird_storage::Storage,
        id: Id<Vertex>,
    ) -> eyre::Result<()> {
        if self.vertices.contains_key(&id) {
            return Ok(());
        }

        let vertex = storage
            .get(id)
            .await?
            .ok_or_eyre(eyre!("vertex {} not found", id))?;

        if let VertexInfo::Document(_)
        | VertexInfo::PackageInformation(_)
        | VertexInfo::Moniker(_) = vertex.data
        {
            self.vertices.insert(id, false);
            return Ok(());
        }
        self.vertices.insert(id, true);

        let fillcolor = if self.vertices.len() == 1 {
            ", fillcolor=yellow, style=filled"
        } else {
            ""
        };

        let label: &'static str = VertexInfoDiscriminants::from(&vertex.data).into();
        writeln!(&mut self.writer)?;
        writeln!(
            &mut self.writer,
            r#"node{} [label="{}"{}];"#,
            vertex.id.id, label, fillcolor
        )?;
        for ln in format!("{:#?}", &vertex).lines() {
            writeln!(&mut self.writer, "// {ln}")?;
        }

        // () <- ...
        let out_edges = storage
            .find::<Edge>(doc! {"data.out_v": vertex.id}, None)
            .await?;

        // () -> ...
        let in_edges = storage
            .find::<Edge>(
                doc! {
                    "$or": [
                        { "data.in_v": vertex.id },
                        { "data.in_vs": vertex.id },
                    ]
                },
                None,
            )
            .await?;

        for node in out_edges {
            self.visit_edge(storage, node, true).boxed_local().await?;
        }
        for node in in_edges {
            self.visit_edge(storage, node, false).boxed_local().await?;
        }

        Ok(())
    }

    #[instrument(level = tracing::Level::INFO, skip_all, fields(e=%edge.id))]
    async fn visit_edge(
        &mut self,
        storage: &shatterbird_storage::Storage,
        edge: Edge,
        out: bool,
    ) -> eyre::Result<()> {
        if self.edges.contains(&edge.id) {
            return Ok(());
        }
        self.edges.insert(edge.id);

        if out {
            // () <- ...
            for in_v in edge.data.in_vs() {
                self.visit_vertex(storage, in_v).await?;
            }
        } else {
            // () -> ...
            self.visit_vertex(storage, edge.data.out_v()).await?;
        }

        let in_vs = edge
            .data
            .in_vs()
            .filter(|x| self.vertices.get(x).copied().unwrap_or_default())
            .collect::<Vec<_>>();
        if !self.vertices.get(&edge.data.out_v()).copied().unwrap_or_default() {
            return Ok(())
        }

        let label: &'static str = EdgeInfoDiscriminants::from(&edge.data).into();
        writeln!(&mut self.writer)?;
        for in_v in in_vs {
            writeln!(
                &mut self.writer,
                r#"node{} -> node{} [label="{}"];"#,
                edge.data.out_v().id,
                in_v.id,
                label
            )?;
        }
        for ln in format!("{:#?}", &edge).lines() {
            writeln!(&mut self.writer, "// {ln}")?;
        }

        Ok(())
    }
}
