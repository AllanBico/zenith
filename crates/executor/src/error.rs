use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Not enough cash available to execute trade. Required: {required}, Available: {available}")]
    InsufficientCash { required: String, available: String },

    #[error("Position not found for symbol: {0}")]
    PositionNotFound(String),

    #[error("Invalid order quantity for closing position. Requested: {requested}, Available: {available}")]
    InvalidClosingQuantity { requested: String, available: String },
    
    #[error("An unexpected portfolio state was encountered: {0}")]
    PortfolioError(String),

    #[error("API error: {0}")]
    Api(String),
}