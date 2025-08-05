use crate::error::EngineError;
use api_client::{ApiClient, LiveConnector}; // LiveConnector is needed here now
use configuration::{Config, LiveConfig};
use database::DbRepository;
use executor::{Executor, Portfolio}; // Import the generic Executor trait
use risk::RiskManager;
use std::collections::HashMap;
use std::sync::Arc;
use strategies::Strategy;
use tokio::sync::Mutex;
use uuid::Uuid;
use chrono::Utc;
use events;

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
            bots: HashMap::new(),
        }
    }

    /// Initializes the engine to a ready state for live trading.
    /// This is the primary setup function that must be called before `run`.
    pub async fn init(&mut self) -> Result<(), EngineError> {
        println!("Initializing trading engine...");

        // 1. Synchronize Portfolio State with the Exchange
        self.sync_portfolio_state().await?;
        println!("Portfolio state synchronized with exchange.");

        // 2. Populate Bots from Configuration
        self.populate_bots()?;
        println!("Loaded {} active bots.", self.bots.len());
        
        // 3. Set Leverage for all active symbols
        for symbol in self.bots.keys() {
            println!("Setting leverage for {}...", symbol);
            // We'll use a hardcoded leverage for now. This could come from config later.
            self.api_client.set_leverage(symbol, 10).await?;
        }
        
        println!("Engine initialization complete.");
        Ok(())
    }

    /// Fetches cash balance and open positions to create an accurate initial portfolio.
    async fn sync_portfolio_state(&mut self) -> Result<(), EngineError> {
        println!("[DEBUG] Fetching account balance and positions...");
        let balances = self.api_client.get_account_balance().await?;
        let positions = self.api_client.get_open_positions().await?;
        
        println!("[DEBUG] Found {} balance entries and {} open positions", balances.len(), positions.len());
        
        let mut portfolio = self.portfolio.lock().await;

        // Find the USDT balance to set our cash value.
        if let Some(usdt_balance) = balances.iter().find(|b| b.asset == "USDT") {
            portfolio.cash = usdt_balance.available_balance;
        } else {
            // Handle case where there is no USDT, for now, we'll just log it.
            println!("Warning: No USDT balance found in account.");
            portfolio.cash = "0".parse().unwrap();
        }

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
                println!("[DEBUG] Added position: {} {:?} {:.4} @ {:.2}", 
                    symbol, side, pos.position_amt.abs(), pos.entry_price);
            }
        }
        println!("[DEBUG] Total API positions: {}, Actual open positions: {}", total_positions, open_positions_count);

        Ok(())
    }

    /// Creates and stores `Bot` instances for all `enabled = true` bots in the config.
    fn populate_bots(&mut self) -> Result<(), EngineError> {
        for bot_config in &self.live_config.bots {
            if bot_config.enabled {
                println!("[DEBUG] Loading bot: {} with strategy: {:?}", bot_config.symbol, bot_config.strategy_id);
                let mut temp_config = self.base_config.clone();
                let strategy = crate::util::create_strategy_from_live_config(&mut temp_config, bot_config)?;
                
                let bot = Bot {
                    symbol: bot_config.symbol.clone(),
                    strategy,
                };
                self.bots.insert(bot_config.symbol.clone(), bot);
                println!("[DEBUG] Bot loaded successfully: {}", bot_config.symbol);
            } else {
                println!("[DEBUG] Skipping disabled bot: {}", bot_config.symbol);
            }
        }
        Ok(())
    }

    /// The main event loop for the live trading engine.
    pub async fn run(&mut self) -> Result<(), EngineError> {
        self.init().await?;

        let symbols: Vec<String> = self.bots.keys().cloned().collect();
        if symbols.is_empty() {
            println!("[WARN] No bots enabled in live.toml. Exiting.");
            return Ok(());
        }
        let interval = &self.base_config.backtest.interval;
        
        // The `live_mode` flag is now derived from the config, not passed in.
        let is_live = self.live_config.live_trading_enabled;
        let connector = LiveConnector::new(is_live);
        let mut kline_rx = connector.subscribe_to_klines(&symbols, interval)?;
        
        let reconciler = StateReconciler::new(
            Arc::clone(&self.portfolio),
            Arc::clone(&self.api_client),
            self.db_repo.clone(),
        );
        tokio::spawn(reconciler.start());
        
        println!("\n--- Engine is running. Subscribed to {} kline streams. Waiting for market data... ---", symbols.len());

        while let Some((symbol, kline)) = kline_rx.recv().await {
            if let Err(e) = self.process_kline(&symbol, &kline).await {
                eprintln!("[ERROR] Failed to process kline: {:?}", e);
            }
        }
        
        eprintln!("[ERROR] WebSocket stream ended unexpectedly.");
        Ok(())
    }

    /// The core logic for processing a single market event (Kline).
    async fn process_kline(&mut self, symbol: &str, kline: &core_types::Kline) -> Result<(), EngineError> {
        let bot = self.bots.get_mut(symbol).ok_or_else(|| EngineError::BotNotFound(symbol.to_string()))?;

        if let Some(signal) = bot.strategy.evaluate(kline)? {
            println!("\nSignal generated for {}: {:?} at price {}", bot.symbol, signal.order_request.side, kline.close);

            let portfolio_guard = self.portfolio.lock().await;
            let latest_equity = portfolio_guard.calculate_total_equity(&HashMap::from([(bot.symbol.clone(), kline.close)]))?;
            let portfolio_state = events::PortfolioState {
                timestamp: Utc::now(),
                cash: portfolio_guard.cash,
                total_value: latest_equity,
                positions: portfolio_guard.positions.values().cloned().collect(),
            };
            drop(portfolio_guard);

            let order_request = self.risk_manager.evaluate_signal(&signal, &portfolio_state, kline.close)?;
            println!("Risk assessment passed. Final Order: {:?} {} @ Market", order_request.quantity, order_request.symbol);

            // --- THE KEY CHANGE IS HERE ---
            // We now call the generic `executor`, not the `api_client`.
            match self.executor.execute(&order_request, kline).await {
                Ok(execution) => {
                    println!("SUCCESS: Execution confirmed: {:?}", execution);
                    // In a real system, we'd wait for a WebSocket confirmation before updating state.
                    // For now, we update our local portfolio optimistically.
                    let mut portfolio = self.portfolio.lock().await;
                    portfolio.update_with_execution(&execution)?;
                }
                Err(e) => {
                    eprintln!("ERROR: Failed to execute order for {}: {:?}", bot.symbol, e);
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
