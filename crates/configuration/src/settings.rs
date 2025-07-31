use rust_decimal::Decimal;
use serde::Deserialize;

/// The root configuration structure for the entire application.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub risk_management: RiskManagement,
    pub strategies: Strategies,
    // Placeholders for future configurations
    // pub database: DatabaseConfig,
    // pub api: ApiConfig,
}

/// Contains parameters for trade-level risk management.
#[derive(Debug, Clone, Deserialize)]
pub struct RiskManagement {
    /// The fraction of total portfolio equity to risk on a single trade (e.g., 0.01 for 1%).
    pub risk_per_trade_pct: Decimal,
    /// The percentage distance from the entry price to set the stop-loss for position sizing calculations.
    pub stop_loss_pct: Decimal,
}

/// Contains the parameter sets for all available strategies.
#[derive(Debug, Deserialize, Clone)]
pub struct Strategies {
    pub ma_crossover: MACrossoverParams,
    pub super_trend: SuperTrendParams,
    pub prob_reversion: ProbReversionParams,
    pub funding_rate_arb: FundingRateArbParams,
}

/// Parameters for the Triple Moving Average Crossover strategy.
#[derive(Debug, Deserialize, Clone)]
pub struct MACrossoverParams {
    pub ma_fast_period: usize,
    pub ma_slow_period: usize,
    /// A long-term MA to act as a trend filter.
    pub trend_filter_period: usize,
}

/// Parameters for the SuperTrend strategy with an ADX trend filter.
#[derive(Debug, Deserialize, Clone)]
pub struct SuperTrendParams {
    pub atr_period: usize,
    pub atr_multiplier: Decimal,
    /// ADX threshold to confirm trend strength.
    pub adx_threshold: Decimal,
    pub adx_period: usize,
}

/// Parameters for the multi-factor Probabilistic Mean Reversion strategy.
#[derive(Debug, Deserialize, Clone)]
pub struct ProbReversionParams {
    pub bb_period: usize,
    pub bb_std_dev: Decimal,
    pub rsi_period: usize,
    pub rsi_oversold: Decimal,
    pub rsi_overbought: Decimal,
    /// ADX threshold to confirm a ranging market (i.e., weak or no trend).
    pub adx_threshold: Decimal,
    pub adx_period: usize,
}

/// Parameters for the Funding Rate Arbitrage strategy.
#[derive(Debug, Deserialize, Clone)]
pub struct FundingRateArbParams {
    /// The target funding rate threshold to trigger a position.
    pub target_rate_threshold: Decimal,
    /// A safety threshold. If spot-perp basis expands beyond this, close the position.
    pub basis_safety_threshold: Decimal,
}