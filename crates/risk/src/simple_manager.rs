use crate::error::RiskError;
use crate::RiskManager;
use configuration::RiskManagement;
use core_types::{OrderRequest, OrderSide, Signal};
use events::PortfolioState;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// A simple, concrete implementation of the `RiskManager` trait.
///
/// This manager calculates position size based on a fixed fractional percentage
/// of portfolio equity and a predefined stop-loss distance.
#[derive(Debug, Clone)]
pub struct SimpleRiskManager {
    params: RiskManagement,
}

impl SimpleRiskManager {
    /// Creates a new `SimpleRiskManager` with the given configuration parameters.
    pub fn new(params: RiskManagement) -> Result<Self, RiskError> {
        // Validate that risk parameters are logical.
        if params.risk_per_trade_pct <= dec!(0) || params.risk_per_trade_pct >= dec!(1) {
            return Err(RiskError::InvalidParameters(
                "risk_per_trade_pct must be between 0 and 1".to_string(),
            ));
        }
        if params.stop_loss_pct <= dec!(0) {
            return Err(RiskError::InvalidParameters(
                "stop_loss_pct must be greater than 0".to_string(),
            ));
        }
        Ok(Self { params })
    }
}

impl RiskManager for SimpleRiskManager {
    /// Performs the stop-loss-driven, fixed-fractional position sizing calculation.
    fn evaluate_signal(
        &self,
        signal: &Signal,
        portfolio_state: &PortfolioState,
        entry_price: Decimal,
    ) -> Result<OrderRequest, RiskError> {
        // --- 1. Validation ---
        if entry_price <= dec!(0) {
            return Err(RiskError::InvalidEntryPrice(entry_price));
        }
        if portfolio_state.total_value <= dec!(0) {
            return Err(RiskError::InsufficientEquity(portfolio_state.total_value));
        }

        // --- 2. Check for existing position ---
        let current_position = portfolio_state.positions.iter()
            .find(|p| p.symbol == signal.order_request.symbol);

        // If we have a position in the opposite direction, close it first
        if let Some(position) = current_position {
            if position.side != signal.order_request.side {
                // Create a market order to close the entire position
                let mut close_order = signal.order_request.clone();
                close_order.quantity = position.quantity;
                close_order.side = position.side.opposite();
                return Ok(close_order);
            }
            // If we have a position in the same direction, we'll add to it below
        }

        // --- 3. Calculate Stop-Loss Price and Distance ---
        let stop_loss_price = match signal.order_request.side {
            OrderSide::Buy => entry_price * (dec!(1) - self.params.stop_loss_pct),
            OrderSide::Sell => entry_price * (dec!(1) + self.params.stop_loss_pct),
        };

        let stop_loss_distance = (entry_price - stop_loss_price).abs();
        if stop_loss_distance.is_zero() {
            return Err(RiskError::Calculation(
                "Stop-loss distance cannot be zero".to_string(),
            ));
        }

        // --- 4. Calculate Risk Capital and Final Quantity ---
        // Determine the total capital to risk on this specific trade.
        let risk_capital = portfolio_state.total_value * self.params.risk_per_trade_pct;

        // Scale the risk down by the strategy's confidence in the signal.
        // A confidence of 0.5 means we risk half the standard amount.
        let scaled_risk_capital = risk_capital * signal.confidence;

        // Calculate the target position size in quote currency (USDT)
        let position_value = scaled_risk_capital / self.params.stop_loss_pct;
        
        // Ensure we don't try to allocate more than our available equity
        let max_position_value = portfolio_state.cash * dec!(0.95); // Leave some buffer
        let position_value = position_value.min(max_position_value);
        
        // Convert position value to base currency (e.g., BTC)
        let target_quantity = if entry_price > Decimal::ZERO {
            position_value / entry_price
        } else {
            Decimal::ZERO
        };
        
        // Round to 6 decimal places to avoid precision issues with very small quantities
        let target_quantity = target_quantity.round_dp(6);
        
        // Debug logging
        println!("Risk calculation - Entry: {}, Risk Capital: {}, Position Value: {}, Max Allowed: {}, Target Qty: {}",
            entry_price, scaled_risk_capital, position_value, max_position_value, target_quantity);
        
        // If we already have a position, calculate how much more to add
        let quantity = if let Some(position) = current_position {
            // Don't reduce position size, only increase if needed
            if target_quantity > position.quantity {
                target_quantity - position.quantity
            } else {
                // If we're not increasing the position, return the original signal
                // which will be a no-op (same side, same or smaller size)
                return Ok(signal.order_request.clone());
            }
        } else {
            target_quantity
        };

        // --- 5. Construct Final Order Request ---
        // Create a new order request, using the original as a template but
        // overriding the quantity with our calculated, risk-managed value.
        let mut final_order = signal.order_request.clone();
        final_order.quantity = quantity;

        Ok(final_order)
    }
}