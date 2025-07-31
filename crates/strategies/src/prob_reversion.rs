use crate::error::StrategyError;
use crate::Strategy;
use configuration::ProbReversionParams;
use core_types::{Kline, OrderRequest, OrderSide, OrderType, Signal};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use ta::indicators::{
    AverageDirectionalIndex as Adx, BollingerBands, BollingerBandsOutput,
    RelativeStrengthIndex as Rsi,
};
use ta::{Next, NextFrom, Period};
use uuid::Uuid;

/// The Probabilistic Mean Reversion strategy.
///
/// This strategy identifies high-probability reversal points by requiring a confluence
/// of three conditions:
/// 1. Volatility: Price has exceeded the Bollinger Bands.
/// 2. Momentum: RSI is in an overbought/oversold state.
/// 3. Market Regime: ADX is low, indicating a ranging (non-trending) market.
pub struct ProbReversion {
    params: ProbReversionParams,
    bb: BollingerBands,
    rsi: Rsi,
    adx: Adx,
}

impl ProbReversion {
    /// Creates a new `ProbReversion` instance.
    pub fn new(params: ProbReversionParams) -> Result<Self, StrategyError> {
        if params.bb_period == 0 || params.rsi_period == 0 || params.adx_period == 0 {
            return Err(StrategyError::InvalidParameters(
                "Indicator periods cannot be zero".to_string(),
            ));
        }

        Ok(Self {
            bb: BollingerBands::new(
                params.bb_period,
                params.bb_std_dev.to_f64().unwrap(),
            )
            .unwrap(),
            rsi: Rsi::new(params.rsi_period).unwrap(),
            adx: Adx::new(params.adx_period).unwrap(),
            params,
        })
    }
}

impl Strategy for ProbReversion {
    fn evaluate(&mut self, kline: &Kline) -> Result<Option<Signal>, StrategyError> {
        // ---===[ 1. Indicator Calculation ]===---
        // Convert to f64 for `ta` crate compatibility.
        let close_f64 = kline.close.to_f64().unwrap();
        let high_f64 = kline.high.to_f64().unwrap();
        let low_f64 = kline.low.to_f64().unwrap();
        let adx_input = ta::DataItem::builder()
            .high(high_f64)
            .low(low_f64)
            .close(close_f64)
            .build()
            .unwrap();
        
        // Calculate all indicator values for the current kline.
        let bb_output: BollingerBandsOutput = self.bb.next(close_f64);
        let rsi_val = Decimal::from_f64(self.rsi.next(close_f64)).unwrap();
        let adx_val = Decimal::from_f64(self.adx.next(&adx_input).adx).unwrap();

        let bb_upper = Decimal::from_f64(bb_output.upper).unwrap();
        let bb_lower = Decimal::from_f64(bb_output.lower).unwrap();
        
        let mut signal = None;

        // ---===[ 2. Condition Confluence Check ]===---
        
        // Regime Filter: Is the market ranging? (Low ADX)
        let is_ranging = adx_val < self.params.adx_threshold;

        if is_ranging {
            // Overbought Check (Price too high, expect reversal down)
            let is_overbought = kline.close >= bb_upper && rsi_val > self.params.rsi_overbought;
            
            // Oversold Check (Price too low, expect reversal up)
            let is_oversold = kline.close <= bb_lower && rsi_val < self.params.rsi_oversold;

            // ---===[ 3. Signal Generation ]===---
            if is_oversold {
                // All three conditions met for a BUY signal.
                signal = Some(Signal {
                    signal_id: Uuid::new_v4(),
                    timestamp: kline.close_time,
                    confidence: dec!(1.0),
                    order_request: OrderRequest {
                        client_order_id: Uuid::new_v4(),
                        symbol: "placeholder".to_string(),
                        side: OrderSide::Buy,
                        order_type: OrderType::Market,
                        quantity: dec!(1.0),
                        price: None,
                    },
                });
            } else if is_overbought {
                // All three conditions met for a SELL signal.
                signal = Some(Signal {
                    signal_id: Uuid::new_v4(),
                    timestamp: kline.close_time,
                    confidence: dec!(1.0),
                    order_request: OrderRequest {
                        client_order_id: Uuid::new_v4(),
                        symbol: "placeholder".to_string(),
                        side: OrderSide::Sell,
                        order_type: OrderType::Market,
                        quantity: dec!(1.0),
                        price: None,
                    },
                });
            }
        }
        
        Ok(signal)
    }
}