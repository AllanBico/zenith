use core_types::{Kline, OrderRequest, OrderSide, OrderType, Signal};
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use ta::indicators::AverageTrueRange;
use ta::Next as _;
use uuid::Uuid;

use crate::error::StrategyError;
use crate::Strategy;
use configuration::settings::SuperTrendParams;

/// The SuperTrend strategy with an ATR filter for trend strength.
pub struct SuperTrend {
    params: SuperTrendParams,
    atr: AverageTrueRange,
    // Track the current upper and lower bands
    upper_band: f64,
    lower_band: f64,
    // State: The direction of the trend on the previous bar to detect a "flip".
    prev_trend: Option<Trend>,
}

/// Represents the direction of the trend
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trend {
    Up,
    Down,
}

impl SuperTrend {
    /// Creates a new `SuperTrend` instance.
    pub fn new(params: SuperTrendParams) -> Result<Self, StrategyError> {
        if params.atr_period == 0 || params.adx_period == 0 {
            return Err(StrategyError::InvalidParameters(
                "Indicator periods cannot be zero".to_string(),
            ));
        }

        Ok(Self {
            atr: AverageTrueRange::new(params.atr_period as usize).map_err(|e| {
                StrategyError::InvalidParameters(format!("Failed to initialize ATR: {:?}", e))
            })?,
            params,
            upper_band: 0.0,
            lower_band: 0.0,
            prev_trend: None,
        })
    }
    
    /// Calculate the SuperTrend indicator values
    fn calculate_bands(&mut self, high: f64, low: f64, close: f64) -> (f64, f64, Trend) {
        // Update ATR
        let atr = self.atr.next(close);
        
        // Calculate basic upper and lower bands
        let hl2 = (high + low) / 2.0;
        let atr_multiplier = self.params.atr_multiplier.to_f64().unwrap_or(3.0);
        let basic_upper = hl2 + (atr_multiplier * atr);
        let basic_lower = hl2 - (atr_multiplier * atr);
        
        // Update final bands based on previous trend
        let (upper_band, lower_band, trend) = match self.prev_trend {
            Some(Trend::Up) => {
                let new_lower = basic_lower.max(self.lower_band);
                (basic_upper, new_lower, Trend::Up)
            }
            Some(Trend::Down) => {
                let new_upper = basic_upper.min(self.upper_band);
                (new_upper, basic_lower, Trend::Down)
            }
            None => (basic_upper, basic_lower, Trend::Up), // Default to Up trend
        };
        
        // Update state
        self.upper_band = upper_band;
        self.lower_band = lower_band;
        
        (upper_band, lower_band, trend)
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
        let (_upper_band, _lower_band, current_trend) = self.calculate_bands(high, low, close);
        
        // Calculate trend strength based on ATR
        let atr = self.atr.next(close);
        let is_trend_strong = atr > (close * 0.01); // Simple trend strength check

        let mut signal = None;

        // Ensure we have a previous trend to compare against for a flip
        if let Some(prev_trend) = self.prev_trend {
            // Trend Flip Detection: Did the trend just change direction on this bar?
            let is_bullish_flip = prev_trend == Trend::Down && current_trend == Trend::Up;
            let is_bearish_flip = prev_trend == Trend::Up && current_trend == Trend::Down;

            if is_bullish_flip && is_trend_strong {
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
                        position_side: None, // Will be set by engine
                    },
                });
            } else if is_bearish_flip && is_trend_strong {
                signal = Some(Signal {
                    signal_id: Uuid::new_v4(),
                    timestamp: kline.close_time,
                    confidence: dec!(1.0),
                    order_request: OrderRequest {
                        client_order_id: Uuid::new_v4(),
                        symbol: "PLACEHOLDER_SYMBOL".to_string(),
                        side: OrderSide::Sell,
                        order_type: OrderType::Market,
                        quantity: dec!(1.0),
                        price: None,
                        position_side: None, // Will be set by engine
                    },
                });
            }
        } else {
            // Initialize prev_trend on first run
            self.prev_trend = Some(current_trend);
        }

        // Update state for the next evaluation
        self.prev_trend = Some(current_trend);

        Ok(signal)
    }
}