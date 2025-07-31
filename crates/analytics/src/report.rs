use chrono::Duration;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A comprehensive, standardized report of a strategy's performance.
///
/// This struct is the final output of the `AnalyticsEngine` and serves as the
/// data transfer object for performance results throughout the entire system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceReport {
    // I. Core Profitability Metrics
    pub total_net_profit: Decimal,
    pub gross_profit: Decimal,
    pub gross_loss: Decimal,
    pub profit_factor: Option<Decimal>, // Option<> because it can be infinite if GrossLoss is 0
    pub total_return_pct: Decimal,

    // II. Risk and Drawdown
    pub max_drawdown: Decimal,
    pub max_drawdown_pct: Decimal,
    pub sharpe_ratio: Option<Decimal>, // Option<> for cases with no stdev
    pub calmar_ratio: Option<Decimal>, // Option<> for cases with no drawdown

    // III. Trade-Level Statistics
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate_pct: Option<Decimal>, // Option<> for cases with 0 trades
    pub average_win: Decimal,
    pub average_loss: Decimal,
    pub payoff_ratio: Option<Decimal>, // Option<> because avg_loss can be 0

    // IV. Time-Based Metrics
    #[serde(with = "humantime_serde")]
    pub average_holding_period: Duration,
}

impl PerformanceReport {
    /// Creates a new, zeroed-out PerformanceReport.
    /// This is useful as a default or starting point before calculations.
    pub fn new() -> Self {
        Self {
            total_net_profit: Decimal::ZERO,
            gross_profit: Decimal::ZERO,
            gross_loss: Decimal::ZERO,
            profit_factor: None,
            total_return_pct: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            max_drawdown_pct: Decimal::ZERO,
            sharpe_ratio: None,
            calmar_ratio: None,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate_pct: None,
            average_win: Decimal::ZERO,
            average_loss: Decimal::ZERO,
            payoff_ratio: None,
            average_holding_period: Duration::zero(),
        }
    }
}

impl Default for PerformanceReport {
    fn default() -> Self {
        Self::new()
    }
}