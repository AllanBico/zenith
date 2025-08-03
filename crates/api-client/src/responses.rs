use core_types::OrderSide;
use rust_decimal::Decimal;
use serde::Deserialize;

// Using `#[serde(rename_all = "camelCase")]` to automatically map from JSON camelCase to Rust snake_case.

/// The response from a successful `POST /fapi/v1/order` request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub client_order_id: String,
    pub cum_qty: Decimal,
    pub cum_quote: Decimal,
    pub executed_qty: Decimal,
    pub order_id: i64,
    pub avg_price: Decimal,
    pub orig_qty: Decimal,
    pub price: Decimal,
    pub reduce_only: bool,
    pub side: OrderSide,
    pub status: String,
    pub stop_price: Decimal,
    pub symbol: String,
    pub time_in_force: String,
    #[serde(rename = "type")]
    pub order_type: String,
    // There are more fields, but these are the most important for us.
}

/// A single asset's balance from `GET /fapi/v2/balance`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceResponse {
    pub account_alias: String,
    pub asset: String,
    pub balance: Decimal,
    pub cross_wallet_balance: Decimal,
    pub cross_un_pnl: Decimal,
    pub available_balance: Decimal,
    pub max_withdraw_amount: Decimal,
}

/// A single open position from `GET /fapi/v2/positionRisk`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionResponse {
    pub entry_price: Decimal,
    pub leverage: String, // Comes as a string, e.g., "10"
    pub max_notional_value: String,
    pub liquidation_price: Decimal,
    pub mark_price: Decimal,
    pub position_amt: Decimal,
    pub symbol: String,
    pub un_realized_profit: Decimal,
}

/// Represents an error response from the Binance API.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorResponse {
    pub code: i16,
    pub msg: String,
}