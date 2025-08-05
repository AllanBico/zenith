use crate::error::EngineError;
use tracing;
use api_client::{ApiClient, LiveConnector}; // LiveConnector is needed here now
use configuration::{Config, LiveConfig};
use database::DbRepository;
use executor::{Executor, Portfolio}; // Import the generic Executor trait
use risk::RiskManager;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use strategies::Strategy;
use tokio::sync::Mutex;
use uuid::Uuid;
use chrono::Utc;
use events::{LogMessage, LogLevel, WsMessage, KlineData};
use tokio::sync::broadcast;

pub mod error;
pub mod reconciler;

pub use reconciler::StateReconciler;
/// Rounds quantity to the appropriate precision for the given symbol.
/// This is a simple implementation - in production, you'd fetch this from exchange info.
fn round_quantity_to_precision(symbol: &str, quantity: rust_decimal::Decimal) -> rust_decimal::Decimal {
    // Simple precision mapping for common symbols
    // In a real implementation, this would come from exchange info API
    let precision = match symbol {
        "BTCUSDT" => 3,  // BTC precision is 0.001
        "ETHUSDT" => 3,  // ETH precision is 0.001
        _ => 2,          // Default to 2 decimal places
    };
    
    // Round to the specified precision
    let scale = rust_decimal::Decimal::from(10_i64.pow(precision as u32));
    (quantity * scale).round() / scale
}

/// A wrapper for Kline data that includes the symbol information.
/// This is needed because the Kline struct doesn't contain symbol information.
#[derive(Debug, Clone)]
pub struct KlineWithSymbol {
    pub symbol: String,
    pub kline: core_types::Kline,
}

/// A container for the components related to a single trading instrument.
pub struct Bot {
    pub symbol: String,
    pub strategy: Box<dyn Strategy>,
}

/// The central orchestrator for the live trading application.
pub struct LiveEngine {
    // --- Configuration ---
    live_config: LiveConfig,
    base_config: Config,

    // --- Shared, Thread-Safe Components ---
    api_client: Arc<dyn ApiClient>, // Still needed for state reconciliation
    executor: Arc<dyn Executor>,   // The generic executor for placing orders
    db_repo: DbRepository,
    portfolio: Arc<Mutex<Portfolio>>,
    risk_manager: Arc<dyn RiskManager>,

    // --- NEW: The event broadcaster ---
    event_tx: broadcast::Sender<WsMessage>,

    // --- Bot Management ---
    bots: HashMap<String, Bot>,
}


impl LiveEngine {
    /// Creates a new `LiveEngine` instance with all its required components.
    pub fn new(
        live_config: LiveConfig,
        base_config: Config,
        api_client: Arc<dyn ApiClient>,
        executor: Arc<dyn Executor>, // <-- NEW: Accepts a generic executor
        db_repo: DbRepository,
        risk_manager: Arc<dyn RiskManager>,
        event_tx: broadcast::Sender<WsMessage>, // <-- ADD THIS
    ) -> Self {
        let portfolio = Arc::new(Mutex::new(Portfolio::new(
            base_config.backtest.initial_capital,
        )));

        Self {
            live_config,
            base_config,
            api_client, // The ApiClient is now passed through
            executor,   // Store the generic executor
            db_repo,
            portfolio,
            risk_manager,
            event_tx, // <-- STORE IT
            bots: HashMap::new(),
        }
    }

    /// A helper method to both log via tracing and broadcast a WsMessage::Log.
    fn log(&self, level: LogLevel, message: &str) {
        let msg = message.to_string();
        match level {
            LogLevel::Info => tracing::info!("{}", msg),
            LogLevel::Warn => tracing::warn!("{}", msg),
            LogLevel::Error => tracing::error!("{}", msg),
        }
        
        let log_msg = WsMessage::Log(LogMessage {
            timestamp: Utc::now(),
            level,
            message: msg,
        });

        // We don't care if there are no subscribers, so we ignore the error.
        let _ = self.event_tx.send(log_msg);
    }
    
