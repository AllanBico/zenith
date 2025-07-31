use crate::error::ConfigError;
use crate::settings::Config;

// Declare the modules that make up this crate.
pub mod error;
pub mod settings;

// Re-export the core types to provide a clean public API.
pub use settings::{
    Config, FundingRateArbParams, MACrossoverParams, ProbReversionParams, RiskManagement,
    Strategies, SuperTrendParams,
};

/// Loads the application configuration from the `config.toml` file.
///
/// This function is the primary entry point for this crate. It reads the configuration file,
/// deserializes it into our strongly-typed `Config` struct, and returns it.
pub fn load_config() -> Result<Config, ConfigError> {
    let builder = config::Config::builder()
        // Tells the builder to look for a file named `config.toml`
        .add_source(config::File::with_name("config.toml"))
        // Optionally, one could add environment variables here as well.
        // .add_source(config::Environment::with_prefix("APP"));
        .build()?;

    // Attempt to deserialize the entire configuration into our `Config` struct
    let config = builder.try_deserialize::<Config>()?;

    Ok(config)
}