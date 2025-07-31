use crate::error::StrategyError;
use crate::Strategy;
use configuration::FundingRateArbParams;
use core_types::{Kline, Signal};

/// The Funding Rate Arbitrage strategy.
///
/// **ARCHITECTURAL NOTE:** This strategy is a placeholder scaffold.
///
/// Unlike other strategies, funding rate arbitrage does not operate on `Kline` data.
/// It requires real-time access to:
/// 1. The funding rate of a perpetual contract.
/// 2. The mark price of the perpetual contract.
/// 3. The index price (or spot price) of the underlying asset.
///
/// The current `Strategy::evaluate` signature only provides a `&Kline`. The live
/// `Engine` (to be built in a later phase) will need to be enhanced to provide a
/// more complex `MarketData` struct to strategies like this, which require more
/// than just candlestick data.
///
/// For now, this implementation satisfies the `Strategy` trait but will not
/// generate signals. Its purpose is to complete the architectural skeleton.
pub struct FundingRateArb {
    _params: FundingRateArbParams,
}

impl FundingRateArb {
    /// Creates a new `FundingRateArb` instance.
    pub fn new(params: FundingRateArbParams) -> Result<Self, StrategyError> {
        Ok(Self { _params: params })
    }
}

impl Strategy for FundingRateArb {
    /// This evaluation function is a no-op by design.
    ///
    /// It will always return `Ok(None)` because it cannot receive the necessary
    /// funding rate and price data through the current `evaluate` method signature.
    /// The actual logic will be implemented once the live `Engine`'s data routing
    /// capabilities are expanded.
    fn evaluate(&mut self, _kline: &Kline) -> Result<Option<Signal>, StrategyError> {
        // The logic would look something like this in the future:
        //
        // if market_data.funding_rate > self.params.target_rate_threshold {
        //     // Generate a signal to short the perpetual and buy spot.
        // }
        //
        // Since we don't have `market_data`, we do nothing.
        Ok(None)
    }
}