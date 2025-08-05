//! # Zenith Executor Crate
//!
//! This crate provides the core components for trade execution and portfolio state
//! management. It defines a generic `Executor` trait and provides a `SimulatedExecutor`
//! for backtesting, as well as a `Portfolio` manager to track the state of a trading account.
//!
//! ## Architectural Principles
//!
//! - **State vs. Logic Decoupling:** The `Executor` trait is designed to be a pure
//!   calculator that determines the effects of a trade (like fees and slippage) without
//!   mutating state. The `Portfolio` struct is the state machine that applies the results
//!   of an execution to the account balance and positions. This separation is key for
//!   testability and clarity.
//! - **Execution Abstraction:** The `Executor` trait allows higher-level components like
//!   the `Backtester` and the live `Engine` to be completely agnostic about whether they
//!   are executing trades against a simulation or a live exchange.
//!
//! ## Public API
//!
//! - `Executor`: The core trait for all execution engines.
//! - `SimulatedExecutor`: The "virtual exchange" for backtesting.
//! - `Portfolio`: The in-memory state manager for a trading account.
//! - `ExecutorError`: The specific error types that can be returned from this crate.

// Declare the modules that constitute this crate.
pub mod error;
pub mod exchange;
pub mod portfolio;

// Re-export the key components to provide a clean, public-facing API.
pub use error::ExecutorError;
pub use exchange::{Executor, LiveExecutor, SimulatedExecutor};
pub use portfolio::Portfolio;