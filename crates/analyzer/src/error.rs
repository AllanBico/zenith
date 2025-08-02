use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalyzerError {
    #[error("Database error: {0}")]
    Database(#[from] database::DbError),

    #[error("No completed backtest runs found for job ID: {0}")]
    NoRunsFound(uuid::Uuid),

    #[error("An internal calculation error occurred: {0}")]
    Calculation(String),
}