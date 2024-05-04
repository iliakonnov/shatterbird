use eyre::{eyre, OptionExt, Report};
use futures::join;
use std::collections::HashMap;
use std::sync::Arc;

use lsp_types::{
    Hover, HoverContents, HoverProviderCapability, InitializeResult, LanguageString, MarkedString,
    MarkupContent, OneOf, Position, Range, ServerCapabilities, ServerInfo,
};
use mongodb::bson::doc;
use shatterbird_storage::model::lang::{EdgeInfoDiscriminants, VertexInfo};
use shatterbird_storage::model::{Commit, Edge, FileContent, Node, Vertex};
use tracing::{debug, info, instrument};
use url::Url;
use shatterbird_storage::util;

use crate::language_server::error::LspError;
use crate::language_server::go_to_definition;
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
    let found = util::graph::find(
        &state.storage,
        Some(EdgeInfoDiscriminants::Hover),
        &req.text_document_position_params,
        false,
    )
    .await?;
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
pub async fn hover_range(
    state: Arc<ServerState>,
    req: lsp_types::HoverParams,
) -> Result<Option<Hover>, LspError> {
    let found = util::graph::find(
        &state.storage,
        None,
        &req.text_document_position_params,
        false,
    )
    .await?;
    let text = format!(
        "```\nranges: {:?}\nline: {:?}\n```\n",
        found.ranges, found.line
    );
    let range = match found.ranges.first() {
        Some(&x) => state.storage.get(x).await?,
        None => None,
    };
    let line_no = match &range {
        Some(x) => Some(util::graph::find_line_no(&state.storage, x).await?),
        None => None,
    };
    Ok(Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value: text,
        }),
        range: range.zip(line_no).map(|(range, line_no)| Range {
            start: Position::new(line_no as _, range.start),
            end: Position::new(line_no as _, range.end),
        }),
    }))
}

#[instrument(skip(state), err)]
pub async fn go_to_definition(
    state: Arc<ServerState>,
    req: lsp_types::GotoDefinitionParams,
) -> Result<Option<lsp_types::GotoDefinitionResponse>, LspError> {
    let result = go_to_definition::find(&state.storage, &req).await?;
    Ok(result)
}
