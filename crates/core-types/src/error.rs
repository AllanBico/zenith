use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Invalid input for {0}: {1}")]
    InvalidInput(String, String),

    #[error("Calculation error: {0}")]
    Calculation(String),
}