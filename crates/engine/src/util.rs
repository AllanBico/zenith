use crate::error::EngineError;
use configuration::{Config, LiveBotConfig, MACrossoverParams, ProbReversionParams, SuperTrendParams};
use serde_json::from_value;
use strategies::{create_strategy, Strategy, StrategyId};

/// Creates a `Strategy` instance by merging the bot-specific parameters from the
/// live config into a temporary copy of the base configuration.
pub fn create_strategy_from_live_config(
    base_config: &Config,
    bot_config: &LiveBotConfig,
) -> Result<Box<dyn Strategy>, EngineError> {
    let mut temp_config = base_config.clone();

    // Deserialize the JSON `Value` from the bot's config into the appropriate
    // concrete parameter struct, then overwrite the corresponding part of our temp config.
    match bot_config.strategy_id {
        StrategyId::MACrossover => {
            let params: MACrossoverParams = from_value(bot_config.params.clone())
                .map_err(|e| EngineError::Configuration(e.to_string()))?;
            temp_config.strategies.ma_crossover = params;
        }
        StrategyId::SuperTrend => {
            let params: SuperTrendParams = from_value(bot_config.params.clone())
                .map_err(|e| EngineError::Configuration(e.to_string()))?;
            temp_config.strategies.super_trend = params;
        }
        StrategyId::ProbReversion => {
            let params: ProbReversionParams = from_value(bot_config.params.clone())
                .map_err(|e| EngineError::Configuration(e.to_string()))?;
            temp_config.strategies.prob_reversion = params;
        }
        _ => {
            return Err(EngineError::Configuration(
                "Strategy not supported in live engine".to_string(),
            ))
        }
    }

    Ok(create_strategy(bot_config.strategy_id, &temp_config, bot_config.symbol.as_str())?)
}