    /// Helper to broadcast the current portfolio state.
    async fn broadcast_portfolio_state(&self) -> Result<(), EngineError> {
        let portfolio = self.portfolio.lock().await;
        // In a real system, we'd need a map of all live mark prices.
        // For now, we'll send a simplified state.
        let state_msg = WsMessage::PortfolioState(events::PortfolioState {
            timestamp: Utc::now(),
            cash: portfolio.cash,
            total_value: portfolio.cash, // Simplified for now
            positions: portfolio.positions.values().cloned().collect(),
        });
        
        if self.event_tx.send(state_msg).is_err() {
             // Optional: log if there are no listeners
        }
        Ok(())
    }

    /// Initializes the engine to a ready state for live trading.
    pub async fn init(&mut self) -> Result<(), EngineError> {
        self.log(LogLevel::Info, "Initializing trading engine...");
        self.sync_portfolio_state().await?;
        self.log(LogLevel::Info, "Portfolio state synchronized with exchange.");
        self.populate_bots()?;
        self.log(LogLevel::Info, &format!("Loaded {} active bots.", self.bots.len()));
        
        for symbol in self.bots.keys() {
            self.log(LogLevel::Info, &format!("Setting leverage for {}...", symbol));
            self.api_client.set_leverage(symbol, 10).await?;
        }
        
        self.log(LogLevel::Info, "Engine initialization complete.");
        self.broadcast_portfolio_state().await?; // Broadcast initial state
        Ok(())
    }

    /// Fetches cash balance and open positions to create an accurate initial portfolio.
    async fn sync_portfolio_state(&mut self) -> Result<(), EngineError> {
        tracing::debug!("Fetching account balance and positions...");
        let balances = self.api_client.get_account_balance().await?;
        let positions = self.api_client.get_open_positions().await?;
        
        tracing::debug!("Found {} balance entries and {} open positions", balances.len(), positions.len());
        
        let mut portfolio = self.portfolio.lock().await;

        // Find the USDT balance to set our cash value.
        if let Some(usdt_balance) = balances.iter().find(|b| b.asset == "USDT") {
            portfolio.cash = usdt_balance.available_balance;
            tracing::info!("[ENGINE] Found USDT balance: {}", usdt_balance.available_balance);
        } else {
            // Handle case where there is no USDT, for now, we'll just log it.
            tracing::warn!("No USDT balance found in account.");
            portfolio.cash = "0".parse().unwrap();
        }
        
        tracing::info!("[ENGINE] Portfolio cash set to: {}", portfolio.cash);

        // Clear any existing positions and reconstruct from the exchange's data.
        portfolio.positions.clear();
        let mut open_positions_count = 0;
        let total_positions = positions.len();
        for pos in positions {
            // We only care about positions that are actually open (non-zero amount).
            if pos.position_amt != rust_decimal::Decimal::ZERO {
                open_positions_count += 1;
                let side = if pos.position_amt.is_sign_positive() {
                    core_types::OrderSide::Buy
                } else {
                    core_types::OrderSide::Sell
                };
                
                let symbol = pos.symbol.clone();
                let position = core_types::Position {
                    position_id: Uuid::new_v4(),
                    symbol: symbol.clone(),
                    side,
                    quantity: pos.position_amt.abs(),
                    entry_price: pos.entry_price,
                    unrealized_pnl: pos.un_realized_profit,
                    last_updated: Utc::now(),
                };
                portfolio.positions.insert(symbol.clone(), position);
                tracing::debug!("Added position: {} {:?} {:.4} @ {:.2}", 
                    symbol, side, pos.position_amt.abs(), pos.entry_price);
            }
        }
        tracing::debug!("Total API positions: {}, Actual open positions: {}", total_positions, open_positions_count);

        Ok(())
    }

    /// Creates and stores `Bot` instances for all `enabled = true` bots in the config.
    fn populate_bots(&mut self) -> Result<(), EngineError> {
        for bot_config in &self.live_config.bots {
            if bot_config.enabled {
                tracing::debug!("Loading bot: {} with strategy: {:?}", bot_config.symbol, bot_config.strategy_id);
                let mut temp_config = self.base_config.clone();
                let strategy = crate::util::create_strategy_from_live_config(&mut temp_config, bot_config)?;
                
                let bot = Bot {
                    symbol: bot_config.symbol.clone(),
                    strategy,
                };
                self.bots.insert(bot_config.symbol.clone(), bot);
                tracing::debug!("Bot loaded successfully: {}", bot_config.symbol);
            } else {
                tracing::debug!("Skipping disabled bot: {}", bot_config.symbol);
            }
        }
        Ok(())
    }

