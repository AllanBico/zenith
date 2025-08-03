use rust_decimal::Decimal;
use serde::Deserialize;
use core_types::enums::StrategyId;
use std::collections::HashMap;

/// Defines an optimization job. This is deserialized from the `optimizer.toml` file.
#[derive(Debug, Clone, Deserialize)]
pub struct OptimizerConfig {
    pub base_config: BaseConfig,
    pub parameter_space: HashMap<String, ParameterRange>,
    #[serde(default)]
    pub analysis: AnalysisConfig,
    #[serde(default)]
    pub wfo: Option<WfoConfig>,
}

/// Base settings for the optimization job.
#[derive(Debug, Clone, Deserialize)]
pub struct BaseConfig {
    pub strategy_id: StrategyId,
    pub symbol: String,
    pub interval: String,
}

/// Contains parameters for a Walk-Forward Optimization job.
#[derive(Debug, Clone, Deserialize)]
pub struct WfoConfig {
    /// The length of the In-Sample (training) period in weeks.
    pub in_sample_weeks: i64,
    /// The length of the Out-of-Sample (testing) period in weeks.
    pub out_of_sample_weeks: i64,
}

/// Configuration for the analysis and ranking of optimization results.
#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisConfig {
    pub filters: Filters,
    pub scoring_weights: Weights,
}

// ... (Filters, Weights, Default implementations, and ParameterRange are unchanged) ...

#[derive(Debug, Clone, Deserialize)]
pub struct Filters {
    pub min_total_trades: usize,
    pub max_drawdown_pct: Decimal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Weights {
    pub weight_profit_factor: Decimal,
    pub weight_calmar_ratio: Decimal,
    pub weight_avg_win_loss_ratio: Decimal,
}

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
            max_drawdown_pct: Decimal::from(25),
        }
    }
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            weight_profit_factor: "0.4".parse().unwrap(),
            weight_calmar_ratio: "0.4".parse().unwrap(),
            weight_avg_win_loss_ratio: "0.2".parse().unwrap(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ParameterRange {
    DiscreteInt(Vec<i64>),
    DiscreteDecimal(Vec<Decimal>),
    LinearInt { start: i64, end: i64, step: i64 },
    LinearDecimal { start: Decimal, end: Decimal, step: Decimal },
}