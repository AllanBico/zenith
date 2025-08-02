//! # Zenith Database Crate
//!
//! This crate acts as a high-level, application-specific interface to the
//! PostgreSQL database. It is the system's "permanent archive."
//!
//! ## Architectural Principles
//!
//! - **Layer 3 Adapter:** This crate is an adapter that encapsulates all database-specific
//!   logic. It provides a clean, abstract API to the rest of the application, hiding
//!   the underlying SQL and database implementation details.
//! - **Compile-Time Safety:** Uses `sqlx` to check all SQL queries against the live
//!   database schema at compile time, preventing a large class of runtime errors.
//! - **Asynchronous & Pooled:** All operations are asynchronous, and it uses a
//!   connection pool (`PgPool`) for high-performance, concurrent database access.
//!
//! ## Public API
//!
//! - `connect`: The async function to establish the database connection pool.
//! - `run_migrations`: A utility to apply database migrations, ensuring the schema is up-to-date.
//! - `DbRepository`: The main struct that holds the connection pool and provides all
//!   the high-level data access methods (e.g., `save_performance_report`).
//! - `DbError`: The specific error types that can be returned from this crate.

// Declare the modules that constitute this crate.
pub mod connection;
pub mod error;
pub mod repository;

// Re-export the key components to create a clean, public-facing API.
pub use connection::{connect, run_migrations};
pub use error::DbError;
pub use repository::{DbBacktestRun, DbRepository, FullReport};