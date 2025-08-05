use crate::error::StrategyError;
use crate::Strategy;
use configuration::MACrossoverParams;
use core_types::{Kline, OrderRequest, OrderSide, OrderType, Signal};
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use ta::indicators::SimpleMovingAverage as Sma;
use ta::Next;
use uuid::Uuid;

/// The Triple Moving Average Crossover strategy.
pub struct MACrossover {
    symbol: String,
    ma_fast: Sma,
    ma_slow: Sma,
    trend_filter: Sma,
    // State: The previous values of the fast and slow MAs to detect a crossover event.
    prev_fast_ma: Option<Decimal>,
    prev_slow_ma: Option<Decimal>,
}

impl MACrossover {
    /// Creates a new `MACrossover` instance with the given parameters.
    ///
    /// It performs validation to ensure the parameters are logical.
    pub fn new(params: MACrossoverParams, symbol: String) -> Result<Self, StrategyError> {
        // Validation: Ensure periods are logical.
        if params.ma_fast_period >= params.ma_slow_period {
            return Err(StrategyError::InvalidParameters(
                "Fast MA period must be less than Slow MA period".to_string(),
            ));
        }

        Ok(Self {
            symbol,
            ma_fast: Sma::new(params.ma_fast_period).unwrap(),
            ma_slow: Sma::new(params.ma_slow_period).unwrap(),
            trend_filter: Sma::new(params.trend_filter_period).unwrap(),
            prev_fast_ma: None,
            prev_slow_ma: None,
        })
    }
}

impl Strategy for MACrossover {
    /// Evaluates the triple MA strategy.
    ///
    /// A buy signal is generated when the fast MA crosses above the slow MA,
    /// AND the closing price is above the long-term trend filter MA.
    ///
    /// A sell signal is generated when the fast MA crosses below the slow MA,
    /// AND the closing price is below the long-term trend filter MA.
    fn evaluate(&mut self, kline: &Kline) -> Result<Option<Signal>, StrategyError> {
        tracing::debug!("MACrossover: Evaluating kline for symbol {}: {:?}", self.symbol, kline);
        
        // The `ta` crate uses `f64`. We must convert from our high-precision `Decimal`.
        // This is a controlled and accepted precision trade-off for using the library.
        let close_f64 = kline.close.to_f64().unwrap();

        // Calculate the current values for all three moving averages.
        let current_fast_ma = Decimal::from_f64(self.ma_fast.next(close_f64)).unwrap();
        let current_slow_ma = Decimal::from_f64(self.ma_slow.next(close_f64)).unwrap();
        let trend_filter_ma = Decimal::from_f64(self.trend_filter.next(close_f64)).unwrap();
        
        tracing::debug!("MACrossover: MAs - Fast: {}, Slow: {}, Trend: {}", current_fast_ma, current_slow_ma, trend_filter_ma);

        let mut signal = None;

        // Ensure we have previous MA values to detect a crossover.
        // This implicitly handles the warm-up period for the indicators.
        if let (Some(prev_fast), Some(prev_slow)) = (self.prev_fast_ma, self.prev_slow_ma) {
            tracing::debug!("MACrossover: Previous MAs - Fast: {}, Slow: {}", prev_fast, prev_slow);
            
            // ---===[ Crossover and Filter Logic ]===---

            // Bullish Crossover Check (Fast crosses Above Slow)
            let is_bullish_cross = prev_fast <= prev_slow && current_fast_ma > current_slow_ma;
            // Trend Filter Check
            let is_uptrend = kline.close > trend_filter_ma;

            // Bearish Crossover Check (Fast crosses Below Slow)
            let is_bearish_cross = prev_fast >= prev_slow && current_fast_ma < current_slow_ma;
            // Trend Filter Check
            let is_downtrend = kline.close < trend_filter_ma;
            
            tracing::debug!("MACrossover: Checks - Bullish cross: {}, Uptrend: {}, Bearish cross: {}, Downtrend: {}", 
                           is_bullish_cross, is_uptrend, is_bearish_cross, is_downtrend);
            
            if is_bullish_cross && is_uptrend {
                tracing::debug!("MACrossover: Generating BUY signal");
                signal = Some(Signal {
                    signal_id: Uuid::new_v4(),
                    timestamp: kline.close_time,
                    confidence: dec!(1.0), // Full confidence on clear signal
                    order_request: OrderRequest {
                        client_order_id: Uuid::new_v4(),
                        symbol: self.symbol.clone(),
                        side: OrderSide::Buy,
                        order_type: OrderType::Market,
                        quantity: Decimal::ZERO, // Let the risk manager determine the size
                        price: None,
                        position_side: None, // Will be set by engine
                    },
                });
            } else if is_bearish_cross && is_downtrend {
                tracing::debug!("MACrossover: Generating SELL signal");
                signal = Some(Signal {
                    signal_id: Uuid::new_v4(),
                    timestamp: kline.close_time,
                    confidence: dec!(1.0), // Full confidence on clear signal
                    order_request: OrderRequest {
                        client_order_id: Uuid::new_v4(),
                        symbol: self.symbol.clone(),
                        side: OrderSide::Sell,
                        order_type: OrderType::Market,
                        quantity: Decimal::ZERO, // Let the risk manager determine the size
                        price: None,
                        position_side: None, // Will be set by engine
                    },
                });
            }
        }

        // Update state for the next evaluation.
        self.prev_fast_ma = Some(current_fast_ma);
        self.prev_slow_ma = Some(current_slow_ma);

        if signal.is_some() {
            tracing::debug!("MACrossover: Returning signal: {:?}", signal);
        } else {
            tracing::debug!("MACrossover: No signal generated");
        }

        Ok(signal)
    }
}