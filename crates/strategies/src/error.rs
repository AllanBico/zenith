use thiserror::Error;

#[derive(Error, Debug)]
pub enum StrategyError {
    #[error("Strategy received invalid parameters: {0}")]
    InvalidParameters(String),

    #[error("An error occurred during indicator calculation: {0}")]
    IndicatorError(String),

    #[error("Strategy of type '{0}' not found or implemented")]
    StrategyNotFound(String),
}