use thiserror::Error;

#[derive(Error, Debug)]
pub enum PortfolioError {
    #[error("Database error: {0}")]
    Database(#[from] database::DbError),

    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Data handler error: {0}")]
    Data(String),
}