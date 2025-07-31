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

        // --- 2. Calculate Stop-Loss Price and Distance ---
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

        // --- 3. Calculate Risk Capital and Final Quantity ---
        // Determine the total capital to risk on this specific trade.
        let risk_capital = portfolio_state.total_value * self.params.risk_per_trade_pct;

        // Scale the risk down by the strategy's confidence in the signal.
        // A confidence of 0.5 means we risk half the standard amount.
        let scaled_risk_capital = risk_capital * signal.confidence;

        // Calculate the final position size.
        // Quantity = (Total Capital to Risk) / (Per-Unit Risk)
        let quantity = scaled_risk_capital / stop_loss_distance;

        // --- 4. Construct Final Order Request ---
        // Create a new order request, using the original as a template but
        // overriding the quantity with our calculated, risk-managed value.
        let mut final_order = signal.order_request.clone();
        final_order.quantity = quantity;

        Ok(final_order)
    }
}