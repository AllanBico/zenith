use api_client::{BookTickerUpdate, MarkPriceUpdate};
use core_types::Kline;
use rust_decimal::Decimal;

/// A complete, real-time snapshot of the market for a single symbol.
/// The engine will maintain one of these structs for each active bot.
#[derive(Debug, Clone, Default)]
pub struct MarketState {
    pub last_kline: Option<Kline>,
    pub mark_price: Option<Decimal>,
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
}

/// A unified enum that represents any possible real-time event the engine can receive.
/// This is the primary input to the engine's main `select!` loop.
#[derive(Debug, Clone)]
pub enum LiveEvent {
    Kline((String, Kline)),
    BookTicker(BookTickerUpdate),
    MarkPrice(MarkPriceUpdate),
}