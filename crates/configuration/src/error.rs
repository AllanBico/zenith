use thiserror::Error;
use serde_json::Error as SerdeJsonError; 

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load configuration from file: {0}")]
    LoadError(#[from] config::ConfigError),

    #[error("Configuration file not found: {0}")]
    FileNotFound(String),

    #[error("Configuration validation error: {0}")]
    ValidationError(String),
    
    #[error("JSON deserialization error: {0}")] 
    JsonError(#[from] SerdeJsonError),
}

impl ConfigError {
    /// Creates a new validation error with a formatted message.
    pub fn validation<S: Into<String>>(msg: S) -> Self {
        Self::ValidationError(msg.into())
    }
}