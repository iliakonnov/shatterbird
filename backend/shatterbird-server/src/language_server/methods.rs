use std::sync::Arc;

use eyre::eyre;
use futures::future::try_join_all;
use lsp_types::{Hover, HoverContents, Location, MarkupContent, Position, Range};
use tracing::instrument;

use shatterbird_storage::model::lang::{
    EdgeInfoDiscriminants, VertexInfo, VertexInfoDiscriminants,
};
use shatterbird_storage::util;

use crate::language_server::error::LspError;
use crate::state::ServerState;

#[instrument(skip(state), err)]
pub async fn hover(
    state: Arc<ServerState>,
    req: lsp_types::HoverParams,
) -> Result<Option<Hover>, LspError> {
    let found = util::graph::find(
        &state.storage,
        Some(EdgeInfoDiscriminants::Hover),
        &req.text_document_position_params,
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
    let found = util::graph::find(&state.storage, None, &req.text_document_position_params).await?;
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
    let found = util::graph::find(
        &state.storage,
        Some(EdgeInfoDiscriminants::Definition),
        &req.text_document_position_params,
    )
    .await?;
    let results =
        util::graph::filter_vertices(found.found, VertexInfoDiscriminants::DefinitionResult);
    let items = util::graph::find_items(&state.storage, results.map(|x| x.id)).await?;
    let locations = try_join_all(
        items
            .iter()
            .map(|x| util::graph::to_location(&state.storage, x)),
    )
    .await?;
    if locations.is_empty() {
        return Ok(None);
    }
    Ok(Some(lsp_types::GotoDefinitionResponse::Array(locations)))
}

#[instrument(skip(state), err)]
pub async fn references(
    state: Arc<ServerState>,
    req: lsp_types::ReferenceParams,
) -> Result<Option<Vec<Location>>, LspError> {
    let found = util::graph::find(
        &state.storage,
        Some(EdgeInfoDiscriminants::References),
        &req.text_document_position,
    )
    .await?;
    let results =
        util::graph::filter_vertices(found.found, VertexInfoDiscriminants::ReferenceResult);
    let items = util::graph::find_items(&state.storage, results.map(|x| x.id)).await?;
    let locations = try_join_all(
        items
            .iter()
            .map(|x| util::graph::to_location(&state.storage, x)),
    )
    .await?;
    if locations.is_empty() {
        return Ok(None);
    }
    Ok(Some(locations))
}
