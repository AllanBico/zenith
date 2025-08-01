use crate::error::OptimizerError;
use crate::generator::generate_parameter_sets;
use backtester::Backtester;
use configuration::optimizer_config::OptimizerConfig;
use configuration::Config;
use database::repository::DbBacktestRun;
use database::DbRepository;
use executor::{Portfolio, SimulatedExecutor};
use indicatif::{ProgressBar, ProgressStyle};
use risk::SimpleRiskManager;
use serde_json::Value as JsonValue;
use strategies::{create_strategy, StrategyId};
use tokio::runtime::Handle;
use uuid::Uuid;
use chrono::Utc;

pub mod error;
pub mod generator;

pub struct Optimizer {
    job_id: Uuid,
    config: OptimizerConfig,
    base_config: Config,
    db_repo: DbRepository,
}

impl Optimizer {
    pub fn new(
        config: OptimizerConfig,
        base_config: Config,
        db_repo: DbRepository,
    ) -> Self {
        Self {
            job_id: Uuid::new_v4(),
            config,
            base_config,
            db_repo,
        }
    }

    pub async fn run(&self) -> Result<(), OptimizerError> {
        self.initialize_job().await?;

        let pending_runs = self.db_repo.get_pending_runs(self.job_id).await?;
        let total_runs = pending_runs.len();
        if total_runs == 0 {
            println!("No pending runs found for job {}. It may have been completed previously.", self.job_id);
            return Ok(());
        }
        
        println!(
            "Starting optimization job {} with {} pending runs on {} CPU cores.",
            self.job_id,
            total_runs,
            rayon::current_num_threads()
        );

        let progress_bar = ProgressBar::new(total_runs as u64);
        progress_bar.set_style(
             ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .map_err(|e| OptimizerError::ProgressBarTemplate(e.to_string()))?
                .progress_chars("=>-")
        );

        // --- THE FIX ---
        // 1. Get a handle to the main Tokio runtime *before* entering the rayon block.
        let tokio_handle = Handle::current();

        // 2. Use `scope` for safer thread management and error handling.
        rayon::scope(|s| {
            for run in pending_runs {
                // 3. Clone the handle and move it into the spawned task.
                let handle_clone = tokio_handle.clone();
                let progress_bar_clone = progress_bar.clone();

                s.spawn(move |_| {
                    // 4. Use the cloned handle to block on the async task.
                    let result = handle_clone.block_on(self.execute_single_backtest(run));

                    if let Err(e) = result {
                        eprintln!("A backtest run failed: {:?}", e);
                    }
                    progress_bar_clone.inc(1);
                });
            }
        });
        
        progress_bar.finish_with_message("Optimization runs complete.");

        println!("Job {} complete. Next step: Analysis.", self.job_id);
        
        Ok(())
    }

    async fn initialize_job(&self) -> Result<(), OptimizerError> {
        self.db_repo.save_optimization_job(
            self.job_id,
            &format!("{:?}", self.config.base_config.strategy_id),
            &self.config.base_config.symbol,
            "Initializing"
        ).await?;

        let param_sets = generate_parameter_sets(&self.config)?;

        for params in param_sets {
            // Convert serde_json::Value to JsonValue (which is an alias for serde_json::Value)
            let json_params: JsonValue = params;
            self.db_repo.save_backtest_run(
                Uuid::new_v4(),
                self.job_id,
                &json_params,
                "Pending"
            ).await?;
        }
        Ok(())
    }
    
    async fn execute_single_backtest(&self, run: DbBacktestRun) -> Result<(), OptimizerError> {
        let run_id = run.run_id;
        
        // Use initial capital from configuration
        let initial_capital = self.base_config.backtest.initial_capital;
        
        let analytics_engine = analytics::AnalyticsEngine::new();
        let portfolio = Portfolio::new(initial_capital);
        let executor = Box::new(SimulatedExecutor::new(self.base_config.simulation.clone()));
        let risk_manager = Box::new(SimpleRiskManager::new(self.base_config.risk_management.clone())?);

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

        // Use backtest dates from configuration
        let start_date = self.base_config.backtest.start_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(chrono::Utc)
            .unwrap();
            
        let end_date = self.base_config.backtest.end_date
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_local_timezone(chrono::Utc)
            .unwrap();

        let report = backtester.run(start_date, end_date).await;

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

    fn create_strategy_instance(&self, optimized_params: &JsonValue) -> Result<Box<dyn strategies::Strategy>, OptimizerError> {
        let strategy_id = self.config.base_config.strategy_id;
        
        let mut temp_config = self.base_config.clone();
        
        // This dynamic deserialization is tricky but powerful.
        let params_map: serde_json::Map<String, JsonValue> = serde_json::from_value(optimized_params.clone())?;

        match strategy_id {
            StrategyId::MACrossover => {
                let mut p = temp_config.strategies.ma_crossover.clone();
                if let Some(val) = params_map.get("ma_fast_period") { p.ma_fast_period = val.as_u64().unwrap() as usize; }
                if let Some(val) = params_map.get("ma_slow_period") { p.ma_slow_period = val.as_u64().unwrap() as usize; }
                temp_config.strategies.ma_crossover = p;
            },
            // Add similar blocks for other strategies as you optimize them
            _ => return Err(OptimizerError::Strategy(strategies::StrategyError::StrategyNotFound("Cannot optimize this strategy yet".to_string()))),
        }
        
        Ok(create_strategy(strategy_id, &temp_config, &self.config.base_config.symbol)?)
    }
}