use thiserror::Error;

#[derive(Error, Debug)]
pub enum AlerterError {
    #[error("Telegram API request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Telegram API returned an error: {0}")]
    ApiError(String),

    #[error("Alerter is not configured. Missing token or chat_id.")]
    NotConfigured,
}