use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BacktestError {
    #[error("Database error occurred during backtest: {0}")]
    Database(#[from] database::DbError),

    #[error("Strategy execution error: {0}")]
    Strategy(#[from] strategies::StrategyError),

    #[error("Risk management error: {0}")]
    Risk(#[from] risk::RiskError),

    #[error("Execution simulation error: {0}")]
    Executor(#[from] executor::ExecutorError),
    
    #[error("Analytics calculation error: {0}")]
    Analytics(#[from] analytics::AnalyticsError),

    #[error("Progress bar template error: {0}")]
    ProgressBarTemplate(String),

    #[error("Historical data for the requested range is incomplete or missing.")]
    DataUnavailable,
}

impl From<indicatif::style::TemplateError> for BacktestError {
    fn from(error: indicatif::style::TemplateError) -> Self {
        BacktestError::ProgressBarTemplate(error.to_string())
    }
}