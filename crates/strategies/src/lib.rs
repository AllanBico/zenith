//! # Zenith Strategy Library
//!
//! This crate contains the core trading logic for the Zenith system. It defines a
//! universal `Strategy` trait and provides several concrete implementations.
//!
//! ## Architectural Principles
//!
//! - **Layer 1 Logic:** This is a pure logic crate. It has no knowledge of databases,
//!   APIs, or execution. It depends only on `core-types` and `configuration`.
//! - **Strategy Agnostic Engine:** By using the `Strategy` trait, higher-level crates
//!   like the `backtester` and `engine` can operate on any strategy without knowing its
//!   internal details.
//! - **Extensibility:** Adding a new strategy involves creating a new module, implementing
//!   the `Strategy` trait, and adding it to the `StrategyId` enum and `factory`.
//!
//! ## Public API
//!
//! The primary public components are:
//! - `Strategy`: The core trait all strategies implement.
//! - `StrategyId`: A simple enum to identify which strategy to create.
//! - `create_strategy`: The factory function to construct a strategy instance.
//! - The concrete strategy structs themselves (e.g., `MACrossover`).

// Declare all the modules that constitute this crate.
pub mod error;
pub mod factory;
pub mod funding_rate_arb;
pub mod ma_crossover;
pub mod prob_reversion;
pub mod super_trend;
pub mod ml_strategy;
// Re-export the key components to create a clean, public-facing API.
pub use error::StrategyError;
pub use factory::create_strategy;
pub use funding_rate_arb::FundingRateArb;
pub use ma_crossover::MACrossover;
pub use prob_reversion::ProbReversion;
pub use super_trend::SuperTrend;

// Re-export StrategyId from core_types
pub use core_types::enums::StrategyId;

use core_types::{Kline, Signal};

/// The core trait that all trading strategies must implement.
///
/// This trait defines a common interface for the backtester and live trading engine,
/// allowing them to be strategy-agnostic.
///
/// The `&mut self` in `evaluate` is crucial, as most strategies need to maintain
/// their own internal state (e.g., the previous values of an indicator).
/// The `Send + Sync` bounds are required to allow strategies to be used across
/// multiple threads in the parallel optimizer.
pub trait Strategy: Send + Sync {
    /// Evaluates the strategy based on a new Kline bar.
    ///
    /// # Arguments
    ///
    /// * `kline` - A reference to the latest market data (`Kline`).
    ///
    /// # Returns
    ///
    /// * `Ok(Some(Signal))` - if the strategy's conditions are met to generate a trade signal.
    /// * `Ok(None)` - if the strategy's conditions are not met, and no action should be taken.
    /// * `Err(StrategyError)` - if an error occurs during evaluation.
    fn evaluate(&mut self, kline: &Kline) -> Result<Option<Signal>, StrategyError>;
}