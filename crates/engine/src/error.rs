use thiserror::Error;

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("API client error: {0}")]
    ApiClient(#[from] api_client::error::ApiError),

    #[error("Database error: {0}")]
    Database(#[from] database::DbError),

    #[error("Strategy error: {0}")]
    Strategy(#[from] strategies::StrategyError),
    
    #[error("Risk management error: {0}")]
    Risk(#[from] risk::RiskError),

    #[error("Portfolio state error: {0}")]
    Portfolio(#[from] executor::ExecutorError),

    #[error("Bot with symbol '{0}' not found in the engine.")]
    BotNotFound(String),

    #[error("Serialization/deserialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
}