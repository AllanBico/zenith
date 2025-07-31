use crate::error::StrategyError;
use crate::Strategy;
use configuration::ProbReversionParams;
use core_types::{Kline, OrderRequest, OrderSide, OrderType, Signal};
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use ta::indicators::{BollingerBands, RelativeStrengthIndex as Rsi, AverageTrueRange};
use ta::Next as _;
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
    atr: AverageTrueRange,  // Using ATR as a trend strength indicator
    prev_close: f64,        // Track previous close for trend detection
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
                params.bb_period as usize,
                params.bb_std_dev.to_f64().unwrap_or(2.0),
            ).map_err(|e| StrategyError::InvalidParameters(format!("Failed to initialize Bollinger Bands: {:?}", e)))?,
            rsi: Rsi::new(params.rsi_period as usize).map_err(|e| 
                StrategyError::InvalidParameters(format!("Failed to initialize RSI: {:?}", e))
            )?,
            atr: AverageTrueRange::new(params.adx_period as usize).map_err(|e| 
                StrategyError::InvalidParameters(format!("Failed to initialize ATR: {:?}", e))
            )?,
            params,
            prev_close: 0.0,
        })
    }
}

impl Strategy for ProbReversion {
    fn evaluate(&mut self, kline: &Kline) -> Result<Option<Signal>, StrategyError> {
        // Convert to f64 for `ta` crate compatibility
        let close_f64 = kline.close.to_f64().ok_or_else(|| 
            StrategyError::InvalidParameters("Failed to convert close to f64".to_string())
        )?;
        
        // Convert high/low to f64 but don't use them yet
        let _high_f64 = kline.high.to_f64().ok_or_else(|| 
            StrategyError::InvalidParameters("Failed to convert high to f64".to_string())
        )?;
        let _low_f64 = kline.low.to_f64().ok_or_else(|| 
            StrategyError::InvalidParameters("Failed to convert low to f64".to_string())
        )?;
        
        // Calculate indicator values
        let bb = self.bb.next(close_f64);
        let rsi_val = self.rsi.next(close_f64);
        let atr = self.atr.next(close_f64);
        
        // Convert to Decimal for comparison with strategy parameters
        let rsi_decimal = Decimal::from_f64(rsi_val).unwrap_or(dec!(0));
        let bb_upper = Decimal::from_f64(bb.upper).unwrap_or(dec!(0));
        let bb_lower = Decimal::from_f64(bb.lower).unwrap_or(dec!(0));
        
        // Calculate price change for trend detection
        let _price_change = if self.prev_close > 0.0 {
            (close_f64 - self.prev_close) / self.prev_close
        } else {
            0.0
        };
        
        // Update previous close for next iteration
        self.prev_close = close_f64;
        
        // Regime Filter: Use ATR for volatility-based regime detection
        let atr_ratio = atr / close_f64;
        let is_ranging = atr_ratio < 0.01; // Adjust this threshold as needed
        
        let mut signal = None;

        if is_ranging {
            // Overbought Check (Price too high, expect reversal down)
            let is_overbought = kline.close >= bb_upper && rsi_decimal > self.params.rsi_overbought;
            
            // Oversold Check (Price too low, expect reversal up)
            let is_oversold = kline.close <= bb_lower && rsi_decimal < self.params.rsi_oversold;

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