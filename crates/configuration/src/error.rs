use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load configuration from file: {0}")]
    LoadError(#[from] config::ConfigError),

    #[error("Configuration validation error: {0}")]
    ValidationError(String),
}