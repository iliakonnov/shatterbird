use axum::http::StatusCode;
use axum_derive_error::ErrorResponse;
use eyre::Report;
use lsp_types::Url;
use std::error::Error;
use thiserror::Error;

#[derive(Error, ErrorResponse)]
pub enum LspError {
    #[error("method {0} not found")]
    #[status(StatusCode::NOT_FOUND)]
    MethodNotFound(String),

    #[error("file {url} not found{}", message.as_ref().map(|x| format!(": {}", x)).unwrap_or_default())]
    #[status(StatusCode::BAD_REQUEST)]
    FileNotFound { url: Url, message: Option<String> },

    #[error("bad request: {0}")]
    #[status(StatusCode::BAD_REQUEST)]
    BadRequest(Report),

    #[error("internal error: {0}")]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    Internal(
        #[from]
        #[source]
        Report,
    ),
}

impl LspError {
    pub fn not_found<T: ToString>(url: &Url, message: T) -> Self {
        Self::FileNotFound {
            url: url.clone(),
            message: Some(message.to_string()),
        }
    }

    pub fn bad_request<T: Into<Report>>(err: T) -> Self {
        Self::BadRequest(err.into())
    }
}
