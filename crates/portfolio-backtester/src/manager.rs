use crate::data_handler::{Event, MarketEvent};
use crate::error::PortfolioError;
use analytics::{AnalyticsEngine, PerformanceReport};

use configuration::Config;
use core_types::{Execution, Trade};
use executor::{Executor, Portfolio};
use indicatif::{ProgressBar, ProgressStyle};
use risk::RiskManager;
use rust_decimal::Decimal;
use std::collections::HashMap;
use strategies::Strategy;
use uuid::Uuid;

pub struct PortfolioManager {
    portfolio: Portfolio,
    risk_manager: Box<dyn RiskManager>,
    executor: Box<dyn Executor>,
    analytics_engine: AnalyticsEngine,
    strategies: HashMap<String, Box<dyn Strategy>>,
    base_config: Config,
}

impl PortfolioManager {
    pub fn new(
        base_config: Config,
        portfolio: Portfolio,
        risk_manager: Box<dyn RiskManager>,
        executor: Box<dyn Executor>,
        analytics_engine: AnalyticsEngine,
        strategies: HashMap<String, Box<dyn Strategy>>,
    ) -> Self {
        Self {
            base_config,
            portfolio,
            risk_manager,
            executor,
            analytics_engine,
            strategies,
        }
    }

    /// Runs the portfolio-level backtest by processing a pre-sorted event stream.
    pub async fn run(
        &mut self,
        events: Vec<Event>,
    ) -> Result<PerformanceReport, PortfolioError> {
        if events.is_empty() {
            return Err(PortfolioError::Data("Event stream is empty.".to_string()));
        }

        let mut equity_curve = Vec::with_capacity(events.len());
        let mut completed_trades = Vec::new();
        // We now need to track pending entries on a per-symbol basis.
        let mut pending_entries: HashMap<String, Execution> = HashMap::new();

        let progress_bar = ProgressBar::new(events.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=>-"),
        );

        // --- Main "Master Clock" Loop ---
        for event in events.iter() {
            let (event_time, symbol, kline) = match event {
                Event::Kline(MarketEvent { symbol, kline }) => (kline.close_time, symbol, kline),
            };

            // 1. Route the kline to the correct strategy for evaluation.
            if let Some(strategy) = self.strategies.get_mut(symbol) {
                let position_before = self.portfolio.get_position(symbol).cloned();

                if let Some(signal) = strategy.evaluate(kline).unwrap() { // Simplified error handling
                    // 2. Process the signal through the shared risk and execution components.
                    let total_equity = self.get_latest_equity()?;
                    
                    let order_request = self.risk_manager.evaluate_signal(
                        &signal,
                        &events::PortfolioState {
                            timestamp: event_time,
                            cash: self.portfolio.cash,
                            total_value: total_equity,
                            positions: self.portfolio.positions.values().cloned().collect(),
                        },
                        kline.close,
                    ).unwrap();

                    let execution = self.executor.execute(&order_request, kline, None, None).await.unwrap();
                    
                    // 3. Update the single, shared portfolio state.
                    self.portfolio.update_with_execution(&execution).unwrap();

                    // 4. Match trades for the specific symbol that was just traded.
                    let position_after = self.portfolio.get_position(symbol);
                    match (position_before, position_after) {
                        (None, Some(_)) => { pending_entries.insert(symbol.clone(), execution); }
                        (Some(_), None) => {
                            if let Some(entry_execution) = pending_entries.remove(symbol) {
                                completed_trades.push(Trade {
                                    trade_id: Uuid::new_v4(),
                                    symbol: symbol.clone(),
                                    entry_execution,
                                    exit_execution: execution,
                                });
                            }
                        }
                        _ => {} // Position was modified or no change
                    }
                }
            }
            
            // 5. Record the total portfolio equity at the end of each event.
            let equity = self.get_latest_equity()?;
            equity_curve.push((event_time, equity));
            progress_bar.inc(1);
        }

        progress_bar.finish_with_message("Portfolio simulation complete.");

        // 6. Generate the final, unified performance report.
        let report = self.analytics_engine.calculate(
            &completed_trades,
            &equity_curve,
            self.base_config.backtest.initial_capital,
            &self.base_config.backtest.interval,
        ).unwrap(); // Simplified error handling

        Ok(report)
    }

    /// Helper to get the most recent portfolio equity.
    /// In a live system, this would need to be more robust.
    fn get_latest_equity(&self) -> Result<Decimal, PortfolioError> {
        // This is a simplification. For a precise equity calculation, we would need
        // the last known price for *every* asset in the portfolio at this timestamp,
        // not just the one in the current event. For now, we assume cash is dominant
        // or that open positions are marked-to-market implicitly by other logic.
        // A full implementation would require a `latest_prices` HashMap here.
        Ok(self.portfolio.cash) // Simple approximation for now
    }
}