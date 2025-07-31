use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RiskError {
    #[error("Risk parameters from configuration are invalid: {0}")]
    InvalidParameters(String),

    #[error("Insufficient portfolio equity ({0}) to execute trade based on risk rules.")]
    InsufficientEquity(Decimal),

    #[error("Calculated stop-loss price is invalid: {0}")]
    InvalidStopLoss(String),

    #[error("The provided entry price ({0}) is zero or negative.")]
    InvalidEntryPrice(Decimal),

    #[error("A calculation error occurred: {0}")]
    Calculation(String),
}