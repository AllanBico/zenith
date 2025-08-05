use crate::error::ExecutorError;
use async_trait::async_trait;
use configuration::Simulation;
use core_types::{Execution, Kline, OrderRequest, OrderSide};
use rust_decimal::Decimal;
use uuid::Uuid;
use chrono::Utc;
use std::sync::Arc;
use api_client::ApiClient;

/// A generic trait for an execution engine.
///
/// This trait allows the backtester and live engine to be agnostic about whether
/// they are talking to a simulated exchange or a real one.
#[async_trait]
pub trait Executor: Send + Sync {
    /// Processes an `OrderRequest` and returns an `Execution` receipt.
    ///
    /// This function calculates the costs of the trade (fees, slippage) but crucially
    /// **does not modify the portfolio state itself**. The caller is responsible for
    /// using the returned `Execution` to update the portfolio.
    async fn execute(
        &self,
        order: &OrderRequest,
        kline: &Kline,
    ) -> Result<Execution, ExecutorError>;
}

/// The "virtual exchange" for backtesting.
///
/// It holds the simulation parameters and implements the `Executor` trait to
/// create trade executions with realistic costs.
pub struct SimulatedExecutor {
    params: Simulation,
}

impl SimulatedExecutor {
    pub fn new(params: Simulation) -> Self {
        Self { params }
    }

    /// Calculates the execution price, modeling for slippage.
    ///
    /// For a simple model, we assume slippage moves the price against us
    /// by a certain percentage of the bar's high-low range.
    fn calculate_slippage_price(&self, order_side: OrderSide, kline: &Kline) -> Decimal {
        let bar_range = kline.high - kline.low;
        if bar_range.is_zero() {
            return kline.close; // No range, no slippage possible
        }

        let slippage_amount = bar_range * self.params.slippage_pct;

        match order_side {
            // For a buy, slippage makes the price HIGHER (worse).
            OrderSide::Buy => kline.close + slippage_amount,
            // For a sell, slippage makes the price LOWER (worse).
            OrderSide::Sell => kline.close - slippage_amount,
        }
    }
}

#[async_trait]
impl Executor for SimulatedExecutor {
    /// Simulates the execution of a market order.
    async fn execute(
        &self,
        order: &OrderRequest,
        kline: &Kline,
    ) -> Result<Execution, ExecutorError> {
        // 1. Calculate the execution price with slippage.
        let execution_price = self.calculate_slippage_price(order.side, kline);

        // 2. Calculate the trading fee.
        let fee = execution_price * order.quantity * self.params.taker_fee_pct;

        // 3. Construct the execution receipt.
        let execution = Execution {
            execution_id: Uuid::new_v4(),
            client_order_id: order.client_order_id,
            symbol: order.symbol.clone(),
            price: execution_price,
            quantity: order.quantity,
            fee,
            fee_asset: "USDT".to_string(), // Assuming quote asset is the fee asset
            timestamp: Utc::now(), // In a real backtest, this would be kline.close_time
            side: order.side, // Add the side to the execution
        };

        Ok(execution)
    }
}

// --- NEW IMPLEMENTATION ---

/// The "live" executor that sends real orders to the exchange via the ApiClient.
pub struct LiveExecutor {
    api_client: Arc<dyn ApiClient>,
}

impl LiveExecutor {
    pub fn new(api_client: Arc<dyn ApiClient>) -> Self {
        Self { api_client }
    }
}

#[async_trait]
impl Executor for LiveExecutor {
    /// Executes a real order by passing it to the API client.
    /// It then transforms the exchange's response into our internal `Execution` format.
    async fn execute(
        &self,
        order: &OrderRequest,
        _kline: &Kline, // The kline is not needed for a live market order
    ) -> Result<Execution, ExecutorError> {
        let order_response = self
            .api_client
            .place_order(order)
            .await
            .map_err(|e| ExecutorError::Api(e.to_string()))?; // Convert ApiError to ExecutorError

        // Transform the exchange's OrderResponse into our internal Execution receipt.
        let execution = Execution {
            execution_id: Uuid::new_v4(),
            client_order_id: Uuid::parse_str(&order_response.client_order_id)
                .unwrap_or_else(|_| order.client_order_id), // Fallback to original
            symbol: order_response.symbol,
            side: order_response.side,
            price: order_response.avg_price,
            quantity: order_response.executed_qty,
            fee: "0".parse().unwrap(), // The response doesn't contain the fee directly, needs another query
            fee_asset: "USDT".to_string(), // Assume USDT for now
            timestamp: Utc::now(), // Use current time for live execution
        };

        Ok(execution)
    }
}