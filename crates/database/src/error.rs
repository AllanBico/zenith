use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Failed to load environment variables for database connection: {0}")]
    ConnectionConfigError(String),

    #[error("Failed to connect to the database: {0}")]
    ConnectionError(#[from] sqlx::Error),

    #[error("Database migration failed: {0}")]
    MigrationError(#[from] sqlx::migrate::MigrateError),

    #[error("An error occurred during JSON serialization/deserialization: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("The requested data was not found in the database.")]
    NotFound,
}