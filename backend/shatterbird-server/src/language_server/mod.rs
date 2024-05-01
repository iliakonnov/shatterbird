mod error;

use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;

use crate::language_server::error::LspError;
use axum::routing::MethodRouter;
use axum::{Json, Router};
use eyre::eyre;
use futures::FutureExt;
use lsp_types::{
    lsp_request, Hover, HoverContents, HoverProviderCapability, InitializeResult, MarkedString,
    PositionEncodingKind, Range, ServerCapabilities, ServerInfo, Url,
};
use shatterbird_storage::model::{Commit, FileContent, Line, Node};
use shatterbird_storage::{Id, Storage};
use tracing::instrument;

use crate::state::{AppState, ServerState};

macro_rules! route {
    ($router:expr, $($method:tt -> $handler:expr),* $(,)?) => {
        $router
        $(
            .route(
                concat!("/", $method),
                axum::routing::post(handler_for::<lsp_request![$method], _, _>($handler)),
            )
        )*
    };
}

pub fn router() -> Router<Arc<ServerState>> {
    route!(Router::new(),
        "initialize" -> initialize,
        "textDocument/hover" -> hover,
    )
}

fn handler_for<R, F, Fut>(f: F) -> MethodRouter<Arc<ServerState>>
where
    R: lsp_types::request::Request,
    F: 'static + Clone + Send + Fn(Arc<ServerState>, R::Params) -> Fut,
    Fut: Send + Future<Output = Result<R::Result, LspError>>,
{
    let handler = move |state: AppState, params: Json<R::Params>| {
        f(state.0, params.0).map(|res| res.map(Json))
    };
    axum::routing::post(handler)
}

#[instrument(skip_all, err, fields(uri = %uri))]
async fn resolve_url(storage: &Storage, uri: &Url) -> Result<Node, LspError> {
    let splitted = uri
        .path()
        .split('/')
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();
    if splitted.is_empty() {
        return Err(LspError::not_found(uri, "empty path"));
    }
    let commit = splitted[0]
        .parse()
        .map_err(|x| LspError::bad_request(eyre!("invalid commit: {x}")))?;
    let commit = storage
        .find(Commit::filter().oid(commit))
        .await?
        .ok_or_else(|| LspError::not_found(uri, format!("no such commit: {}", commit)))?;
    let mut curr = commit.root;
    for &component in splitted[1..].iter() {
        let node = match storage.get(curr).await? {
            Some(x) => x,
            None => return Err(LspError::Internal(eyre!("can't find {}", curr))),
        };
        let children = match node.content {
            FileContent::Directory { children } => children,
            _ => {
                return Err(LspError::not_found(
                    uri,
                    format!("{} is not a directory", curr),
                ))?
            }
        };
        curr = match children.get(component) {
            Some(x) => *x,
            None => {
                return Err(LspError::not_found(
                    uri,
                    format!("no child named {} in {}", component, curr),
                ))
            }
        };
    }
    storage
        .get(curr)
        .await?
        .ok_or_else(|| LspError::Internal(eyre!("can't find {}", curr)))
}

#[instrument(skip_all, err, fields(uri = %position.text_document.uri, position = ?position.position))]
async fn resolve_position(
    storage: &Storage,
    position: &lsp_types::TextDocumentPositionParams,
) -> Result<(Line, u32), LspError> {
    let node = resolve_url(&storage, &position.text_document.uri).await?;
    let lines = match node.content {
        FileContent::Text { lines, .. } => lines,
        _ => return Err(LspError::BadRequest(eyre!("not a text file"))),
    };
    let line = lines
        .get(position.position.line as usize)
        .copied()
        .ok_or_else(|| LspError::BadRequest(eyre!("invalid line number")))?;
    let line = storage
        .get(line)
        .await?
        .ok_or_else(|| LspError::Internal(eyre!("can't find {}", line)))?;
    Ok((line, position.position.character))
}

#[instrument(skip(state), err)]
async fn initialize(
    state: Arc<ServerState>,
    req: lsp_types::InitializeParams,
) -> Result<InitializeResult, LspError> {
    Ok(InitializeResult {
        capabilities: ServerCapabilities {
            // position_encoding: Some(PositionEncodingKind::UTF8),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "hello".to_string(),
            version: None,
        }),
    })
}

#[instrument(skip(state), err)]
async fn hover(
    state: Arc<ServerState>,
    req: lsp_types::HoverParams,
) -> Result<Option<Hover>, LspError> {
    let (line, col) = resolve_position(&state.storage, &req.text_document_position_params).await?;
    Ok(Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(format!("{col} at {}", line.id))),
        range: Some(Range {
            start: req.text_document_position_params.position,
            end: req.text_document_position_params.position,
        }),
    }))
}
