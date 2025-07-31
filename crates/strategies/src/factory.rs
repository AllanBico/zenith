use crate::error::StrategyError;
use crate::funding_rate_arb::FundingRateArb; // <-- ADD THIS LINE
use crate::ma_crossover::MACrossover;
use crate::prob_reversion::ProbReversion;
use crate::super_trend::SuperTrend;
use crate::Strategy;
use configuration::Config;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum StrategyId {
    MACrossover,
    SuperTrend,
    ProbReversion,
    FundingRateArb,
}

/// Creates a new strategy instance based on the provided ID and configuration.
// ... (documentation is unchanged)
pub fn create_strategy(
    id: StrategyId,
    config: &Config,
) -> Result<Box<dyn Strategy>, StrategyError> {
    // With all strategies implemented, we can use a complete match statement.
    // The compiler will now error if a new StrategyId is added but not handled here.
    match id {
        StrategyId::MACrossover => {
            let params = config.strategies.ma_crossover.clone();
            Ok(Box::new(MACrossover::new(params)?))
        }
        StrategyId::SuperTrend => {
            let params = config.strategies.super_trend.clone();
            Ok(Box::new(SuperTrend::new(params)?))
        }
        StrategyId::ProbReversion => {
            let params = config.strategies.prob_reversion.clone();
            Ok(Box::new(ProbReversion::new(params)?))
        }
        StrategyId::FundingRateArb => { // <-- ADD THIS BLOCK
            let params = config.strategies.funding_rate_arb.clone();
            Ok(Box::new(FundingRateArb::new(params)?))
        }
    }
}