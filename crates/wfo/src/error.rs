use thiserror::Error;

#[derive(Error, Debug)]
pub enum WfoError {
    #[error("Optimizer error during in-sample training: {0}")]
    Optimizer(#[from] optimizer::OptimizerError),

    #[error("Analyzer error during in-sample analysis: {0}")]
    Analyzer(#[from] analyzer::error::AnalyzerError),

    #[error("Backtester error during out-of-sample validation: {0}")]
    Backtester(#[from] backtester::error::BacktestError),
    
    #[error("Database error: {0}")]
    Database(#[from] database::DbError),
    
    #[error("Risk management error: {0}")]
    Risk(#[from] risk::RiskError),
    
    #[error("Configuration error: WFO settings are missing from the config file.")]
    ConfigMissing,

    #[error("No best parameter set found during in-sample optimization for period {start} to {end}.")]
    NoBestParamsFound { start: String, end: String },

    #[error("Date range or period error: {0}")]
    DateError(String),
}