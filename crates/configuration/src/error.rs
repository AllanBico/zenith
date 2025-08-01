use std::path::PathBuf;
use thiserror::Error;

/// Represents all possible errors that can occur when loading or validating configuration.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Occurs when the configuration file cannot be found at the specified path.
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),

    /// Wraps errors from the `config` crate when loading or parsing the configuration.
    #[error("Failed to load configuration: {0}")]
    LoadError(#[from] config::ConfigError),

    /// Occurs when configuration values fail validation.
    #[error("Configuration validation error: {0}")]
    ValidationError(String),
}

impl ConfigError {
    /// Creates a new validation error with a formatted message.
    pub fn validation<S: Into<String>>(msg: S) -> Self {
        Self::ValidationError(msg.into())
    }
}