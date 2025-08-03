use std::path::Path;
pub mod optimizer_config;
pub use optimizer_config::{OptimizerConfig, ParameterRange, BaseConfig};

use rust_decimal_macros::dec;
use crate::error::ConfigError;

// Declare the modules that make up this crate.
pub mod error;
pub mod settings;

// Re-export the core types to provide a clean public API.
pub use settings::{
    LiveBotConfig, LiveConfig,Config, FundingRateArbParams, MACrossoverParams, ProbReversionParams, RiskManagement,PortfolioBotConfig, PortfolioConfig,
    Simulation, Strategies, SuperTrendParams,
};

/// Loads the application configuration from the specified path.
///
/// # Arguments
/// * `config_path` - Optional path to the configuration file. If None, it will look for 'config.toml' in the current directory.
///
/// # Returns
/// A `Result` containing the deserialized `Config` if successful, or a `ConfigError` if loading or parsing fails.
///
/// # Examples
/// ```no_run
/// use configuration::load_config;
///
/// let config = load_config(Some("path/to/config.toml"));
/// match config {
///     Ok(cfg) => println!("Configuration loaded successfully: {:?}", cfg),
///     Err(e) => eprintln!("Failed to load configuration: {}", e),
/// }
/// ```
pub fn load_config(config_path: Option<&str>) -> Result<Config, ConfigError> {
    let config_path = config_path.unwrap_or("config.toml");
    
    // Check if the config file exists and is readable
    if !Path::new(config_path).exists() {
        return Err(ConfigError::FileNotFound(config_path.to_string()));
    }

    let builder = config::Config::builder()
        // Load configuration from the specified file
        .add_source(config::File::with_name(config_path).required(true))
        // Add environment variables with APP_ prefix (e.g., APP_SIMULATION__TAKER_FEE_PCT)
        .add_source(
            config::Environment::with_prefix("ZENITH")
                .prefix_separator("_")
                .separator("__"),
        )
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("__")
                .separator("__")
                .try_parsing(true),
        )
        .build()?;

    // Deserialize the configuration into our strongly-typed struct
    let config: Config = builder.try_deserialize()?;

    // Validate the configuration values
    validate_config(&config)?;

    Ok(config)
}

/// Validates the configuration values after loading.
fn validate_config(config: &Config) -> Result<(), ConfigError> {
    // Validate simulation parameters
    if config.simulation.taker_fee_pct.is_sign_negative() || config.simulation.taker_fee_pct > dec!(1.0) {
        return Err(ConfigError::ValidationError("taker_fee_pct must be between 0 and 1".into()));
    }

    if config.simulation.slippage_pct.is_sign_negative() || config.simulation.slippage_pct > dec!(1.0) {
        return Err(ConfigError::ValidationError("slippage_pct must be between 0 and 1".into()));
    }

    // Validate risk management parameters
    if config.risk_management.risk_per_trade_pct <= dec!(0.0) || config.risk_management.risk_per_trade_pct > dec!(0.1) {
        return Err(ConfigError::ValidationError("risk_per_trade_pct must be between 0 and 0.1 (10%)".into()));
    }

    if config.risk_management.stop_loss_pct <= dec!(0.0) || config.risk_management.stop_loss_pct > dec!(0.2) {
        return Err(ConfigError::ValidationError("stop_loss_pct must be between 0 and 0.2 (20%)".into()));
    }

    // Add more validation as needed

    Ok(())
}
/// Loads the optimizer configuration from a specific TOML file path.
pub fn load_optimizer_config(path: &Path) -> Result<OptimizerConfig, ConfigError> {
    let builder = config::Config::builder()
        .add_source(config::File::from(path))
        .build()?;
    builder.try_deserialize::<OptimizerConfig>().map_err(Into::into)
}

/// Loads the portfolio configuration from a specific TOML file path.
pub fn load_portfolio_config(path: &Path) -> Result<PortfolioConfig, ConfigError> {
    let builder = config::Config::builder()
        .add_source(config::File::from(path))
        .build()?;
    builder.try_deserialize::<PortfolioConfig>().map_err(Into::into)
}

/// Loads the live trading configuration from a specific TOML file path.
pub fn load_live_config(path: &Path) -> Result<LiveConfig, ConfigError> {
    let builder = config::Config::builder()
        .add_source(config::File::from(path))
        .build()?;
    builder.try_deserialize::<LiveConfig>().map_err(Into::into)
}