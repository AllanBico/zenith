//! # Zenith Events
//!
//! This crate defines the real-time event structures used for WebSocket communication
//! between the backend engine and the frontend UI.
//!
//! As a Layer 0 crate, it depends only on `core-types` and provides the definitive
//! language for all real-time state synchronization.

// Declare the modules that make up this crate.
pub mod error;
pub mod messages;

// Re-export the core types to provide a clean public API.
pub use error::EventsError;
pub use messages::{LogLevel, LogMessage, PortfolioState, WsMessage, KlineData};