use crate::error::EngineError;
use api_client::ApiClient;
use database::DbRepository;
use executor::Portfolio;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use tracing;
use events::{WsMessage, LogLevel, LogMessage};
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

/// The "Source of Truth Auditor" for the live engine.
///
/// This component is designed to run in a concurrent background task. Its sole
/// responsibility is to periodically compare the engine's in-memory state
/// against the actual state reported by the exchange API.
pub struct StateReconciler {
    /// A shared, thread-safe reference to the live portfolio state.
    portfolio: Arc<Mutex<Portfolio>>,
    /// A shared, thread-safe reference to the API client for fetching exchange state.
    api_client: Arc<dyn ApiClient>,
    /// A database repository for logging discrepancies (future enhancement).
    db_repo: DbRepository,
    event_tx: broadcast::Sender<WsMessage>,
}

impl StateReconciler {
    /// Creates a new `StateReconciler`.
    ///
    /// It takes shared `Arc` pointers to the components it needs to interact with,
    /// allowing it to safely coexist with the main engine's event loop.
    pub fn new(
        portfolio: Arc<Mutex<Portfolio>>,
        api_client: Arc<dyn ApiClient>,
        db_repo: DbRepository,
        event_tx: broadcast::Sender<WsMessage>,
    ) -> Self {
        Self {
            portfolio,
            api_client,
            db_repo,
            event_tx,
        }
    }

    fn log(&self, level: LogLevel, message: &str) {
        let _ = self.event_tx.send(WsMessage::Log(LogMessage {
            timestamp: chrono::Utc::now(),
            level,
            message: message.to_string(),
        }));
    }

    pub async fn run_reconciliation(&self) -> Result<(), EngineError> {
        self.log(LogLevel::Info, "[RECONCILER] Running state check...");

        // 1. Concurrently fetch the ground truth from the exchange.
        let (balances_result, positions_result) = tokio::join!(
            self.api_client.get_account_balance(),
            self.api_client.get_open_positions()
        );
        let live_balances = balances_result?;
        let live_positions = positions_result?;

        // Map live positions into a HashMap for efficient lookup.
        let live_positions_map: HashMap<String, _> = live_positions
            .into_iter()
            .filter(|p| !p.position_amt.is_zero()) // Only care about open positions
            .map(|p| (p.symbol.clone(), p))
            .collect();
            
        // 2. Acquire a lock on our local portfolio state.
        let mut portfolio = self.portfolio.lock().await;

        // 3. Update Cash/Balance from exchange
        if let Some(usdt_balance) = live_balances.iter().find(|b| b.asset == "USDT") {
            let local_cash = portfolio.cash;
            let live_cash = usdt_balance.available_balance;
            
            // Always update to exchange balance (source of truth)
            if local_cash != live_cash {
                self.log(LogLevel::Info, &format!("Updating cash balance: Local: {} -> Exchange: {}", local_cash, live_cash));
                portfolio.cash = live_cash;
            }
        }

        // 4. Replace all local positions with exchange positions (source of truth)
        self.log(LogLevel::Info, &format!("Replacing local positions with exchange positions. Local count: {}, Exchange count: {}", 
            portfolio.positions.len(), live_positions_map.len()));
        
        // Clear all local positions and replace with exchange data
        portfolio.positions.clear();
        
        for (symbol, live_pos) in &live_positions_map {
            let side = if live_pos.position_amt.is_sign_positive() {
                core_types::OrderSide::Buy
            } else {
                core_types::OrderSide::Sell
            };
            
            let position = core_types::Position {
                position_id: uuid::Uuid::new_v4(), // Generate new ID for exchange position
                symbol: symbol.clone(),
                side,
                quantity: live_pos.position_amt.abs(),
                entry_price: live_pos.entry_price,
                unrealized_pnl: live_pos.un_realized_profit,
                last_updated: chrono::Utc::now(),
            };
            
            portfolio.positions.insert(symbol.clone(), position);
            self.log(LogLevel::Info, &format!("Updated position: {} {} @ {}", symbol, live_pos.position_amt, live_pos.entry_price));
        }

        // At the end of a successful reconciliation, broadcast the updated state.
        // This keeps the UI in sync even if no trades are happening.
        // Note: We already have the portfolio lock from above, so we can use it directly
        let state_msg = WsMessage::PortfolioState(events::PortfolioState {
            timestamp: chrono::Utc::now(),
            cash: portfolio.cash,
            positions: portfolio.positions.values().cloned().collect(),
            total_value: portfolio.cash, // Simplified for now - in a real system we'd calculate with current prices
        });
        let _ = self.event_tx.send(state_msg);

        self.log(LogLevel::Info, "[RECONCILER] Reconciliation check complete.");
        Ok(())
    }

    pub async fn start(self) {
        tracing::info!("[RECONCILER] Starting continuous state reconciliation task...");
        // Create a timer that ticks every 30 seconds.
        let mut timer = interval(Duration::from_secs(30));

        // Enter the infinite loop. This task will run as long as the engine is alive.
        loop {
            // Wait for the timer to tick. The first tick is immediate.
            timer.tick().await;

            // Execute the core reconciliation logic.
            if let Err(e) = self.run_reconciliation().await {
                self.log(LogLevel::Error, &format!("An error occurred during the reconciliation check: {:?}", e));
            }
        }
    }
}