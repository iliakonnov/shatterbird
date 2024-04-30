use std::future::Future;
use std::sync::Arc;

use axum::routing::MethodRouter;
use axum::{Json, Router};
use futures::FutureExt;
use lsp_types::{lsp_request, Hover, HoverProviderCapability, InitializeResult, Range, ServerCapabilities, ServerInfo, HoverContents, MarkedString};

use crate::state::{AppState, ServerState};
use crate::utils::AppError;

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
    Fut: Send + Future<Output = eyre::Result<R::Result>>,
{
    let handler = move |state: AppState, params: Json<R::Params>| {
        f(state.0, params.0).map(|res| res.map(Json).map_err(AppError::from))
    };
    axum::routing::post(handler)
}

async fn initialize(
    state: Arc<ServerState>,
    req: lsp_types::InitializeParams,
) -> eyre::Result<InitializeResult> {
    Ok(InitializeResult {
        capabilities: ServerCapabilities {
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "hello".to_string(),
            version: None,
        }),
    })
}

async fn hover(
    state: Arc<ServerState>,
    req: lsp_types::HoverParams,
) -> eyre::Result<Option<Hover>> {
    Ok(Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String("Hello".to_string())),
        range: Some(Range {
            start: req.text_document_position_params.position,
            end: req.text_document_position_params.position,
        }),
    }))
}
