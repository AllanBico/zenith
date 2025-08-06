use crate::error::ExecutorError;
use async_trait::async_trait;
use configuration::Simulation;
use core_types::{Execution, Kline, OrderRequest, OrderSide, OrderType};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;
use chrono::Utc;
use std::sync::Arc;
use api_client::ApiClient;
use tracing;

/// Rounds a price to the appropriate tick size for the given symbol.
fn round_price_to_tick_size(symbol: &str, price: Decimal) -> Decimal {
    // Binance Futures tick sizes (minimum price increments)
    let tick_size = match symbol {
        "BTCUSDT" => dec!(0.1),    // BTC tick size is $0.1
        "ETHUSDT" => dec!(0.01),   // ETH tick size is $0.01
        _ => dec!(0.01),           // Default tick size
    };
    
    // Round to the nearest tick size
    let rounded = (price / tick_size).round() * tick_size;
    rounded
}

/// Rounds a quantity to the appropriate step size for the given symbol.
fn round_quantity_to_step_size(symbol: &str, quantity: Decimal) -> Decimal {
    // Binance Futures step sizes (minimum quantity increments)
    let step_size = match symbol {
        "BTCUSDT" => dec!(0.001),  // BTC step size is 0.001
        "ETHUSDT" => dec!(0.001),  // ETH step size is 0.001
        _ => dec!(0.001),          // Default step size
    };
    
    // Round down to the nearest step size
    let rounded = (quantity / step_size).floor() * step_size;
    
    // Ensure we don't return zero if the original quantity was positive
    if quantity > Decimal::ZERO && rounded == Decimal::ZERO {
        step_size // Return minimum quantity
    } else {
        rounded
    }
}

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
        best_bid: Option<Decimal>, 
        best_ask: Option<Decimal>, 
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
        tracing::debug!("Slippage calculation: bar_range={}, slippage_pct={}", bar_range, self.params.slippage_pct);
        
        if bar_range.is_zero() {
            tracing::debug!("No bar range, returning close price: {}", kline.close);
            return kline.close; // No range, no slippage possible
        }

        let slippage_amount = bar_range * self.params.slippage_pct;
        tracing::debug!("Slippage amount: {}", slippage_amount);

        let result = match order_side {
            // For a buy, slippage makes the price HIGHER (worse).
            OrderSide::Buy => kline.close + slippage_amount,
            // For a sell, slippage makes the price LOWER (worse).
            OrderSide::Sell => kline.close - slippage_amount,
        };
        
        tracing::debug!("Final execution price: {} (close: {}, side: {:?})", result, kline.close, order_side);
        result
    }
}

#[async_trait]
impl Executor for SimulatedExecutor {
    /// Simulates the execution of a market order.
    async fn execute(
        &self,
        order: &OrderRequest,
        kline: &Kline,
        best_bid: Option<Decimal>, // <-- ADDED
        best_ask: Option<Decimal>, // <-- ADDED
    ) -> Result<Execution, ExecutorError> {
        tracing::debug!("SimulatedExecutor: Executing order {:?} with kline {:?}", order, kline);
        
        // 1. Calculate the execution price with slippage.
        let execution_price = self.calculate_slippage_price(order.side, kline);
        tracing::debug!("SimulatedExecutor: Calculated execution price: {} (original close: {})", execution_price, kline.close);

        // 2. Calculate the trading fee.
        let fee = execution_price * order.quantity * self.params.taker_fee_pct;
        tracing::debug!("SimulatedExecutor: Calculated fee: {}", fee);

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

        tracing::debug!("SimulatedExecutor: Created execution: {:?}", execution);
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
        kline: &Kline, // The kline is not needed for a live market order
        best_bid: Option<Decimal>, // <-- ADDED
        best_ask: Option<Decimal>, // <-- ADDED
    ) -> Result<Execution, ExecutorError> {
        tracing::debug!("LiveExecutor: Executing order {:?} with kline {:?}", order, kline);
        
        let order_response = self
            .api_client
            .place_order(order)
            .await
            .map_err(|e| ExecutorError::Api(e.to_string()))?; // Convert ApiError to ExecutorError

        tracing::debug!("LiveExecutor: Received order response: {:?}", order_response);
        
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

        tracing::debug!("LiveExecutor: Created execution: {:?}", execution);
        Ok(execution)
    }
}

