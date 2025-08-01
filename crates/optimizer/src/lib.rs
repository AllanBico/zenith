use crate::error::OptimizerError;
use crate::generator::generate_parameter_sets;
use backtester::Backtester;
use chrono::Utc;
use configuration::optimizer_config::OptimizerConfig;
use configuration::{Config, MACrossoverParams, ProbReversionParams, SuperTrendParams};
use database::repository::DbBacktestRun;
use database::DbRepository;
use executor::Portfolio;
use executor::SimulatedExecutor;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use risk::SimpleRiskManager;
use serde_json::Value as JsonValue;
use strategies::{create_strategy, StrategyId};
use tokio::runtime::Handle;
use uuid::Uuid;

pub mod error;
pub mod generator;

pub struct Optimizer {
    job_id: Uuid,
    config: OptimizerConfig,
    base_config: Config, // The main config.toml for non-optimized params
    db_repo: DbRepository,
}

impl Optimizer {
    pub fn new(
        config: OptimizerConfig,
        base_config: Config,
        db_repo: DbRepository,
    ) -> Self {
        Self {
            job_id: Uuid::new_v4(), // A unique ID for this entire optimization job
            config,
            base_config,
            db_repo,
        }
    }

    pub async fn run(&self) -> Result<(), OptimizerError> {
        // 1. Create Job & Generate Tasks
        self.initialize_job().await?;

        // 2. Fetch Pending Tasks
        let pending_runs = self.db_repo.get_pending_runs(self.job_id).await?;
        let total_runs = pending_runs.len();
        println!(
            "Starting optimization job {} with {} pending runs on {} CPU cores.",
            self.job_id,
            total_runs,
            rayon::current_num_threads()
        );

        let progress_bar = ProgressBar::new(total_runs as u64);
        progress_bar.set_style(
             ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("=>-"),
        );

        // 3. Parallel Execution
        pending_runs.into_par_iter().for_each(|run| {
            // Must block on the async code inside the sync rayon closure.
            let handle = Handle::current();
            let result = handle.block_on(self.execute_single_backtest(run));

            if let Err(e) = result {
                eprintln!("A backtest run failed: {:?}", e);
            }
            progress_bar.inc(1);
        });
        
        progress_bar.finish_with_message("Optimization runs complete.");

        // 4. Finalize Job
        // Here we would kick off Phase 11: The Analyst
        println!("Job {} complete. Next step: Analysis.", self.job_id);
        
        Ok(())
    }

    /// Prepares the database by creating records for the job and all pending backtest runs.
    async fn initialize_job(&self) -> Result<(), OptimizerError> {
        // Save the optimization job with all required parameters
        self.db_repo.save_optimization_job(
            self.job_id,
            &format!("{:?}", self.config.base_config.strategy_id),
            &self.config.base_config.symbol,
            "Initializing",
        ).await?;

        let param_sets = generate_parameter_sets(&self.config)?;

        for params in param_sets {
            // Convert parameters to JSON value
            let params_json = serde_json::to_value(&params)
                .map_err(|e| OptimizerError::ParameterGeneration(e.to_string()))?;
            
            self.db_repo.save_backtest_run(
                Uuid::new_v4(), // run_id
                self.job_id,
                &params_json,
                "Pending",
            ).await?;
        }
        Ok(())
    }
    
    /// Executes a single, complete backtest for one set of parameters.
    async fn execute_single_backtest(&self, run: DbBacktestRun) -> Result<(), OptimizerError> {
        let run_id = run.run_id;
        
        // A. Instantiate all components for the backtest
        let analytics_engine = analytics::AnalyticsEngine::new();
        // Using default initial capital since it's not in BaseConfig
        let portfolio = Portfolio::new(rust_decimal_macros::dec!(10000.0));
        let executor = Box::new(SimulatedExecutor::new(self.base_config.simulation.clone()));
        let risk_manager = Box::new(SimpleRiskManager::new(self.base_config.risk_management.clone())?);

        // B. Create the specific strategy instance with the optimized parameters
        let strategy = self.create_strategy_instance(&run.parameters)?;

        let mut backtester = Backtester::new(
            self.config.base_config.symbol.clone(),
            self.config.base_config.interval.clone(),
            portfolio,
            strategy,
            risk_manager,
            executor,
            analytics_engine,
            self.db_repo.clone(),
        );

        // C. Run the backtest
        // Using default dates since they're not in BaseConfig
        let start_date = chrono::Utc::now() - chrono::Duration::days(30);
        let end_date = chrono::Utc::now();
        
        let report = backtester.run(
            start_date,
            end_date,
        ).await;

        // D. Handle the result
        match report {
            Ok(rep) => {
                self.db_repo.save_performance_report(run_id, &rep).await?;
                self.db_repo.update_run_status(run_id, "Completed").await?;
            }
            Err(e) => {
                eprintln!("Backtest run {} failed: {:?}", run_id, e);
                self.db_repo.update_run_status(run_id, "Failed").await?;
            }
        }
        
        Ok(())
    }

    /// Creates a strategy instance by merging optimized params with base params.
    fn create_strategy_instance(&self, optimized_params: &JsonValue) -> Result<Box<dyn strategies::Strategy>, OptimizerError> {
        let strategy_id = self.config.base_config.strategy_id;
        
        // This is complex. We need to deserialize the JSON into the correct params struct.
        // A simpler approach for now is to use the existing factory, but it needs the main `Config` object.
        // We'll create a temporary, modified `Config` object for this run.
        let mut temp_config = self.base_config.clone();
        
        match strategy_id {
            StrategyId::MACrossover => {
                let params: MACrossoverParams = serde_json::from_value(optimized_params.clone())?;
                temp_config.strategies.ma_crossover = params;
            },
            StrategyId::SuperTrend => {
                let params: SuperTrendParams = serde_json::from_value(optimized_params.clone())?;
                temp_config.strategies.super_trend = params;
            },
            StrategyId::ProbReversion => {
                let params: ProbReversionParams = serde_json::from_value(optimized_params.clone())?;
                temp_config.strategies.prob_reversion = params;
            }
            _ => return Err(OptimizerError::Strategy(strategies::StrategyError::StrategyNotFound("Cannot optimize this strategy yet".to_string()))),
        }
        
        // Create strategy with the merged configuration
        create_strategy(
            strategy_id,
            &temp_config,
            &self.config.base_config.symbol
        ).map_err(OptimizerError::from)
    }
}

// Add this to configuration `lib.rs`
// use serde_json::Error as SerdeJsonError;
// In `ConfigError` enum:
// #[error("JSON deserialization error: {0}")]
// JsonError(#[from] SerdeJsonError),
// And add thiserror + serde_json to optimizer Cargo.toml
// And add this to optimizer error.rs
// #[error("JSON deserialization error: {0}")]
// JsonError(#[from] serde_json::Error),