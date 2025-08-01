use core_types::enums::StrategyId;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;

/// Defines an optimization job. This is deserialized from the `optimizer.toml` file.
#[derive(Debug, Clone, Deserialize)]
pub struct OptimizerConfig {
    pub base_config: BaseConfig,
    pub parameter_space: HashMap<String, ParameterRange>,
}

/// Base settings for the optimization job.
#[derive(Debug, Clone, Deserialize)]
pub struct BaseConfig {
    pub strategy_id: StrategyId,
    pub symbol: String,
    pub interval: String,
}

/// Represents a range of values for a single parameter to be tested.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)] // Allows serde to try deserializing into one of the variants
pub enum ParameterRange {
    // e.g., `ma_fast = [10, 12, 15]`
    DiscreteInt(Vec<i64>),
    // e.g., `atr_multiplier = [2.0, 2.5, 3.0]`
    DiscreteDecimal(Vec<Decimal>),
    // e.g., `ma_slow = { start = 20, end = 50, step = 5 }`
    LinearInt { start: i64, end: i64, step: i64 },
    // e.g., `bb_std_dev = { start = 1.8, end = 2.2, step = 0.1 }`
    LinearDecimal { start: Decimal, end: Decimal, step: Decimal },
}