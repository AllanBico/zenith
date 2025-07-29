pub mod enums;
pub mod error;
pub mod structs;

// Re-export the core types to provide a clean public API.
pub use enums::{OrderSide, OrderType};
pub use error::CoreError;
pub use structs::{Execution, Kline, OrderRequest, Position, Signal, Trade};