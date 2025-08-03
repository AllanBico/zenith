use crate::error::EngineError;
use api_client::ApiClient;
use database::DbRepository;
use executor::Portfolio;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
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
    ) -> Self {
        Self {
            portfolio,
            api_client,
            db_repo,
        }
    }

    pub async fn run_reconciliation(&self) -> Result<(), EngineError> {
        println!("[RECONCILER] Running state reconciliation check...");

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
        let portfolio = self.portfolio.lock().await;

        // 3. Compare Cash/Balance
        // For now, we'll just check for the USDT balance.
        if let Some(usdt_balance) = live_balances.iter().find(|b| b.asset == "USDT") {
            let local_cash = portfolio.cash;
            let live_cash = usdt_balance.available_balance;
            // Use a small tolerance for dust amounts
            if (local_cash - live_cash).abs() > Decimal::from_str_exact("0.01").unwrap() {
                println!("[RECONCILER-WARN] Cash discrepancy found! Local: {:.4}, Exchange: {:.4}", local_cash, live_cash);
                // In the future: self.db_repo.log_discrepancy(...)
            }
        }

        // 4. Compare Positions
        let mut checked_symbols = std::collections::HashSet::new();

        for (symbol, local_pos) in &portfolio.positions {
            checked_symbols.insert(symbol.clone());
            
            if let Some(live_pos) = live_positions_map.get(symbol) {
                // Position exists in both states; compare them.
                let local_qty = if local_pos.side == core_types::OrderSide::Buy { local_pos.quantity } else { -local_pos.quantity };
                let live_qty = live_pos.position_amt;

                if local_qty != live_qty {
                    println!("[RECONCILER-ERROR] Quantity discrepancy for {}! Local: {}, Exchange: {}", symbol, local_qty, live_qty);
                }

                if (local_pos.entry_price - live_pos.entry_price).abs() > Decimal::from_str_exact("0.0001").unwrap() {
                     println!("[RECONCILER-WARN] Entry price discrepancy for {}! Local: {}, Exchange: {}", symbol, local_pos.entry_price, live_pos.entry_price);
                }
            } else {
                // Position exists locally but NOT on the exchange (a "ghost" position).
                println!("[RECONCILER-CRITICAL] Ghost position found! Local state shows a position for {}, but none exists on the exchange.", symbol);
            }
        }

        // 5. Check for positions that exist on the exchange but NOT locally.
        for (symbol, live_pos) in &live_positions_map {
            if !checked_symbols.contains(symbol) {
                 println!("[RECONCILER-CRITICAL] Un-tracked position found! Exchange shows a position for {} ({}), but it does not exist in local state.", symbol, live_pos.position_amt);
            }
        }

        println!("[RECONCILER] Reconciliation check complete.");
        Ok(())
    }

    pub async fn start(self) {
        println!("[RECONCILER] Starting continuous state reconciliation task...");
        // Create a timer that ticks every 30 seconds.
        let mut timer = interval(Duration::from_secs(30));

        // Enter the infinite loop. This task will run as long as the engine is alive.
        loop {
            // Wait for the timer to tick. The first tick is immediate.
            timer.tick().await;

            // Execute the core reconciliation logic.
            if let Err(e) = self.run_reconciliation().await {
                eprintln!("[RECONCILER-ERROR] An error occurred during the reconciliation check: {:?}", e);
            }
        }
    }
}