use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use crate::store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid SemVer version: {0}")]
    InvalidSemVer(String),
    #[error("invalid path segment: {0}")]
    InvalidPathSegment(String),
    #[error("module already published; bump the version to publish a new release")]
    AlreadyExists,
    #[error("module not found")]
    NotFound,
    #[error("missing 'module' field in multipart upload")]
    MissingModuleField,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("internal server error")]
    Internal(#[from] StoreError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::InvalidSemVer(_)
            | AppError::InvalidPathSegment(_)
            | AppError::MissingModuleField
            | AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::AlreadyExists => StatusCode::CONFLICT,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Internal(StoreError::AlreadyExists) => StatusCode::CONFLICT,
            AppError::Internal(StoreError::NotFound) => StatusCode::NOT_FOUND,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
