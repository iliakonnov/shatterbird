use crate::App;
use bson::doc;
use clap::Args;
use eyre::{eyre, OptionExt};
use futures::FutureExt;
use graphviz_rust::cmd::{Format, Layout};
use graphviz_rust::printer::PrinterContext;
use image::Rgba;
use shatterbird_storage::model::lang::{
    EdgeInfoDiscriminants, VertexInfo, VertexInfoDiscriminants,
};
use shatterbird_storage::model::{Edge, Range, Vertex};
use shatterbird_storage::{util, Id};
use std::collections::{HashMap, HashSet};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use tracing::instrument;

#[derive(Args)]
pub struct Graph {
    #[arg(long, required = true)]
    ranges: Vec<Id<Range>>,

    #[clap(long)]
    dot: Option<PathBuf>,

    #[clap(long)]
    svg: Option<PathBuf>,

    #[clap(long, short, action)]
    show: bool,
}

struct State<W> {
    vertices: HashMap<Id<Vertex>, bool>,
    edges: HashSet<Id<Edge>>,
    writer: W,
}

impl Graph {
    pub async fn run(self, app: App) -> eyre::Result<()> {
        let ranges = app
            .storage
            .find::<Range>(doc! { "_id": {"$in": &self.ranges }}, None)
            .await?;
        let initital = app
            .storage
            .find::<Vertex>(
                doc! {
                    "data.vertex": { "$eq": "Range" },
                    "data.range": { "$in": ranges.iter().map(|r| r.id).collect::<Vec<_>>() },
                },
                None,
            )
            .await?;

        let mut output = Vec::new();

        {
            let mut state = State {
                vertices: HashMap::new(),
                edges: HashSet::new(),
                writer: BufWriter::new(&mut output),
            };

            writeln!(&mut state.writer, "strict digraph {{")?;
            writeln!(
                &mut state.writer,
                "overlap = scale; splines = true; sep = 1;"
            )?;
            for i in initital {
                state.visit_vertex(&app.storage, i.id, true).await?;
            }
            writeln!(&mut state.writer, "}}")?;
        }

        if let Some(dot_out) = self.dot {
            std::fs::write(dot_out, &output)?;
        }

        let svg = if self.svg.is_some() || self.show {
            let text = std::str::from_utf8(&output).expect("graph must be valid UTF-8");
            let graph =
                graphviz_rust::parse(text).map_err(|e| eyre!("failed to parse graph: {}", e))?;
            Some(graphviz_rust::exec(
                graph,
                &mut PrinterContext::default(),
                vec![Layout::Neato.into(), Format::Svg.into()],
            )?)
        } else {
            None
        };

        if let Some(svg_out) = self.svg {
            let svg = svg.as_ref().expect("graph must have been already rendered");
            std::fs::write(svg_out, &svg[..])?;
        }

        if self.show {
            let svg = svg.as_ref().expect("graph must have been already rendered");

            let options = resvg::usvg::Options {
                ..Default::default()
            };
            let mut fonts = resvg::usvg::fontdb::Database::new();
            fonts.load_system_fonts();
            let tree = resvg::usvg::Tree::from_str(&svg[..], &options, &fonts)?;
            let size = tree.size();
            let (width, height) = (size.width() as _, size.height() as _);

            let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::default(),
                &mut pixmap.as_mut(),
            );

            let image = image::RgbaImage::from_raw(width, height, pixmap.data().to_vec())
                .ok_or_eyre(eyre!("failed to create image"))?;
            let mut on_white =
                image::RgbaImage::from_fn(width, height, |_, _| Rgba([255, 255, 255, 255]));
            image::imageops::overlay(&mut on_white, &image, 0, 0);
            let image = image::DynamicImage::ImageRgba8(on_white);
            viuer::print(
                &image,
                &viuer::Config {
                    absolute_offset: false,
                    ..Default::default()
                },
            )?;
        }

        Ok(())
    }
}

impl<W: Write> State<W> {
    #[instrument(level = tracing::Level::INFO, skip_all, fields(v=%id))]
    async fn visit_vertex(
        &mut self,
        storage: &shatterbird_storage::Storage,
        id: Id<Vertex>,
        root: bool,
    ) -> eyre::Result<()> {
        if self.vertices.contains_key(&id) {
            return Ok(());
        }

        let vertex = storage
            .get(id)
            .await?
            .ok_or_eyre(eyre!("vertex {} not found", id))?;

        if !root {
            if let VertexInfo::Document(_)
            | VertexInfo::PackageInformation(_)
            | VertexInfo::Moniker(_) = vertex.data
            {
                self.vertices.insert(id, false);
                return Ok(());
            }
        }
        self.vertices.insert(id, true);

        let mut xlabel = None;
        let mut tooltip = None;

        if let VertexInfo::Range { range, .. } = &vertex.data {
            let range = storage
                .get(*range)
                .await?
                .ok_or_eyre(eyre!("{range} not found"))?;
            let line_no = 1 + util::graph::find_line_no(storage, &range).await?;
            let filename = util::graph::find_file_path(storage, &range).await?;
            let filename = &filename[1..].join("/");
            let line = storage
                .get(range.line_id)
                .await?
                .ok_or_eyre(eyre!("line {} not found", range.line_id))?;
            let start = range.start.min(line.text.len() as _) as _;
            let end = range.end.min(line.text.len() as _) as _;
            let span = line.text[start..end].to_string();
            xlabel = Some(format!("«{span}»@L{line_no}"));
            tooltip = Some(format!(
                "{filename}:{line_no}:{}-{}",
                range.start, range.end
            ));
        }

        let fillcolor = if root {
            ", fillcolor=yellow, style=filled"
        } else {
            ""
        };
        let xlabel = if let Some(x) = xlabel {
            format!(r#", xlabel="{x}""#)
        } else {
            String::new()
        };
        let tooltip = if let Some(x) = tooltip {
            format!(r#", tooltip="{x}""#)
        } else {
            String::new()
        };

        let label: &'static str = VertexInfoDiscriminants::from(&vertex.data).into();
        writeln!(&mut self.writer)?;
        writeln!(
            &mut self.writer,
            r#"node{} [label="{label}"{fillcolor}{xlabel}{tooltip}];"#,
            vertex.id.id
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
                self.visit_vertex(storage, in_v, false).await?;
            }
        } else {
            // () -> ...
            self.visit_vertex(storage, edge.data.out_v(), false).await?;
        }

        let in_vs = edge
            .data
            .in_vs()
            .filter(|x| self.vertices.get(x).copied().unwrap_or_default())
            .collect::<Vec<_>>();
        if !self
            .vertices
            .get(&edge.data.out_v())
            .copied()
            .unwrap_or_default()
        {
            return Ok(());
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
