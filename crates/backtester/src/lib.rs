use crate::error::BacktestError;
use analytics::{AnalyticsEngine, PerformanceReport};
use chrono::{DateTime, Utc};
use core_types::{Execution, OrderSide, Trade};
use database::DbRepository;
use executor::{Executor, Portfolio};
use indicatif::{ProgressBar, ProgressStyle};
use risk::RiskManager;
use rust_decimal::Decimal;
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
        let mut pending_entries = HashMap::<Uuid, Execution>::new(); // Maps order ID to entry execution
        
        let progress_bar = ProgressBar::new(klines.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("=>-"),
        );

        // 3. Main Simulation Loop
        for kline in klines.iter() {
            // A. Evaluate strategy
            if let Some(signal) = self.strategy.evaluate(kline)? {
                // B. Evaluate risk
                let order_request = self.risk_manager.evaluate_signal(
                    &signal, 
                    // This is a simplification; needs a PortfolioState object
                    &events::PortfolioState { 
                        timestamp: kline.close_time,
                        cash: self.portfolio.cash,
                        total_value: self.portfolio.calculate_total_equity(&HashMap::from([(self.symbol.clone(), kline.close)]))?,
                        positions: self.portfolio.positions.values().cloned().collect()
                    },
                    kline.close
                )?;

                // C. Execute order
                let execution = self.executor.execute(&order_request, kline).await?;

                // D. Update portfolio state
                self.portfolio.update_with_execution(&execution)?;
                
                // E. Match trades
                let position = self.portfolio.get_position(&self.symbol);
                if let Some(pos) = position {
                    // Position exists or was just opened/increased
                    if execution.side == pos.side {
                        pending_entries.insert(execution.client_order_id, execution);
                    }
                } else {
                    // Position was just closed
                    if let Some(entry_execution) = pending_entries.remove(&execution.client_order_id) {
                         completed_trades.push(Trade {
                            trade_id: Uuid::new_v4(),
                            symbol: self.symbol.clone(),
                            entry_execution,
                            exit_execution: execution,
                        });
                    }
                }
            }

            // F. Record equity
            let market_prices = HashMap::from([(self.symbol.clone(), kline.close)]);
            let total_equity = self.portfolio.calculate_total_equity(&market_prices)?;
            equity_curve.push((kline.close_time, total_equity));
            progress_bar.inc(1);
        }

        progress_bar.finish_with_message("Simulation complete.");

        // 4. Generate Final Report
        let initial_capital = equity_curve.first().map_or(Decimal::ZERO, |&(_, v)| v);
        let report = self.analytics_engine.calculate(&completed_trades, &equity_curve, initial_capital)?;
        
        Ok(report)
    }
}