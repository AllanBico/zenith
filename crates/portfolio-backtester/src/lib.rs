//! # Zenith Portfolio Backtester
//!
//! This crate provides the engine for running sophisticated, multi-asset,
//! multi-strategy portfolio-level backtests. It uses a "master clock"
//! architecture to process events chronologically across all assets.

pub mod data_handler;
pub mod error;
pub mod manager;

pub use data_handler::{load_and_prepare_data, Event, MarketEvent};
pub use error::PortfolioError;
pub use manager::PortfolioManager;