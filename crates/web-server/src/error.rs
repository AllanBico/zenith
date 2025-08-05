use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use tracing;
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] database::DbError),
    #[error("Analyzer error: {0}")]
    Analyzer(#[from] analyzer::error::AnalyzerError),
    #[error("Configuration error: {0}")]
    Config(#[from] configuration::error::ConfigError),
    #[error("Not found: {0}")]
    NotFound(String),
}

/// Converts our custom `AppError` into an HTTP response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Database(db_err) => {
                tracing::error!(error = ?db_err, "Database error.");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal database error occurred".to_string(),
                )
            }
            AppError::Analyzer(analyzer_err) => {
                tracing::error!(error = ?analyzer_err, "Analyzer error.");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An error occurred during analysis".to_string(),
                )
            }
            AppError::Config(config_err) => {
                tracing::error!(error = ?config_err, "Configuration error.");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "A server configuration error occurred".to_string(),
                )
            }
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, message),
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}