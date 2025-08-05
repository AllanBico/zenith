use rust_decimal::Decimal;
use serde::Deserialize;
use chrono::NaiveDate;
use serde_json::Value as JsonValue;
use core_types::enums::StrategyId;
use clap::ValueEnum;
/// The root configuration structure for the entire application.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub api: ApiConfig,
    pub simulation: Simulation,
    pub risk_management: RiskManagement,
    pub strategies: Strategies,
    /// Configuration for backtesting parameters
    pub backtest: Backtest,
}
/// Holds the API connection details and secrets for different environments.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub binance_api_key: String,
    pub binance_api_secret: String,
    #[serde(default)] // Use default (empty) if not provided by env
    pub testnet: ApiKeys,
    #[serde(default)] // Use default (empty) if not provided by env
    pub production: ApiKeys,
}

/// A structure to hold an API key and secret pair.
#[derive(Debug, Clone, Deserialize, Default)] // Default provides empty strings
pub struct ApiKeys {
    pub key: String,
    pub secret: String,
}
/// Contains parameters for a single backtest run.
#[derive(Debug, Clone, Deserialize)]
pub struct Backtest {
    /// The strategy to use for the backtest.
    pub strategy_id: StrategyId,
    /// The symbol to use for the backtest (e.g., "BTCUSDT").
    pub symbol: String,
    /// The timeframe interval to use (e.g., "1h").
    pub interval: String,
    /// The initial starting capital for the simulation.
    pub initial_capital: Decimal,
    /// The default start date for the backtest period.
    pub start_date: NaiveDate,
    /// The default end date for the backtest period.
    pub end_date: NaiveDate,
}

/// Defines the configuration for the live trading engine.
#[derive(Debug, Clone, Deserialize)]
pub struct LiveConfig {
    /// A master safety switch to enable or disable all live trading.
    pub live_trading_enabled: bool,
    /// The timeframe interval to use for all bots (e.g., "1m", "5m", "1h").
    pub interval: String,
    /// A collection of individual trading bots to run.
    #[serde(rename = "bot")]
    pub bots: Vec<LiveBotConfig>,
}
// --- Execution Mode ---
// Defines the possible execution environments for the `run` command.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExecutionMode {
    /// Live data, simulated execution (local `SimulatedExecutor`).
    Paper,
    /// Live data, real orders sent to the Binance Testnet exchange.
    Testnet,
    /// Live data, real orders sent to the Binance Production exchange (REAL MONEY).
    Live,
}
/// Defines a single trading bot for the live engine.
#[derive(Debug, Clone, Deserialize)]
pub struct LiveBotConfig {
    /// A master switch to enable or disable this specific bot.
    pub enabled: bool,
    pub symbol: String,
    pub strategy_id: StrategyId,
    /// The specific parameters for this bot's strategy, stored as a flexible object.
    pub params: JsonValue,
}

/// Contains parameters for the backtesting and simulation engine.
#[derive(Debug, Clone, Deserialize)]
pub struct Simulation {
    /// The trading fees charged by the exchange for a "taker" order.
    /// 0.0004 corresponds to 0.04%.
    pub taker_fee_pct: Decimal,
    
    /// The assumed price slippage for market orders.
    /// This is a simple model where slippage is a percentage of the bar's high-low range.
    /// 0.1 means we assume we get a price that is 10% worse than the close.
    pub slippage_pct: Decimal,
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
/// Defines a portfolio, which is a collection of individual trading bots.
#[derive(Debug, Clone, Deserialize)]
pub struct PortfolioConfig {
    #[serde(rename = "bot")]
    pub bots: Vec<PortfolioBotConfig>,
}

/// Defines a single trading bot with its parameters embedded directly.
#[derive(Debug, Clone, Deserialize)]
pub struct PortfolioBotConfig {
    pub symbol: String,
    pub strategy_id: StrategyId,
    /// The specific parameters for this bot, stored as a flexible JSON/TOML object.
    pub params: JsonValue,
}