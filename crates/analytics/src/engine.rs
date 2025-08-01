use crate::error::AnalyticsError;
use crate::report::PerformanceReport;
use chrono::{DateTime, Duration, Utc};
use core_types::{OrderSide, Trade};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// A stateless calculator for deriving performance metrics from trading activity.
#[derive(Debug, Default)]
pub struct AnalyticsEngine {}

impl AnalyticsEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// The main entry point for calculating performance metrics.
    /// The `interval` string is required to correctly annualize the Sharpe Ratio.
    pub fn calculate(
        &self,
        trades: &[Trade],
        equity_curve: &[(DateTime<Utc>, Decimal)],
        initial_capital: Decimal,
        interval: &str, // <-- FIX: Added interval for annualization
    ) -> Result<PerformanceReport, AnalyticsError> {
        if trades.is_empty() || equity_curve.len() < 2 {
            return Ok(PerformanceReport::new());
        }

        // First, calculate all metrics that can be computed independently
        let mut profitability_report = PerformanceReport::new();
        self.calculate_profitability(trades, initial_capital, &mut profitability_report)?;
        
        let mut drawdown_report = PerformanceReport::new();
        self.calculate_drawdown(equity_curve, &mut drawdown_report)?;
        
        let mut time_metrics_report = PerformanceReport::new();
        self.calculate_time_metrics(trades, &mut time_metrics_report)?;
        
        // Combine the results into a single report
        let mut report = PerformanceReport {
            // Copy all fields from profitability_report
            total_net_profit: profitability_report.total_net_profit,
            gross_profit: profitability_report.gross_profit,
            gross_loss: profitability_report.gross_loss,
            profit_factor: profitability_report.profit_factor,
            total_return_pct: profitability_report.total_return_pct,
            max_drawdown: drawdown_report.max_drawdown,
            max_drawdown_pct: drawdown_report.max_drawdown_pct,
            sharpe_ratio: None,  // Will be set by calculate_ratios
            calmar_ratio: None,   // Will be set by calculate_ratios
            total_trades: profitability_report.total_trades,
            winning_trades: profitability_report.winning_trades,
            losing_trades: profitability_report.losing_trades,
            win_rate_pct: profitability_report.win_rate_pct,
            average_win: profitability_report.average_win,
            average_loss: profitability_report.average_loss,
            payoff_ratio: profitability_report.payoff_ratio,
            average_holding_period: time_metrics_report.average_holding_period,
        };
        
        // Extract the fields needed for calculate_ratios
        let total_return_pct = report.total_return_pct;
        let max_drawdown_pct = report.max_drawdown_pct;
        
        // Now calculate ratios using the extracted values
        self.calculate_ratios(
            equity_curve, 
            interval, 
            total_return_pct, 
            max_drawdown_pct, 
            &mut report
        )?;

        Ok(report)
    }

    /// Calculates all profitability-related metrics.
    fn calculate_profitability(
        &self,
        trades: &[Trade],
        initial_capital: Decimal,
        report: &mut PerformanceReport,
    ) -> Result<(), AnalyticsError> {
        report.total_trades = trades.len();

        for trade in trades {
            // --- FIX #1: Correctly calculate PnL based on trade side ---
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
            // --- END FIX #1 ---

            report.total_net_profit += pnl;

            if pnl.is_sign_positive() {
                report.gross_profit += pnl;
                report.winning_trades += 1;
            } else {
                report.gross_loss += pnl.abs();
                report.losing_trades += 1;
            }
        }

        if report.gross_loss > Decimal::ZERO {
            report.profit_factor = Some(report.gross_profit / report.gross_loss);
        }

        if report.total_trades > 0 {
            report.win_rate_pct = Some(
                (Decimal::from(report.winning_trades) / Decimal::from(report.total_trades))
                    * Decimal::from(100),
            );
        }

        if report.winning_trades > 0 {
            report.average_win = report.gross_profit / Decimal::from(report.winning_trades);
        }

        if report.losing_trades > 0 {
            report.average_loss = report.gross_loss / Decimal::from(report.losing_trades);
            if !report.average_loss.is_zero() {
                report.payoff_ratio = Some(report.average_win / report.average_loss);
            }
        }

        if initial_capital > Decimal::ZERO {
            report.total_return_pct = (report.total_net_profit / initial_capital) * Decimal::from(100);
        }

        Ok(())
    }

    /// Calculates maximum drawdown from the equity curve.
    fn calculate_drawdown(
        &self,
        equity_curve: &[(DateTime<Utc>, Decimal)],
        report: &mut PerformanceReport,
    ) -> Result<(), AnalyticsError> {
        let mut peak_equity = equity_curve[0].1;
        let mut max_drawdown_val = Decimal::ZERO;

        for &(_timestamp, equity) in equity_curve.iter() {
            if equity > peak_equity {
                peak_equity = equity;
            }
            
            let drawdown = peak_equity - equity;
            if drawdown > max_drawdown_val {
                max_drawdown_val = drawdown;
                // --- FIX #3: Calculate drawdown % based on the peak *at that time* ---
                if !peak_equity.is_zero() {
                    report.max_drawdown_pct = (max_drawdown_val / peak_equity) * Decimal::from(100);
                }
                // --- END FIX #3 ---
            }
        }
        
        report.max_drawdown = max_drawdown_val;

        Ok(())
    }
    
    fn calculate_ratios(
        &self,
        equity_curve: &[(DateTime<Utc>, Decimal)],
        interval: &str,
        total_return_pct: Decimal,
        max_drawdown_pct: Decimal,
        new_report: &mut PerformanceReport,
    ) -> Result<(), AnalyticsError> {
        if max_drawdown_pct > Decimal::ZERO {
            new_report.calmar_ratio = Some(total_return_pct / max_drawdown_pct);
        }

        // --- FIX #2: Sharpe Ratio Annualization ---
        let returns: Vec<Decimal> = equity_curve
            .windows(2)
            .map(|w| (w[1].1 - w[0].1) / w[0].1)
            .collect();

        if returns.len() < 2 {
            new_report.sharpe_ratio = None;
            return Ok(());
        }

        let mean_return: Decimal = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());
        
        let std_dev: Decimal = {
            let variance = returns
                .iter()
                .map(|r| (*r - mean_return) * (*r - mean_return))
                .sum::<Decimal>()
                / Decimal::from(returns.len());
            variance.sqrt().ok_or_else(|| AnalyticsError::InternalError("Could not calculate standard deviation.".to_string()))?
        };

        if std_dev > Decimal::ZERO {
            let periods_in_year = self.get_periods_in_year(interval)?;
            let annualization_factor = Decimal::from(periods_in_year).sqrt().ok_or_else(|| AnalyticsError::InternalError("Could not get annualization factor.".to_string()))?;
            
            let sharpe_ratio = (mean_return / std_dev) * annualization_factor;
            new_report.sharpe_ratio = Some(sharpe_ratio);
        }
        // --- END FIX #2 ---

        Ok(())
    }

    fn calculate_time_metrics(
        &self,
        trades: &[Trade],
        report: &mut PerformanceReport,
    ) -> Result<(), AnalyticsError> {
        if trades.is_empty() {
            return Ok(());
        }
        
        let total_duration_secs: i64 = trades.iter().map(|t| {
            (t.exit_execution.timestamp - t.entry_execution.timestamp).num_seconds()
        }).sum();

        if !trades.is_empty() {
            let avg_secs = total_duration_secs / trades.len() as i64;
            report.average_holding_period = Duration::seconds(avg_secs);
        }
        
        Ok(())
    }
    
    // Helper function to determine the annualization factor for the Sharpe Ratio.
    fn get_periods_in_year(&self, interval: &str) -> Result<u32, AnalyticsError> {
        // This is a simplified mapping. A more robust solution might parse the interval string.
        // Assuming 252 trading days in a year for crypto for simplicity.
        match interval {
            "1m" => Ok(252 * 24 * 60),
            "5m" => Ok(252 * 24 * 12),
            "15m" => Ok(252 * 24 * 4),
            "1h" => Ok(252 * 24),
            "4h" => Ok(252 * 6),
            "1d" => Ok(252),
            _ => Err(AnalyticsError::InternalError(format!("Unsupported interval for Sharpe Ratio annualization: {}", interval))),
        }
    }
}