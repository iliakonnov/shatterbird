use std::future::Future;
use std::sync::Arc;

use axum::routing::MethodRouter;
use axum::{Json, Router};
use futures::FutureExt;
use lsp_types::lsp_request;

use crate::language_server::error::LspError;
use crate::state::{AppState, ServerState};

mod error;
mod methods;
mod util;
mod go_to_definition;

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
        "initialize" -> methods::initialize,
        "textDocument/hover" -> methods::hover_range,
        "textDocument/definition" -> methods::go_to_definition,
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
