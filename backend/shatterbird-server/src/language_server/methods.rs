use eyre::{eyre, OptionExt, Report};
use futures::join;
use std::collections::HashMap;
use std::sync::Arc;

use lsp_types::{
    Hover, HoverContents, HoverProviderCapability, InitializeResult, MarkedString, OneOf, Position,
    Range, ServerCapabilities, ServerInfo,
};
use mongodb::bson::doc;
use shatterbird_storage::model::lang::VertexInfo;
use shatterbird_storage::model::{Commit, Edge, FileContent, Node, Vertex};
use tracing::{debug, info, instrument};
use url::Url;

use crate::language_server::error::LspError;
use crate::language_server::util;
use crate::state::ServerState;

#[instrument(skip(state), err)]
pub async fn initialize(
    state: Arc<ServerState>,
    req: lsp_types::InitializeParams,
) -> Result<InitializeResult, LspError> {
    Ok(InitializeResult {
        capabilities: ServerCapabilities {
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            definition_provider: Some(OneOf::Left(true)),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "shatterbird".to_string(),
            version: None,
        }),
    })
}

#[instrument(skip(state), err)]
pub async fn hover(
    state: Arc<ServerState>,
    req: lsp_types::HoverParams,
) -> Result<Option<Hover>, LspError> {
    let found = util::find(&state.storage, "Hover", &req.text_document_position_params).await?;
    let result = match found.found.into_iter().next() {
        None => return Ok(None),
        Some(x) => match x.data {
            VertexInfo::HoverResult { result } => result,
            _ => {
                return Err(LspError::Internal(eyre!(
                    "expected hover result, found {:#?}",
                    x
                )))
            }
        },
    };
    Ok(Some(result))
}

#[instrument(skip(state), err)]
pub async fn go_to_definition(
    state: Arc<ServerState>,
    req: lsp_types::GotoDefinitionParams,
) -> Result<Option<lsp_types::GotoDefinitionResponse>, LspError> {
    let found = util::find(
        &state.storage,
        "Definition",
        &req.text_document_position_params,
    )
    .await?;
    let result_vertex = match found.found.into_iter().next() {
        None => return Ok(None),
        Some(x) => match x.data {
            VertexInfo::DefinitionResult {} => x.id,
            _ => {
                return Err(LspError::Internal(eyre!(
                    "expected definition result, found {:#?}",
                    x
                )))
            }
        },
    };
    let items = state
        .storage
        .find::<Edge>(
            doc! {
                "data.out_v": { "$eq": result_vertex },
                "data.edge": { "$eq": "Item" }
            },
            None,
        )
        .await?
        .into_iter()
        .flat_map(|e| e.data.in_vs().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let definitions = state
        .storage
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
        let range = state
            .storage
            .get(def)
            .await?
            .ok_or_eyre(eyre!("range {} referenced, but not found", def))?;
        let path = async {
            let nodes = state
                .storage
                .find::<Node>(doc! { "_id": { "$in": &range.path }}, None)
                .await?
                .into_iter()
                .map(|i| (i.id, i))
                .collect::<HashMap<_, _>>();
            let mut path = Vec::new();

            let root = range.path[0];
            let commit = state
                .storage
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
            Ok(format!("bird:///{}", path.join("/"))
                .parse()
                .map_err(Report::new)?)
        };
        let line_no = async {
            let line = state
                .storage
                .get(range.line_id)
                .await?
                .ok_or_eyre(eyre!("line {} referenced, but not found", range.line_id))?;
            let doc = state
                .storage
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
