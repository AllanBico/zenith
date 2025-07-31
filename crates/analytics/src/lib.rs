//! # Zenith Analytics Engine
//!
//! This crate provides the tools for conducting quantitative analysis of trading strategy
//! performance. It acts as the "unbiased judge" of the system.
//!
//! ## Architectural Principles
//!
//! - **Layer 1 Logic:** This is a pure logic crate. It has no knowledge of external systems.
//!   It depends only on `core-types` (Layer 0).
//! - **Stateless Calculation:** The `AnalyticsEngine` is a stateless calculator. It takes
//!   raw trading data as input and produces a `PerformanceReport` as output. This makes
//!   it highly reliable and easy to test.
//!
//! ## Public API
//!
//! - `AnalyticsEngine`: The main struct that contains the calculation logic.
//! - `PerformanceReport`: The standardized struct that holds all 17+ performance metrics.
//! - `AnalyticsError`: The specific error types that can be returned from this crate.

// Declare the modules that constitute this crate.
pub mod engine;
pub mod error;
pub mod report;

// Re-export the key components to create a clean, public-facing API.
pub use engine::AnalyticsEngine;
pub use error::AnalyticsError;
pub use report::PerformanceReport;