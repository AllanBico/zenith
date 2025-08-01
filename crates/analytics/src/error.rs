use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalyticsError {
    #[error("Not enough data to perform calculation: {0}")]
    NotEnoughData(String),

    #[error("Calculation error: Division by zero encountered in metric '{0}'")]
    DivisionByZero(String),

    #[error("An unexpected error occurred during analytics calculation: {0}")]
    InternalError(String),
    
    #[error("Error in calculation: {0}")]
    Calculation(String),
}