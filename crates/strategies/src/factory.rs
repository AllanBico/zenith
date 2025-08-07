use crate::error::StrategyError;
use crate::funding_rate_arb::FundingRateArb;
use crate::ma_crossover::MACrossover;
use crate::ml_strategy::MlStrategy;
use crate::prob_reversion::ProbReversion;
use crate::super_trend::SuperTrend;
use crate::Strategy;
use configuration::Config;
use core_types::enums::StrategyId;

/// Creates a new strategy instance based on the provided ID and configuration.
// ... (documentation is unchanged)
pub fn create_strategy(
    id: StrategyId,
    config: &Config,
    symbol: &str,
) -> Result<Box<dyn Strategy>, StrategyError> {
    // With all strategies implemented, we can use a complete match statement.
    // The compiler will now error if a new StrategyId is added but not handled here.
    match id {
        StrategyId::MACrossover => {
            let params = config.strategies.ma_crossover.clone();
            Ok(Box::new(MACrossover::new(params, symbol.to_string())?))
        }
        StrategyId::SuperTrend => {
            let params = config.strategies.super_trend.clone();
            Ok(Box::new(SuperTrend::new(params, symbol.to_string())?))
        }
        StrategyId::ProbReversion => {
            let params = config.strategies.prob_reversion.clone();
            Ok(Box::new(ProbReversion::new(params, symbol.to_string())?))
        }
        StrategyId::FundingRateArb => { // <-- ADD THIS BLOCK
            let params = config.strategies.funding_rate_arb.clone();
            Ok(Box::new(FundingRateArb::new(params)?))
        }
        StrategyId::MlStrategy => {
            let params = &config.strategies.ml_strategy;
            if params.model_path.as_os_str().is_empty() {
                return Err(StrategyError::InvalidParameters(
                    "MlStrategy requires a `model_path` in config.".to_string()
                ));
            }
            Ok(Box::new(MlStrategy::new(&params.model_path, symbol.to_string())?))
        }
    }
}