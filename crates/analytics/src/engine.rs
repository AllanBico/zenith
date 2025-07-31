use crate::error::AnalyticsError;
use crate::report::PerformanceReport;
use chrono::{DateTime, Duration, Utc};
use core_types::Trade;
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
    ///
    /// # Arguments
    ///
    /// * `trades` - A slice of all completed `Trade`s from a trading session.
    /// * `equity_curve` - A time-series of the portfolio's value.
    /// * `initial_capital` - The starting capital of the trading session.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `PerformanceReport` or an `AnalyticsError`.
    pub fn calculate(
        &self,
        trades: &[Trade],
        equity_curve: &[(DateTime<Utc>, Decimal)],
        initial_capital: Decimal,
    ) -> Result<PerformanceReport, AnalyticsError> {
        let mut report = PerformanceReport::new();

        if trades.is_empty() {
            // If there are no trades, many metrics are zero or undefined.
            // Return a default report, which is mostly zeroed out.
            return Ok(report);
        }

        self.calculate_profitability(trades, initial_capital, &mut report)?;
        self.calculate_drawdown(equity_curve, &mut report)?;
        self.calculate_time_metrics(trades, &mut report)?;
        
        // Create a local copy of the report to avoid borrowing issues
        let report_copy = report.clone();
        self.calculate_ratios(equity_curve, &report_copy, &mut report)?;

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
            // Assumes exit_execution.quantity is the same as entry_execution.quantity
            let pnl = (trade.exit_execution.price - trade.entry_execution.price)
                * trade.exit_execution.quantity;

            report.total_net_profit += pnl;

            if pnl.is_sign_positive() {
                report.gross_profit += pnl;
                report.winning_trades += 1;
            } else {
                report.gross_loss += pnl.abs();
                report.losing_trades += 1;
            }
        }

        // --- Ratios ---
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
            report.payoff_ratio = Some(report.average_win / report.average_loss);
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
        let mut max_drawdown = Decimal::ZERO;

        if equity_curve.is_empty() {
            return Ok(());
        }
        
        let mut peak_equity = equity_curve[0].1;

        for &(_timestamp, equity) in equity_curve {
            if equity > peak_equity {
                peak_equity = equity;
            }
            let drawdown = peak_equity - equity;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }
        
        report.max_drawdown = max_drawdown;

        if peak_equity > Decimal::ZERO {
            report.max_drawdown_pct = (max_drawdown / peak_equity) * Decimal::from(100);
        }

        Ok(())
    }
    
    /// Calculates all ratio-based metrics like Sharpe and Calmar.
    fn calculate_ratios(
        &self,
        equity_curve: &[(DateTime<Utc>, Decimal)],
        report: &PerformanceReport,
        new_report: &mut PerformanceReport,
    ) -> Result<(), AnalyticsError> {
        // --- Calmar Ratio ---
        if report.max_drawdown_pct > Decimal::ZERO {
            new_report.calmar_ratio = Some(report.total_return_pct / report.max_drawdown_pct);
        }

        // --- Sharpe Ratio ---
        // 1. Calculate periodic returns (e.g., daily returns)
        let returns: Vec<Decimal> = equity_curve
            .windows(2)
            .map(|w| (w[1].1 - w[0].1) / w[0].1)
            .collect();

        if returns.len() < 2 {
            new_report.sharpe_ratio = None;
            return Ok(());
        }

        // 2. Calculate the mean of returns
        let returns_sum: Decimal = returns.iter().sum();
        let mean_return = returns_sum / Decimal::from(returns.len());

        // 3. Calculate the standard deviation of returns
        let variance: Decimal = returns
            .iter()
            .map(|r| (*r - mean_return) * (*r - mean_return))
            .sum::<Decimal>()
            / Decimal::from(returns.len());
        
        if variance <= Decimal::ZERO {
            new_report.sharpe_ratio = None;
            return Ok(());
        }

        let std_dev = variance.sqrt()
            .ok_or_else(|| AnalyticsError::InternalError("Failed to calculate square root for variance".to_string()))?;
        
        // 4. Calculate Sharpe (assuming risk-free rate is 0)
        if std_dev > Decimal::ZERO {
            // Annualize by multiplying by sqrt(num_periods_in_year)
            // This is a simplification. A real implementation would need to know the period (daily, hourly etc).
            // For now, we calculate the non-annualized Sharpe.
            new_report.sharpe_ratio = Some(mean_return / std_dev);
        }

        Ok(())
    }

    /// Calculates time-based metrics.
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

        let avg_secs = total_duration_secs / trades.len() as i64;
        report.average_holding_period = Duration::seconds(avg_secs);
        
        Ok(())
    }
}