    /// The main event loop for the live trading engine.
    pub async fn run(&mut self) -> Result<(), EngineError> {
        self.init().await?;

        let symbols: Vec<String> = self.bots.keys().cloned().collect();
        if symbols.is_empty() {
            self.log(LogLevel::Warn, "No bots enabled in live.toml. Exiting.");
            return Ok(());
        }
        let interval = &self.base_config.backtest.interval;
        
        let is_live = self.live_config.live_trading_enabled;
        let connector = LiveConnector::new(is_live);
        let mut kline_rx = connector.subscribe_to_klines(&symbols, interval)?;
        
        let reconciler = StateReconciler::new(
            Arc::clone(&self.portfolio),
            Arc::clone(&self.api_client),
            self.db_repo.clone(),
            self.event_tx.clone(), // Give the reconciler the sender
        );
        tokio::spawn(reconciler.start());
        
        self.log(LogLevel::Info, &format!("Engine is running. Subscribed to {} kline streams.", symbols.len()));

        tracing::info!("[ENGINE] Starting kline processing loop");
        while let Some((symbol, kline)) = kline_rx.recv().await {
            let start_time = std::time::Instant::now();
            tracing::info!("[ENGINE] Received kline for {}: close price {}", symbol, kline.close);
            match self.process_kline(&symbol, &kline).await {
                Ok(_) => {
                    let duration = start_time.elapsed();
                    tracing::info!("[ENGINE] Successfully processed kline for {} in {:?}", symbol, duration);
                }
                Err(e) => {
                    let duration = start_time.elapsed();
                    tracing::error!("[ENGINE] Failed to process kline for {} in {:?}: {:?}", symbol, duration, e);
                    self.log(LogLevel::Error, &format!("Failed to process kline: {:?}", e));
                }
            }
        }
        
        tracing::error!("[ENGINE] Kline channel closed - no more klines will be received");
        
        self.log(LogLevel::Error, "WebSocket stream ended unexpectedly.");
        Ok(())
    }

