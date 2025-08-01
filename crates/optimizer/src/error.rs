use indicatif::style::TemplateError;
use risk::RiskError;
use serde_json::Error as JsonError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OptimizerError {
    #[error("Configuration error: {0}")]
    Config(#[from] configuration::error::ConfigError),

    #[error("Database error: {0}")]
    Database(#[from] database::DbError),

    #[error("Backtest execution failed within optimizer: {0}")]
    Backtest(#[from] backtester::error::BacktestError),
    
    #[error("Strategy error during parameter generation: {0}")]
    Strategy(#[from] strategies::StrategyError),

    #[error("Error joining parallel tasks: {0}")]
    JoinError(String),

    #[error("Parameter generation failed: {0}")]
    ParameterGeneration(String),
    
    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] JsonError),
    
    #[error("Risk management error: {0}")]
    Risk(#[from] RiskError),
    
    #[error("Progress bar template error: {0}")]
    ProgressBarTemplate(String),
}

impl From<TemplateError> for OptimizerError {
    fn from(error: TemplateError) -> Self {
        OptimizerError::ProgressBarTemplate(error.to_string())
    }
}