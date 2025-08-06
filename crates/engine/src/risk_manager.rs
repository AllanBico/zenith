use crate::error::EngineError;
use configuration::settings::GlobalRiskConfig;
use core_types::{Trade, OrderSide};
use events::{LogLevel, WsMessage, LogMessage};
use executor::Portfolio;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tokio::time::sleep;
use chrono::Utc;

/// The "Portfolio Pit Boss" - a concurrent, stateful supervisor.
///
/// This component runs in the background, monitoring the overall health of the
/// trading portfolio and enforcing a set of global, non-negotiable risk rules.
pub struct GlobalRiskManager {
    // --- Configuration ---
    config: GlobalRiskConfig,

    // --- Shared State (with the LiveEngine) ---
    /// A shared reference to the live portfolio state.
    portfolio: Arc<Mutex<Portfolio>>,
    /// A shared map of trading flags that this manager controls.
    trading_enabled_flags: Arc<Mutex<HashMap<String, bool>>>,
    /// The broadcast sender for sending alerts.
    event_tx: broadcast::Sender<WsMessage>,

    // --- Internal State ---
    /// Tracks the peak equity reached during the current trading session.
    peak_equity_today: Mutex<Decimal>,
    /// Tracks the number of consecutive losses for each individual bot.
    consecutive_losses: Mutex<HashMap<String, u32>>,
}

impl GlobalRiskManager {
    /// Creates a new `GlobalRiskManager`.
    pub fn new(
        config: GlobalRiskConfig,
        portfolio: Arc<Mutex<Portfolio>>,
        trading_enabled_flags: Arc<Mutex<HashMap<String, bool>>>,
        event_tx: broadcast::Sender<WsMessage>,
        initial_equity: Decimal,
    ) -> Self {
        Self {
            config,
            portfolio,
            trading_enabled_flags,
            event_tx,
            peak_equity_today: Mutex::new(initial_equity),
            consecutive_losses: Mutex::new(HashMap::new()),
        }
    }

    /// This is the primary event handler for the risk manager.
    /// It should be called by the `LiveEngine` after every trade is closed.
    pub async fn on_trade_closed(&self, trade: &Trade) -> Result<(), EngineError> {
        // 1. Calculate the P&L of the closed trade.
        let pnl = match trade.entry_execution.side {
            OrderSide::Buy => {
                (trade.exit_execution.price - trade.entry_execution.price)
                    * trade.exit_execution.quantity
            }
            OrderSide::Sell => {
                (trade.entry_execution.price - trade.exit_execution.price)
                    * trade.exit_execution.quantity
            }
        };

        // 2. Update the consecutive loss counter for the specific symbol.
        let mut losses = self.consecutive_losses.lock().await;
        let loss_counter = losses.entry(trade.symbol.clone()).or_insert(0);

        if pnl.is_sign_negative() {
            *loss_counter += 1;
            self.log(
                LogLevel::Warn,
                &format!(
                    "Consecutive loss streak for {} is now {}.",
                    trade.symbol, *loss_counter
                ),
            );
        } else {
            // Any winning trade resets the counter.
            *loss_counter = 0;
        }

        let current_streak = *loss_counter;
        drop(losses); // Release the lock before the next await call

        // 3. Check if the consecutive loss limit has been breached.
        if current_streak >= self.config.max_consecutive_losses {
            self.log(
                LogLevel::Error,
                &format!(
                    "CRITICAL: {} has hit the max consecutive loss limit of {}. Halting bot.",
                    trade.symbol, self.config.max_consecutive_losses
                ),
            );
            self.halt_bot(&trade.symbol).await; // This will be implemented in Task 4
        }
        
        // 4. After every trade, check the portfolio-wide drawdown.
        self.check_daily_drawdown().await?;

        Ok(())
    }

    /// Checks the current portfolio equity against the session's peak to enforce max drawdown.
    async fn check_daily_drawdown(&self) -> Result<(), EngineError> {
        let current_equity = {
            let portfolio = self.portfolio.lock().await;
            // A full implementation would need to mark-to-market all open positions here.
            // For now, we use a simplified equity measure.
            portfolio.cash // Simplified equity for now
        };

        let mut peak_equity = self.peak_equity_today.lock().await;

        // Update the peak equity if we've reached a new high.
        if current_equity > *peak_equity {
            *peak_equity = current_equity;
        }

        // Calculate the current drawdown percentage.
        let drawdown = (*peak_equity - current_equity) / *peak_equity;

        if drawdown >= self.config.max_daily_drawdown_pct {
            self.log(
                LogLevel::Error,
                &format!(
                    "CRITICAL: Portfolio has breached the max daily drawdown limit of {:.2}%. Halting all trading.",
                    self.config.max_daily_drawdown_pct * Decimal::from(100)
                )
            );
            self.halt_all_bots().await; // This will be implemented in Task 4
        }
        
        Ok(())
    }

    // A placeholder for the log helper, to be fully implemented with others
    fn log(&self, level: LogLevel, message: &str) {
        let msg = WsMessage::Log(LogMessage {
            timestamp: Utc::now(),
            level,
            message: message.to_string(),
        });
        let _ = self.event_tx.send(msg);
        // In a real implementation, you might want to handle the send error
    }

    /// Disables trading for a single bot and starts the cool-down timer.
    async fn halt_bot(&self, symbol: &str) {
        // 1. Lock the shared trading flags and disable the specific bot.
        let mut flags = self.trading_enabled_flags.lock().await;
        flags.insert(symbol.to_string(), false);

        self.log(
            LogLevel::Error,
            &format!(
                "BOT HALTED: Trading for {} has been disabled due to risk limits.",
                symbol
            ),
        );

        // 2. Spawn a separate, concurrent task for the cool-down timer.
        let symbol_clone = symbol.to_string();
        let flags_clone = Arc::clone(&self.trading_enabled_flags);
        let event_tx_clone = self.event_tx.clone();
        let cooldown_hours = self.config.bot_cooldown_hours;
        let cooldown_duration = Duration::from_secs(cooldown_hours * 3600);

        tokio::spawn(async move {
            tracing::warn!(
                symbol = %symbol_clone,
                cooldown_hours = %cooldown_hours,
                "Bot entered cool-down period."
            );

            // 3. Wait for the configured duration.
            sleep(cooldown_duration).await;

            // 4. Re-enable the bot after the timer expires.
            let mut flags = flags_clone.lock().await;
            flags.insert(symbol_clone.clone(), true);
            
            // Log and broadcast the re-enabling event.
            let log_msg = WsMessage::Log(LogMessage {
                timestamp: Utc::now(),
                level: LogLevel::Info,
                message: format!("BOT RE-ENABLED: Trading for {} has been automatically re-enabled after cool-down.", symbol_clone),
            });
            let _ = event_tx_clone.send(log_msg);
            tracing::info!(symbol = %symbol_clone, "Bot has been re-enabled after cool-down.");
        });
    }

    /// Disables trading for ALL bots in the system.
    async fn halt_all_bots(&self) {
        let mut flags = self.trading_enabled_flags.lock().await;
        for (symbol, is_enabled) in flags.iter_mut() {
            if *is_enabled {
                *is_enabled = false;
                self.log(
                    LogLevel::Error,
                    &format!("PORTFOLIO HALTED: Trading for {} disabled due to portfolio drawdown.", symbol),
                );
            }
        }
    }
}