    /// The core logic for processing a single market event (Kline).
    async fn process_kline(&mut self, symbol: &str, kline: &core_types::Kline) -> Result<(), EngineError> {
        tracing::info!("[ENGINE] Processing kline for {}: broadcast_klines = {}", symbol, self.live_config.broadcast_klines);
        // Broadcast kline data to WebSocket clients if enabled
        if self.live_config.broadcast_klines {
            let kline_data = events::KlineData {
                symbol: symbol.to_string(),
                kline: kline.clone(),
            };
            let msg = events::WsMessage::KlineData(kline_data);
            tracing::info!("[ENGINE] Broadcasting kline data for {}: {:?}", symbol, msg);
            match self.event_tx.send(msg) {
                Ok(_) => {
                    tracing::info!("[ENGINE] Successfully broadcast kline data for {}", symbol);
                }
                Err(e) => {
                    tracing::error!("[ENGINE] Failed to broadcast kline data for {}: {:?}", symbol, e);
                    // Check if it's a channel full error
                    if e.to_string().contains("channel full") {
                        tracing::error!("[ENGINE] Broadcast channel is full! Consider increasing capacity.");
                    }
                }
            }
        } else {
            tracing::debug!("[ENGINE] Kline broadcasting is disabled in config");
        }

        let bot = self.bots.get_mut(symbol).ok_or_else(|| EngineError::BotNotFound(symbol.to_string()))?;

        if let Some(signal) = bot.strategy.evaluate(&kline)? {
            let bot_symbol = bot.symbol.clone();
            let signal_side = signal.order_request.side;
            let close_price = kline.close;
            
            self.log(LogLevel::Info, &format!("Signal generated for {}: {:?} at price {}", bot_symbol, signal_side, close_price));
            tracing::info!("[ENGINE] About to enter risk management section for {}", bot_symbol);

            let order_request = { // Scoped to release the lock quickly
                tracing::info!("[ENGINE] About to lock portfolio for {}", bot_symbol);
                let portfolio_guard = self.portfolio.lock().await;
                tracing::info!("[ENGINE] Portfolio locked successfully for {}", bot_symbol);
                // Create a map of all current prices needed for equity calculation
                let mut market_prices = HashMap::new();
                market_prices.insert(bot_symbol.clone(), close_price);
                
                // Add prices for any other symbols that have positions
                for (pos_symbol, _) in &portfolio_guard.positions {
                    if pos_symbol != &bot_symbol {
                        // For now, we'll use the last known price or a default
                        // In a real system, you'd fetch current prices for all symbols
                        market_prices.insert(pos_symbol.clone(), dec!(0)); // Placeholder
                    }
                }
                
                let latest_equity = portfolio_guard.calculate_total_equity(&market_prices)?;
                let portfolio_state = events::PortfolioState {
                    timestamp: Utc::now(),
                    cash: portfolio_guard.cash,
                    total_value: latest_equity,
                    positions: portfolio_guard.positions.values().cloned().collect(),
                };
                
                tracing::info!("[ENGINE] Portfolio state - Cash: {}, Total Value: {}, Positions: {:?}", 
                    portfolio_state.cash, portfolio_state.total_value, portfolio_state.positions);
                
                tracing::info!("[ENGINE] Calling risk manager with signal: {:?}", signal);
                
                match self.risk_manager.evaluate_signal(&signal, &portfolio_state, close_price) {
                    Ok(order) => {
                        tracing::info!("[ENGINE] Risk manager approved order: {:?}", order);
                        order
                    },
                    Err(e) => {
                        tracing::error!("[ENGINE] Risk management rejected signal: {:?}", e);
                        self.log(LogLevel::Warn, &format!("Risk management rejected signal: {:?}", e));
                        tracing::info!("[ENGINE] Skipping signal due to risk management rejection, but continuing to process klines");
                        return Ok(()); // Skip this signal but continue processing
                    }
                }
            };
            self.log(LogLevel::Info, &format!("Risk assessment passed. Final Order: {:?} {} @ Market", order_request.quantity, order_request.symbol));

            match self.executor.execute(&order_request, kline).await {
                Ok(execution) => {
                    self.log(LogLevel::Info, &format!("SUCCESS: Execution confirmed for {}: {:?}", execution.symbol, execution.price));
                    
                    // Update local portfolio state
                    {
                        let mut portfolio = self.portfolio.lock().await;
                        portfolio.update_with_execution(&execution)?;
                    } // Portfolio lock is released here
                    
                    self.broadcast_portfolio_state().await?; // Broadcast updated state
                    
                    // Trigger immediate portfolio sync to ensure our state matches the exchange
                    tracing::info!("[ENGINE] Triggering immediate portfolio sync after execution");
                    self.sync_portfolio_state().await?;
                    self.broadcast_portfolio_state().await?; // Broadcast the synced state
                }
                Err(e) => {
                    self.log(LogLevel::Error, &format!("ERROR: Failed to execute order for {}: {:?}", bot_symbol, e));
                }
            }
        }
        Ok(())
    }
}

// We need a helper to create strategies, let's put it in a `util` module.
pub mod util {
    use super::*;
    use configuration::{LiveBotConfig, MACrossoverParams, ProbReversionParams, SuperTrendParams};
    use serde_json::from_value;
    use strategies::create_strategy;

    pub fn create_strategy_from_live_config(
        base_config: &mut Config,
        bot_config: &LiveBotConfig,
    ) -> Result<Box<dyn Strategy>, EngineError> {
        match bot_config.strategy_id {
            strategies::StrategyId::MACrossover => {
                let params: MACrossoverParams = from_value(bot_config.params.clone())?;
                base_config.strategies.ma_crossover = params;
            },
            strategies::StrategyId::SuperTrend => {
                let params: SuperTrendParams = from_value(bot_config.params.clone())?;
                base_config.strategies.super_trend = params;
            },
            strategies::StrategyId::ProbReversion => {
                let params: ProbReversionParams = from_value(bot_config.params.clone())?;
                base_config.strategies.prob_reversion = params;
            },
            _ => return Err(EngineError::Configuration("Strategy not supported in live engine".to_string())),
        }
        
        Ok(create_strategy(bot_config.strategy_id, base_config, bot_config.symbol.as_str())?)
    }
}
