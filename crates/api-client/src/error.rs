use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Failed to build the HTTP request: {0}")]
    RequestBuild(#[from] reqwest::Error),

    #[error("The API request returned an error: {0}")]
    ApiError(String),

    #[error("Failed to deserialize the API response: {0}")]
    Deserialization(String),

    #[error("Invalid data format from API: {0}")]
    InvalidData(String),
}