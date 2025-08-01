use crate::error::BacktestError;
use analytics::{AnalyticsEngine, PerformanceReport};
use chrono::{DateTime, Utc};
use core_types::{Execution, Trade};
use database::DbRepository;
use executor::{Executor, Portfolio};
use indicatif::{ProgressBar, ProgressStyle};
use risk::RiskManager;
use std::collections::HashMap;
use strategies::Strategy;
use uuid::Uuid;

pub mod error;

pub struct Backtester {
    portfolio: Portfolio,
    strategy: Box<dyn Strategy>,
    risk_manager: Box<dyn RiskManager>,
    executor: Box<dyn Executor>,
    analytics_engine: AnalyticsEngine,
    db_repo: DbRepository,
    symbol: String,
    interval: String,
}

impl Backtester {
    pub fn new(
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

    pub async fn run(
        &mut self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<PerformanceReport, BacktestError> {
        // 1. Load Data
        let klines = self.db_repo
            .get_klines_by_date_range(&self.symbol, &self.interval, start_date, end_date)
            .await?;
        
        if klines.is_empty() {
            return Err(BacktestError::DataUnavailable);
        }

        // 2. Initialize State
        let mut equity_curve = Vec::with_capacity(klines.len());
        let mut completed_trades = Vec::new();
        // CORRECTED: We only need to store one potential open entry at a time for simple strategies.
        let mut pending_entry: Option<Execution> = None;
        
        let progress_bar = ProgressBar::new(klines.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("=>-"),
        );

        // 3. Main Simulation Loop
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
                
                // --- CORRECTED TRADE MATCHING LOGIC ---
                let position_after = self.portfolio.get_position(&self.symbol);

                match (position_before, position_after) {
                    // Case 1: Opened a new position
                    (None, Some(_)) => {
                        pending_entry = Some(execution);
                    }
                    // Case 2: Closed an existing position
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
                    // Case 3: Position size changed (scaling in/out) or no change.
                    // This simple logic doesn't handle scaling in/out, but it correctly
                    // handles simple open/close, which fixes the "zero trades" bug.
                    _ => {}
                }
            }

            let market_prices = HashMap::from([(self.symbol.clone(), kline.close)]);
            let total_equity = self.portfolio.calculate_total_equity(&market_prices)?;
            equity_curve.push((kline.close_time, total_equity));
            progress_bar.inc(1);
        }

        progress_bar.finish_with_message("Simulation complete.");

        progress_bar.finish_with_message("Simulation complete.");

        // 4. Generate Final Report
        let initial_capital = self.portfolio.cash; // More accurate initial capital
        let report = self.analytics_engine.calculate(
            &completed_trades,
            &equity_curve,
            initial_capital,
            &self.interval,
        )?;
        
        Ok(report)
    }
}