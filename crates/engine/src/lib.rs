use crate::error::EngineError;
use api_client::ApiClient;
use configuration::{Config, LiveConfig};
use database::DbRepository;
use executor::Portfolio;
use risk::{SimpleRiskManager, RiskManager};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use strategies::Strategy;
use tokio::sync::Mutex;
use uuid::Uuid;
use chrono::Utc;

/// Rounds quantity to the appropriate precision for the given symbol.
/// This is a simple implementation - in production, you'd fetch this from exchange info.
fn round_quantity_to_precision(symbol: &str, quantity: Decimal) -> Decimal {
    // Simple precision mapping for common symbols
    // In a real implementation, this would come from exchange info API
    let precision = match symbol {
        "BTCUSDT" => 3,  // BTC precision is 0.001
        "ETHUSDT" => 3,  // ETH precision is 0.001
        _ => 2,          // Default to 2 decimal places
    };
    
    // Round to the specified precision
    let scale = Decimal::from(10_i64.pow(precision as u32));
    (quantity * scale).round() / scale
}

/// A wrapper for Kline data that includes the symbol information.
/// This is needed because the Kline struct doesn't contain symbol information.
#[derive(Debug, Clone)]
pub struct KlineWithSymbol {
    pub symbol: String,
    pub kline: core_types::Kline,
}

pub mod error;
// pub mod util;
/// A container for the components related to a single trading instrument.
pub struct Bot {
    pub symbol: String,
    pub strategy: Box<dyn Strategy>,
}

/// The central orchestrator for the live trading application.
///
/// This struct holds all the shared components and is responsible for the main
/// event loop that drives the trading logic in real-time.
pub struct Engine {
    // --- Configuration ---
    live_config: LiveConfig,
    base_config: Config,

    // --- Shared, Thread-Safe Components ---
    // `Arc` allows multiple parts of the engine to safely share ownership of these components.
    api_client: Arc<dyn ApiClient>,
    db_repo: DbRepository,
    // `Mutex` ensures that only one task can access the portfolio state at a time, preventing race conditions.
    portfolio: Arc<Mutex<Portfolio>>,
    risk_manager: SimpleRiskManager,

    // --- Bot Management ---
    // A map from a symbol (e.g., "BTCUSDT") to its corresponding Bot instance.
    bots: HashMap<String, Bot>,
}


