use core_types::{Kline, OrderRequest, OrderSide, OrderType, Signal};
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use ta::indicators::AverageTrueRange;
use ta::Next as _;
use uuid::Uuid;

use crate::error::StrategyError;
use crate::Strategy;
use configuration::settings::SuperTrendParams;

/// The SuperTrend strategy with an ATR-based trend strength filter.
pub struct SuperTrend {
    params: SuperTrendParams,
    symbol: String,
    atr: AverageTrueRange,
    // Track the current SuperTrend value and direction
    supertrend_value: f64,
    trend_direction: Option<Trend>,
    // Previous close for trend detection
    prev_close: Option<f64>,
}

/// Represents the direction of the SuperTrend
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trend {
    Up,
    Down,
}

impl SuperTrend {
    /// Creates a new `SuperTrend` instance.
    pub fn new(params: SuperTrendParams, symbol: String) -> Result<Self, StrategyError> {
        if params.atr_period == 0 {
            return Err(StrategyError::InvalidParameters(
                "ATR period cannot be zero".to_string(),
            ));
        }

        Ok(Self {
            atr: AverageTrueRange::new(params.atr_period as usize).map_err(|e| {
                StrategyError::InvalidParameters(format!("Failed to initialize ATR: {:?}", e))
            })?,
            params,
            symbol,
            supertrend_value: 0.0,
            trend_direction: None,
            prev_close: None,
        })
    }
    
    /// Calculate the SuperTrend indicator value
    fn calculate_supertrend(&mut self, high: f64, low: f64, close: f64) -> (f64, Trend) {
        // Calculate ATR
        let atr = self.atr.next(close);
        
        // Calculate basic upper and lower bands
        let hl2 = (high + low) / 2.0;
        let atr_multiplier = self.params.atr_multiplier.to_f64().unwrap_or(3.0);
        let basic_upper = hl2 + (atr_multiplier * atr);
        let basic_lower = hl2 - (atr_multiplier * atr);
        
        // Determine trend direction and SuperTrend value
        let (supertrend_value, trend) = match self.trend_direction {
            Some(Trend::Up) => {
                if close <= self.supertrend_value {
                    // Trend flipped to down
                    (basic_lower, Trend::Down)
                } else {
                    // Continue up trend
                    (basic_lower.max(self.supertrend_value), Trend::Up)
                }
            }
            Some(Trend::Down) => {
                if close >= self.supertrend_value {
                    // Trend flipped to up
                    (basic_upper, Trend::Up)
                } else {
                    // Continue down trend
                    (basic_upper.min(self.supertrend_value), Trend::Down)
                }
            }
            None => {
                // Initialize on first run
                if close > basic_upper {
                    (basic_lower, Trend::Up)
                } else {
                    (basic_upper, Trend::Down)
                }
            }
        };
        
        // Update state
        self.supertrend_value = supertrend_value;
        
        (supertrend_value, trend)
    }
    
    /// Check if trend strength is sufficient using ATR
    fn is_trend_strong(&mut self, high: f64, low: f64, close: f64) -> bool {
        // Calculate ATR
        let atr = self.atr.next(close);
        
        // Check if ATR is above a minimum threshold (indicating sufficient volatility)
        // Use a much lower percentage to allow more trades
        let volatility_threshold = close * 0.001; // 0.1% of close price (reduced from 0.5%)
        
        atr >= volatility_threshold
    }
}

impl Strategy for SuperTrend {
    fn evaluate(&mut self, kline: &Kline) -> Result<Option<Signal>, StrategyError> {
        // Convert Decimals to f64 for the `ta` crate.
        let high = kline.high.to_f64().ok_or_else(|| {
            StrategyError::InvalidParameters("Failed to convert high to f64".to_string())
        })?;
        let low = kline.low.to_f64().ok_or_else(|| {
            StrategyError::InvalidParameters("Failed to convert low to f64".to_string())
        })?;
        let close = kline.close.to_f64().ok_or_else(|| {
            StrategyError::InvalidParameters("Failed to convert close to f64".to_string())
        })?;

        // Calculate SuperTrend values
        let (_supertrend_value, current_trend) = self.calculate_supertrend(high, low, close);
        
        // Check trend strength using ADX
        let is_trend_strong = self.is_trend_strong(high, low, close);

        let mut signal = None;

        // Detect trend changes and generate signals
        if let Some(prev_trend) = self.trend_direction {
            // Trend Flip Detection: Did the trend just change direction on this bar?
            let is_bullish_flip = prev_trend == Trend::Down && current_trend == Trend::Up;
            let is_bearish_flip = prev_trend == Trend::Up && current_trend == Trend::Down;

            if is_bullish_flip {
                signal = Some(Signal {
                    signal_id: Uuid::new_v4(),
                    timestamp: kline.close_time,
                    confidence: dec!(1.0),
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
            } else if is_bearish_flip {
                signal = Some(Signal {
                    signal_id: Uuid::new_v4(),
                    timestamp: kline.close_time,
                    confidence: dec!(1.0),
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

        // Update state for the next evaluation
        self.trend_direction = Some(current_trend);
        self.prev_close = Some(close);

        Ok(signal)
    }
}