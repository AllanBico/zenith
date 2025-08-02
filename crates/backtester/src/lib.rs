use crate::error::BacktestError;
use analytics::{AnalyticsEngine, PerformanceReport};
use chrono::{DateTime, Utc};
use core_types::{Execution, Trade};
use database::DbRepository;
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
        // 1. Load Data
        let klines = self
            .db_repo
            .get_klines_by_date_range(&self.symbol, &self.interval, start_date, end_date)
            .await?;
        
        if klines.is_empty() {
            return Err(BacktestError::DataUnavailable);
        }

        // 2. Initialize State
        let mut equity_curve = Vec::with_capacity(klines.len());
        let mut completed_trades = Vec::new();
        let mut pending_entry: Option<Execution> = None;
        
        let progress_bar = ProgressBar::new(klines.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("=>-"),
        );

        // 3. Main Simulation Loop (Logic remains the same)
        for kline in klines.iter() {
            let position_before = self.portfolio.get_position(&self.symbol).cloned();

            if let Some(signal) = self.strategy.evaluate(kline)? {
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

                let execution = self.executor.execute(&order_request, kline).await?;
                self.portfolio.update_with_execution(&execution)?;
                
                let position_after = self.portfolio.get_position(&self.symbol);

                match (position_before, position_after) {
                    (None, Some(_)) => { pending_entry = Some(execution); }
                    (Some(_), None) => {
                        if let Some(entry_execution) = pending_entry.take() {
                             completed_trades.push(Trade {
                                trade_id: Uuid::new_v4(),
                                symbol: self.symbol.clone(),
                                entry_execution,
                                exit_execution: execution,
                            });
                        }
                    }
                    _ => {}
                }
            }

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