impl Engine {
    /// Creates a new `Engine` instance with all its required components.
    pub fn new(
        live_config: LiveConfig,
        base_config: Config,
        api_client: Arc<dyn ApiClient>,
        db_repo: DbRepository,
    ) -> Result<Self, EngineError> {
        // The portfolio is initialized empty. It will be populated by the `init` method.
        let portfolio = Arc::new(Mutex::new(Portfolio::new(
            base_config.backtest.initial_capital, // Start with this as a placeholder
        )));

        Ok(Self {
            live_config,
            base_config: base_config.clone(),
            api_client,
            db_repo,
            portfolio,
            risk_manager: SimpleRiskManager::new(base_config.risk_management.clone())?,
            bots: HashMap::new(),
        })
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
            if pos.position_amt != Decimal::ZERO {
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
        // 1. Perform all startup initializations.
        self.init().await?;

        // 2. Set up the live data subscription.
        let symbols: Vec<String> = self.bots.keys().cloned().collect();
        let interval = &self.live_config.interval; // Use the interval from live.toml
        
        let connector = api_client::LiveConnector::new(self.live_config.live_trading_enabled);
        let mut kline_rx = connector.subscribe_to_klines(&symbols, interval)?;
        
        println!("\n--- Engine is running. Subscribed to {} kline streams. Waiting for market data... ---", symbols.len());

        // 3. Enter the main event loop.
        while let Some((symbol, kline)) = kline_rx.recv().await {
            // Create a wrapper with the actual symbol from the WebSocket
            let kline_with_symbol = KlineWithSymbol {
                symbol,
                kline,
            };
            
            // Every time a new kline is received, process it.
            if let Err(e) = self.process_kline(kline_with_symbol).await {
                eprintln!("[ERROR] Failed to process kline: {:?}", e);
            }
        }
        
        eprintln!("[ERROR] WebSocket stream ended unexpectedly.");
        Ok(())
    }

    /// The core logic for processing a single market event (Kline).
    async fn process_kline(&mut self, kline_with_symbol: KlineWithSymbol) -> Result<(), EngineError> {
        // Debug: Print received kline data
        println!("[DEBUG] Received kline for {}: O:{:.2} H:{:.2} L:{:.2} C:{:.2} V:{:.2} at {}", 
            kline_with_symbol.symbol,
            kline_with_symbol.kline.open,
            kline_with_symbol.kline.high,
            kline_with_symbol.kline.low,
            kline_with_symbol.kline.close,
            kline_with_symbol.kline.volume,
            kline_with_symbol.kline.open_time.format("%H:%M:%S")
        );

        // 1. Route: Find the bot responsible for this symbol.
        let bot = self.bots.get_mut(&kline_with_symbol.symbol)
            .ok_or_else(|| EngineError::BotNotFound(kline_with_symbol.symbol.clone()))?;

        println!("[DEBUG] Processing kline for bot: {} with strategy: {:?}", 
            bot.symbol, std::any::type_name_of_val(&*bot.strategy));

        // 2. Evaluate: Let the bot's strategy evaluate the new data.
        println!("[DEBUG] Evaluating strategy for {} at price {:.2}", bot.symbol, kline_with_symbol.kline.close);
        let signal_result = bot.strategy.evaluate(&kline_with_symbol.kline);
        
        match signal_result {
            Ok(Some(signal)) => {
                println!("[SIGNAL] Generated for {}: {:?} at price {:.2}", bot.symbol, signal.order_request.side, kline_with_symbol.kline.close);

                            // 3. Risk Management: Get current portfolio state and evaluate risk.
            let portfolio_guard = self.portfolio.lock().await;
            
            // Debug: Log portfolio state
            println!("[DEBUG] Portfolio has {} positions: {:?}", 
                portfolio_guard.positions.len(), 
                portfolio_guard.positions.keys().collect::<Vec<_>>()
            );
            
            // Build market prices for all symbols in the portfolio
            let mut market_prices = HashMap::new();
            
            // Add the current symbol's price
            market_prices.insert(bot.symbol.clone(), kline_with_symbol.kline.close);
            
            // For other symbols in the portfolio, we need to get their current prices
            // For now, we'll use a placeholder approach - in a real system, you'd have
            // a separate task updating mark prices for all symbols
            for (symbol, _) in &portfolio_guard.positions {
                if symbol != &bot.symbol {
                    // For now, use the last known price or a placeholder
                    // In a real implementation, you'd fetch this from a price cache
                    market_prices.insert(symbol.clone(), kline_with_symbol.kline.close); // Placeholder
                    println!("[DEBUG] Using placeholder price for {}: {:.2}", symbol, kline_with_symbol.kline.close);
                }
            }
            
            let latest_equity = portfolio_guard.calculate_total_equity(&market_prices)?;
                
                let portfolio_state = events::PortfolioState {
                    timestamp: Utc::now(),
                    cash: portfolio_guard.cash,
                    total_value: latest_equity,
                    positions: portfolio_guard.positions.values().cloned().collect(),
                };
                // Drop the lock as soon as we're done reading the state.
                drop(portfolio_guard);

                let mut order_request = self.risk_manager.evaluate_signal(&signal, &portfolio_state, kline_with_symbol.kline.close)?;
                
                // Fix precision issue by rounding to appropriate decimal places
                order_request.quantity = round_quantity_to_precision(&order_request.symbol, order_request.quantity);
                
                // Add position side for hedge mode
                order_request.position_side = Some(core_types::enums::PositionSide::from_order_side(order_request.side));
                
                println!("[RISK] Assessment passed. Final Order: {:?} {} @ Market", order_request.quantity, order_request.symbol);

                // 4. Execution: Place the final, risk-managed order.
                match self.api_client.place_order(&order_request).await {
                    Ok(response) => {
                        println!("[SUCCESS] Order placed successfully. Response: {:?}", response.status);
                        // Here we would ideally save the confirmed order to the database.
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Failed to place order for {}: {:?}", bot.symbol, e);
                        // Here we would send a Telegram alert about the failed order.
                    }
                }
            },
            Ok(None) => {
                println!("[DEBUG] No signal generated for {} at price {:.2}", bot.symbol, kline_with_symbol.kline.close);
            },
            Err(e) => {
                eprintln!("[ERROR] Strategy evaluation failed for {}: {:?}", bot.symbol, e);
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
