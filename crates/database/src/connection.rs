use crate::error::DbError;
use dotenvy::dotenv;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use std::time::Duration;

/// Establishes a connection pool to the PostgreSQL database.
///
/// This function reads the `DATABASE_URL` from the `.env` file, creates a
/// connection pool with robust settings, and returns it. This pool can be
/// shared across the entire application for high-performance database access.
pub async fn connect() -> Result<PgPool, DbError> {
    // Load environment variables from the .env file.
    dotenv().map_err(|e| DbError::ConnectionConfigError(e.to_string()))?;

    let database_url = env::var("DATABASE_URL")
        .map_err(|_e| DbError::ConnectionConfigError("DATABASE_URL must be set.".to_string()))?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    Ok(pool)
}

/// A utility function to run database migrations automatically.
///
/// This is useful for ensuring the database schema is up-to-date when the application starts,
/// which is especially important in production deployments.
pub async fn run_migrations(pool: &PgPool) -> Result<(), DbError> {
    // Use a relative path from the crate root
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}