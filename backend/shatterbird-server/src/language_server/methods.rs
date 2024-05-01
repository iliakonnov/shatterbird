use crate::language_server::error::LspError;
use crate::language_server::util;
use crate::state::ServerState;
use lsp_types::{
    Hover, HoverContents, HoverProviderCapability, InitializeResult, MarkedString, Range,
    ServerCapabilities, ServerInfo,
};
use std::sync::Arc;
use tracing::instrument;

#[instrument(skip(state), err)]
pub async fn initialize(
    state: Arc<ServerState>,
    req: lsp_types::InitializeParams,
) -> Result<InitializeResult, LspError> {
    Ok(InitializeResult {
        capabilities: ServerCapabilities {
            hover_provider: Some(HoverProviderCapability::Simple(true)),
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
    let position =
        util::resolve_position(&state.storage, &req.text_document_position_params).await?;
    Ok(Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(format!("{:#?}", position.ranges))),
        range: Some(Range {
            start: req.text_document_position_params.position,
            end: req.text_document_position_params.position,
        }),
    }))
}
