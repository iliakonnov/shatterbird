use crate::language_server::error::LspError;
use crate::language_server::util;
use eyre::{eyre, OptionExt, Report};
use futures::join;
use lsp_types::{Position, Range};
use mongodb::bson::doc;
use shatterbird_storage::model::lang::{EdgeInfoDiscriminants, VertexInfo};
use shatterbird_storage::model::{Commit, Edge, FileContent, Node, Vertex};
use shatterbird_storage::Storage;
use std::collections::HashMap;

pub async fn find(
    storage: &Storage,
    req: &lsp_types::GotoDefinitionParams,
) -> Result<Option<lsp_types::GotoDefinitionResponse>, LspError> {
    // range --(Definition)-> DefinitionResult
    let found = util::find(
        storage,
        Some(EdgeInfoDiscriminants::Item),
        &req.text_document_position_params,
        false,
    )
    .await?;
    let results = found
        .found
        .into_iter()
        .filter_map(|v| match v.data {
            VertexInfo::DefinitionResult {} => Some(v.id),
            _ => None,
        })
        .collect::<Vec<_>>();

    // DefinitionResult <--(Item)-- Range
    let items = storage
        .find::<Edge>(
            doc! {
                "data.out_v": { "$in": results },
                "data.edge": { "$eq": "Item" }
            },
            None,
        )
        .await?
        .into_iter()
        .flat_map(|e| e.data.in_vs().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let definitions = storage
        .find::<Vertex>(doc! { "_id": { "$in": items} }, None)
        .await?;

    let mut locations = Vec::new();
    for def in definitions {
        let def = match def.data {
            VertexInfo::Range { range, .. } => range,
            _ => {
                return Err(LspError::Internal(eyre!(
                    "expected range, found {:#?}",
                    def.data
                )))
            }
        };
        let range = storage
            .get(def)
            .await?
            .ok_or_eyre(eyre!("range {} referenced, but not found", def))?;
        let path = async {
            let nodes = storage
                .find::<Node>(doc! { "_id": { "$in": &range.path }}, None)
                .await?
                .into_iter()
                .map(|i| (i.id, i))
                .collect::<HashMap<_, _>>();
            let mut path = Vec::new();

            let root = range.path[0];
            let commit = storage
                .find_one::<Commit>(doc! { "root": root }, None)
                .await?
                .ok_or_eyre(eyre!("no commit for root {} is found", root))?;
            let commit_oid = commit.oid.to_hex().to_string();

            path.push(&commit_oid[..]);
            for pair in range.path.windows(2) {
                let (parent, curr) = match pair {
                    &[parent, curr] => (parent, curr),
                    _ => unreachable!(),
                };
                let parent = nodes
                    .get(&parent)
                    .ok_or_eyre(eyre!("node {} not found in database", parent))?;
                let children = match &parent.content {
                    FileContent::Directory { children, .. } => children,
                    _ => return Err(eyre::eyre!("node {:?} is not a directory", parent.id)),
                };
                let name = children
                    .iter()
                    .find(|(k, v)| **v == curr)
                    .map(|(k, _)| k)
                    .ok_or_eyre(eyre!("node {} not found in {}", curr, parent.id))?;
                path.push(&name[..])
            }
            format!("bird:///{}", path.join("/"))
                .parse()
                .map_err(Report::new)
        };
        let line_no = async {
            let line = storage
                .get(range.line_id)
                .await?
                .ok_or_eyre(eyre!("line {} referenced, but not found", range.line_id))?;
            let doc = storage
                .find_one::<Node>(
                    doc! {"content.Text.lines": { "$elemMatch": { "$eq": line.id }} },
                    None,
                )
                .await?
                .ok_or_eyre(eyre!("can't find file containing line {}", line.id))?;
            let line_no = match doc.content {
                FileContent::Text { lines, .. } => {
                    lines.iter().position(|&x| x == line.id).unwrap()
                }
                _ => {
                    return Err(LspError::Internal(eyre!(
                        "expected text file, found {:?}",
                        doc.content
                    )))
                }
            };
            Ok(line_no)
        };
        let (path, line_no) = join!(path, line_no);
        let (path, line_no) = (path?, line_no?);
        locations.push(lsp_types::Location {
            uri: path,
            range: Range {
                start: Position::new(line_no as _, range.start),
                end: Position::new(line_no as _, range.end),
            },
        })
    }
    Ok(Some(lsp_types::GotoDefinitionResponse::Array(locations)))
}
