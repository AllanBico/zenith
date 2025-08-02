use rust_decimal::Decimal;
use serde::Deserialize;
use core_types::enums::StrategyId;
use std::collections::HashMap;
use std::str::FromStr;

/// Defines an optimization job. This is deserialized from the `optimizer.toml` file.
#[derive(Debug, Clone, Deserialize)]
pub struct OptimizerConfig {
    pub base_config: BaseConfig,
    pub parameter_space: HashMap<String, ParameterRange>,
    #[serde(default)] // Use default values if the [analysis] section is missing
    pub analysis: AnalysisConfig,
}

/// Base settings for the optimization job.
#[derive(Debug, Clone, Deserialize)]
pub struct BaseConfig {
    pub strategy_id: StrategyId,
    pub symbol: String,
    pub interval: String,
}

/// Configuration for the analysis and ranking of optimization results.
#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisConfig {
    /// Hard filters to discard unacceptable runs before scoring.
    pub filters: Filters,
    /// Weights for the multi-objective scoring function. Must sum to 1.0.
    pub scoring_weights: Weights,
}

/// Hard filters to apply to the set of performance reports.
#[derive(Debug, Clone, Deserialize)]
pub struct Filters {
    pub min_total_trades: usize,
    pub max_drawdown_pct: Decimal,
}

/// Weights for the scoring function.
#[derive(Debug, Clone, Deserialize)]
pub struct Weights {
    pub weight_profit_factor: Decimal,
    pub weight_calmar_ratio: Decimal,
    pub weight_avg_win_loss_ratio: Decimal,
}

// --- Default Implementations ---
// This allows a user to omit the `[analysis]` section from their toml
// and still have it work with sensible defaults.

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            filters: Filters::default(),
            scoring_weights: Weights::default(),
        }
    }
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            min_total_trades: 20,
            max_drawdown_pct: Decimal::from(25), // Default max drawdown of 25%
        }
    }
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            weight_profit_factor: Decimal::from_str("0.4").unwrap(),
            weight_calmar_ratio: Decimal::from_str("0.4").unwrap(),
            weight_avg_win_loss_ratio: Decimal::from_str("0.2").unwrap(),
        }
    }
}

/// Represents a range of values for a single parameter to be tested.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ParameterRange {
    DiscreteInt(Vec<i64>),
    DiscreteDecimal(Vec<Decimal>),
    LinearInt { start: i64, end: i64, step: i64 },
    LinearDecimal { start: Decimal, end: Decimal, step: Decimal },
}