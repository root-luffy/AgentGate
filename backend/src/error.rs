use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden: tool not permitted")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("rate limit exceeded")]
    RateLimited,
    #[error("bad gateway")]
    BadGateway,
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::Unauthorized  => (StatusCode::UNAUTHORIZED,          "unauthorized"),
            AppError::Forbidden     => (StatusCode::FORBIDDEN,             "tool_not_permitted"),
            AppError::NotFound      => (StatusCode::NOT_FOUND,             "not_found"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST,           "bad_request"),
            AppError::RateLimited   => (StatusCode::TOO_MANY_REQUESTS,     "rate_limit_exceeded"),
            AppError::BadGateway    => (StatusCode::BAD_GATEWAY,           "bad_gateway"),
            AppError::Internal(_)   => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            AppError::Database(_)   => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        let message = self.to_string();
        (status, Json(json!({ "error": code, "message": message }))).into_response()
    }
}
