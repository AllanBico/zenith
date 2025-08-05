use chrono::{DateTime, Utc};
use core_types::{Execution, Position, Kline};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Enum representing the severity of a log message for structured logging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// A structured log message to be sent over WebSocket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogMessage {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
}

/// A complete snapshot of the portfolio's current state.
/// This message provides the frontend with all the data needed to render the main dashboard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortfolioState {
    pub timestamp: DateTime<Utc>,
    pub cash: Decimal,
    pub total_value: Decimal,
    pub positions: Vec<Position>,
}

/// A kline data message containing symbol and kline information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KlineData {
    pub symbol: String,
    pub kline: Kline,
}

/// The top-level WebSocket message enum.
/// All communication from the server to the client will be one of these variants.
///
/// The `#[serde(tag = "type", content = "payload")]` attribute is a powerful `serde` feature.
/// It serializes the enum into a clean JSON object, which is easy for the frontend to handle.
/// For example, a `Log` variant would look like:
/// `{
///   "type": "Log",
///   "payload": {
///     "timestamp": "...",
///     "level": "Info",
///     "message": "..."
///   }
/// }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    /// A structured log message.
    Log(LogMessage),
    /// A full snapshot of the portfolio state.
    PortfolioState(PortfolioState),
    /// A notification that a single trade has been executed.
    TradeExecuted(Execution),
    /// A simple message to confirm to a new client that its WebSocket connection is active.
    Connected,
    /// Real-time kline data for a symbol.
    KlineData(KlineData),
}