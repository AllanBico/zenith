use crate::error::StrategyError;
use crate::Strategy;
use configuration::SuperTrendParams;
use core_types::{Kline, OrderRequest, OrderSide, OrderType, Signal};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use ta::indicators::{AverageDirectionalIndex as Adx, Supertrend, SupertrendOutput};
use ta::{Next, NextFrom, Period};
use uuid::Uuid;

/// Represents the state of the SuperTrend signal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Trend {
    Up,
    Down,
}

/// The SuperTrend strategy with an ADX filter for trend strength.
pub struct SuperTrend {
    params: SuperTrendParams,
    supertrend: Supertrend,
    adx: Adx,
    // State: The direction of the trend on the previous bar to detect a "flip".
    prev_trend: Option<Trend>,
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
            supertrend: Supertrend::new(params.atr_period, params.atr_multiplier.to_f64().unwrap())
                .unwrap(),
            adx: Adx::new(params.adx_period).unwrap(),
            params,
            prev_trend: None,
        })
    }
}

impl Strategy for SuperTrend {

    fn evaluate(&mut self, kline: &Kline) -> Result<Option<Signal>, StrategyError> {
        // Convert Decimals to f64 for the `ta` crate.
        let high = kline.high.to_f64().unwrap();
        let low = kline.low.to_f64().unwrap();
        let close = kline.close.to_f64().unwrap();

        // The Supertrend indicator in the `ta` crate requires a specific input struct.
        let kline_input = ta::DataItem::builder()
            .high(high)
            .low(low)
            .close(close)
            .build()
            .unwrap();

        // Calculate current indicator values.
        let st_output: SupertrendOutput = self.supertrend.next(&kline_input);
        let adx_output = self.adx.next(&kline_input);
        
        let current_adx = Decimal::from_f64(adx_output.adx).unwrap();
        let current_trend = if st_output.is_up() { Trend::Up } else { Trend::Down };

        let mut signal = None;

        // Ensure we have a previous trend to compare against for a flip.
        // This also handles the indicator warm-up period.
        if let Some(prev_trend) = self.prev_trend {
            // ---===[ Signal Logic ]===---

            // Trend Strength Filter: Is the ADX high enough to confirm a trend?
            let is_trend_strong = current_adx > self.params.adx_threshold;

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
                    },
                });
            } else if is_bearish_flip && is_trend_strong {
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

        // Update state for the next evaluation.
        self.prev_trend = Some(current_trend);

        Ok(signal)
    }
}