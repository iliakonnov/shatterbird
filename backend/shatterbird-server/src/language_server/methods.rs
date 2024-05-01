use eyre::eyre;
use std::sync::Arc;

use lsp_types::{
    Hover, HoverContents, HoverProviderCapability, InitializeResult, MarkedString, OneOf, Range,
    ServerCapabilities, ServerInfo,
};
use tracing::{info, instrument};

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
    Ok(Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(format!("{:#?}", found))),
        range: Some(Range {
            start: req.text_document_position_params.position,
            end: req.text_document_position_params.position,
        }),
    }))
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
    info!("{:#?}", found);
    Err(LspError::Internal(eyre!("TODO")))
}
