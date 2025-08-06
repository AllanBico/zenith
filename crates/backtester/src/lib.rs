use crate::error::BacktestError;
use analytics::{AnalyticsEngine, PerformanceReport};
use chrono::{DateTime, Utc};
use configuration::Config; // We need the full config for stop-loss pct
use core_types::{Execution, OrderRequest, OrderSide, OrderType, Signal, Trade};
use database::DbRepository;
use events; // For PortfolioState
use executor::{Executor, Portfolio};
use indicatif::{ProgressBar, ProgressStyle};
use risk::RiskManager;
use rust_decimal::Decimal;
use std::collections::HashMap;
use strategies::Strategy;
use uuid::Uuid;

pub mod error;

/// The main backtesting engine.
///
/// This struct now also handles the persistence of its own results.
pub struct Backtester {
    // --- Context ---
    run_id: Uuid, // The unique ID for this specific run, used as a foreign key.
    symbol: String,
    interval: String,
    config: Config, // Store the full config for stop-loss access
    // --- Components ---
    portfolio: Portfolio,
    strategy: Box<dyn Strategy>,
    risk_manager: Box<dyn RiskManager>,
    executor: Box<dyn Executor>,
    analytics_engine: AnalyticsEngine,
    db_repo: DbRepository,
}

impl Backtester {
    /// Constructs a new `Backtester`, now requiring a `run_id`.
    pub fn new(
        run_id: Uuid, // <-- ADDED
        symbol: String,
        interval: String,
        config: Config, // Pass in the full config
        portfolio: Portfolio,
        strategy: Box<dyn Strategy>,
        risk_manager: Box<dyn RiskManager>,
        executor: Box<dyn Executor>,
        analytics_engine: AnalyticsEngine,
        db_repo: DbRepository,
    ) -> Self {
        Self {
            run_id, // <-- ADDED
            symbol,
            interval,
            config, // Store the full config
            portfolio,
            strategy,
            risk_manager,
            executor,
            analytics_engine,
            db_repo,
        }
    }

    /// Runs the simulation and saves all results to the database upon completion.
    pub async fn run(
        &mut self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<PerformanceReport, BacktestError> {
        let klines = self.db_repo.get_klines_by_date_range(&self.symbol, &self.interval, start_date, end_date).await?;
        if klines.is_empty() { return Err(BacktestError::DataUnavailable); }

        let mut equity_curve = Vec::with_capacity(klines.len());
        let mut completed_trades = Vec::new();
        let mut pending_entry: Option<Execution> = None;
        let mut stop_loss_price: Option<Decimal> = None; // Track the stop-loss for the open position

        let progress_bar = ProgressBar::new(klines.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );

        for kline in klines.iter() {
            let mut signal_from_strategy: Option<Signal> = None;

            // --- 1. STOP-LOSS CHECK (NEW LOGIC) ---
            // Check for stop-loss triggers *before* evaluating the strategy.
            if let Some(position) = self.portfolio.get_position(&self.symbol) {
                if let Some(sl_price) = stop_loss_price {
                    let should_stop = match position.side {
                        OrderSide::Buy => kline.low <= sl_price,
                        OrderSide::Sell => kline.high >= sl_price,
                    };

                    if should_stop {
                        // Create a synthetic "stop-loss signal" to close the position.
                        let close_signal = Signal {
                            signal_id: Uuid::new_v4(),
                            timestamp: kline.close_time,
                            confidence: "1.0".parse().unwrap(),
                            order_request: OrderRequest {
                                client_order_id: Uuid::new_v4(), // New order ID for the exit
                                symbol: self.symbol.clone(),
                                side: if position.side == OrderSide::Buy { OrderSide::Sell } else { OrderSide::Buy },
                                order_type: OrderType::Market,
                                quantity: position.quantity, // Close the full position
                                price: Some(sl_price), // Execute at the SL price for realism
                                position_side: None, // Will be set by engine
                            },
                        };
                        
                        // Execute the stop-loss order
                        let execution = self.executor.execute(&close_signal.order_request, kline, None, None).await?;
                        self.portfolio.update_with_execution(&execution)?;
                        
                        // Match the trade
                        if let Some(entry_execution) = pending_entry.take() {
                            completed_trades.push(Trade {
                                trade_id: Uuid::new_v4(),
                                symbol: self.symbol.clone(),
                                entry_execution,
                                exit_execution: execution,
                            });
                        }
                        stop_loss_price = None; // Clear the stop-loss
                        continue; // Skip strategy evaluation for this bar, as we were stopped out.
                    }
                }
            } else {
                 // If there's no position, there should be no stop loss. Clean up.
                 stop_loss_price = None;
            }

            // --- 2. STRATEGY EVALUATION ---
            signal_from_strategy = self.strategy.evaluate(kline)?;

            // --- 3. SIGNAL PROCESSING ---
            if let Some(signal) = signal_from_strategy {
                let position_before = self.portfolio.get_position(&self.symbol).cloned();
                
                let total_equity = self.portfolio.calculate_total_equity(&HashMap::from([(self.symbol.clone(), kline.close)]))?;
                
                let order_request = self.risk_manager.evaluate_signal(
                    &signal,
                    &events::PortfolioState { 
                        timestamp: kline.close_time,
                        cash: self.portfolio.cash,
                        total_value: total_equity,
                        positions: self.portfolio.positions.values().cloned().collect()
                    },
                    kline.close
                )?;

                let execution = self.executor.execute(&order_request, kline, None, None).await?;
                self.portfolio.update_with_execution(&execution)?;
                
                let position_after = self.portfolio.get_position(&self.symbol);

                match (position_before, position_after) {
                    (None, Some(pos_after)) => { // Opened a new position
                        pending_entry = Some(execution);
                        // SET THE STOP-LOSS PRICE
                        let sl_pct = self.config.risk_management.stop_loss_pct;
                        stop_loss_price = Some(match pos_after.side {
                            OrderSide::Buy => pos_after.entry_price * (Decimal::ONE - sl_pct),
                            OrderSide::Sell => pos_after.entry_price * (Decimal::ONE + sl_pct),
                        });
                    }
                    (Some(_), None) => { // Closed an existing position
                        if let Some(entry_execution) = pending_entry.take() {
                            completed_trades.push(Trade {
                                trade_id: Uuid::new_v4(),
                                symbol: self.symbol.clone(),
                                entry_execution,
                                exit_execution: execution,
                            });
                        }
                        stop_loss_price = None; // Clear SL on close
                    }
                    _ => {}
                }
            }

            // --- 4. RECORD EQUITY ---
            let market_prices = HashMap::from([(self.symbol.clone(), kline.close)]);
            let total_equity = self.portfolio.calculate_total_equity(&market_prices)?;
            equity_curve.push((kline.close_time, total_equity));
            progress_bar.inc(1);
        }

        progress_bar.finish_with_message("Simulation complete. Analyzing and saving results...");

        // 4. Generate Final Report
        let initial_capital = self.portfolio.cash + self.portfolio.positions.values().map(|p| p.entry_price * p.quantity).sum::<Decimal>();
        let report = self.analytics_engine.calculate(
            &completed_trades,
            &equity_curve,
            initial_capital,
            &self.interval,
        )?;
        
        // --- 5. Persist All Results to Database ---
        self.db_repo.save_performance_report(self.run_id, &report).await?;
        self.db_repo.save_trades(self.run_id, &completed_trades).await?;
        self.db_repo.save_equity_curve(self.run_id, &equity_curve).await?;
        
        progress_bar.finish_with_message("Results saved successfully.");

        Ok(report)
    }
}