/// An executor that places "Post-Only" LIMIT orders to act as a market maker.
pub struct LimitOrderExecutor {
    api_client: Arc<dyn ApiClient>,
}

impl LimitOrderExecutor {
    pub fn new(api_client: Arc<dyn ApiClient>) -> Self {
        Self { api_client }
    }
}

#[async_trait]
impl Executor for LimitOrderExecutor {
    /// Executes a "Post-Only" LIMIT order inside the spread to ensure maker execution.
    async fn execute(
        &self,
        order: &OrderRequest,
        _kline: &Kline,
        best_bid: Option<Decimal>,
        best_ask: Option<Decimal>,
    ) -> Result<Execution, ExecutorError> {
        // Calculate a price inside the spread to ensure the order acts as a maker
        let (bid, ask) = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) => (bid, ask),
            _ => return Err(ExecutorError::Api("Best bid and ask prices not available for LIMIT order.".to_string())),
        };
        
        // Ensure we have a valid spread
        if bid >= ask {
            return Err(ExecutorError::Api("Invalid spread: bid >= ask".to_string()));
        }
        
        let calculated_price = match order.side {
            OrderSide::Buy => {
                // For buy orders, place slightly above the best bid but below the best ask
                // This ensures we're inside the spread and will act as a maker
                let spread = ask - bid;
                let offset = spread * dec!(0.1); // 10% into the spread
                let price = bid + offset;
                tracing::debug!("LimitOrderExecutor: Buy order - Bid: {}, Ask: {}, Spread: {}, Offset: {}, Raw Price: {}", 
                    bid, ask, spread, offset, price);
                price
            },
            OrderSide::Sell => {
                // For sell orders, place slightly below the best ask but above the best bid
                // This ensures we're inside the spread and will act as a maker
                let spread = ask - bid;
                let offset = spread * dec!(0.1); // 10% into the spread
                let price = ask - offset;
                tracing::debug!("LimitOrderExecutor: Sell order - Bid: {}, Ask: {}, Spread: {}, Offset: {}, Raw Price: {}", 
                    bid, ask, spread, offset, price);
                price
            },
        };
        
        // Round the price to the appropriate tick size
        let price = round_price_to_tick_size(&order.symbol, calculated_price);
        tracing::debug!("LimitOrderExecutor: Rounded price for {}: {} -> {}", order.symbol, calculated_price, price);
        
        // Round the quantity to the appropriate step size
        let rounded_quantity = round_quantity_to_step_size(&order.symbol, order.quantity);
        tracing::debug!("LimitOrderExecutor: Rounded quantity for {}: {} -> {}", order.symbol, order.quantity, rounded_quantity);
        
        // Create a new order request that specifies the limit price and rounded quantity.
        let mut limit_order = order.clone();
        limit_order.order_type = OrderType::Limit;
        limit_order.price = Some(price);
        limit_order.quantity = rounded_quantity;

        // Call the specialized `place_limit_order` method on the ApiClient.
        let order_response = self
            .api_client
            .place_limit_order(&limit_order) // We will build this in Task 5
            .await
            .map_err(|e| ExecutorError::Api(e.to_string()))?;

        // Transform the response into our internal Execution format.
        // NOTE: A LIMIT order may not fill immediately. This `Execution` is an acknowledgement
        // that the order was PLACED. A separate process (User Data Stream) will be needed
        // to confirm the FILL. For now, we optimistically create the execution.
        let execution = Execution {
            execution_id: Uuid::new_v4(),
            client_order_id: Uuid::parse_str(&order_response.client_order_id).unwrap_or(order.client_order_id),
            symbol: order_response.symbol,
            side: order_response.side,
            price: order_response.price, // This will be the limit price, not necessarily the fill price
            quantity: order_response.orig_qty, // The full quantity is placed
            fee: "0".parse().unwrap(),
            fee_asset: "USDT".to_string(),
            timestamp: Utc::now(),
        };

        Ok(execution)
    }
}