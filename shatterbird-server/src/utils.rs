use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::fmt::Debug;
use tracing::error;

pub type AppResult<T, E = eyre::Report> = Result<T, AppError<E>>;

pub struct AppError<E = eyre::Report>(E);

impl<E: Debug> IntoResponse for AppError<E> {
    fn into_response(self) -> Response {
        error!("internal server error: {error:?}", error = self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.").into_response()
    }
}

impl<E: Into<eyre::Report>> From<E> for AppError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

pub struct May404<T>(pub Option<T>);

impl<T: IntoResponse> IntoResponse for May404<T> {
    fn into_response(self) -> Response {
        match self.0 {
            Some(x) => x.into_response(),
            None => (StatusCode::NOT_FOUND, "not found").into_response(),
        }
    }
}
