use crate::generator::generate_parameter_sets;
use backtester::Backtester;
use configuration::optimizer_config::OptimizerConfig;
use configuration::Config;
use database::{DbBacktestRun, DbRepository};
use executor::{Portfolio, SimulatedExecutor};
use indicatif::{ProgressBar, ProgressStyle};
use risk::SimpleRiskManager;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde_json::Value as JsonValue;
use strategies::{create_strategy, StrategyId};
use tokio::runtime::Handle;
use tracing;
use uuid::Uuid;
use chrono::Utc;

pub mod error;
pub mod generator;

pub use error::OptimizerError;

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

    /// Returns the job ID for this optimizer instance.
    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub async fn run(&self) -> Result<(), OptimizerError> {
        self.initialize_job().await?;

        let pending_runs = self.db_repo.get_pending_runs(self.job_id).await?;
        let total_runs = pending_runs.len();
        if total_runs == 0 {
            tracing::info!("No pending runs found for job {}. It may have been completed previously.", self.job_id);
            return Ok(());
        }
        
        tracing::info!(
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

        let tokio_handle = Handle::current();

        rayon::scope(|s| {
            for run in pending_runs {
                let handle_clone = tokio_handle.clone();
                let progress_bar_clone = progress_bar.clone();

                s.spawn(move |_| {
                    let result = handle_clone.block_on(self.execute_single_backtest(run));

                    if let Err(e) = result {
                        tracing::error!(error = ?e, "A backtest run failed.");
                    }
                    progress_bar_clone.inc(1);
                });
            }
        });
        
        progress_bar.finish_with_message("Optimization runs complete.");

        tracing::info!("Job {} complete. Run `analyze {}` to see the results.", self.job_id, self.job_id);
        
        Ok(())
    }

    async fn initialize_job(&self) -> Result<(), OptimizerError> {
        self.db_repo.save_optimization_job(
            self.job_id,
            &format!("{:?}", self.config.base_config.strategy_id),
            &self.config.base_config.symbol,
            "Running", // Set status to Running
        ).await?;

        let param_sets = generate_parameter_sets(&self.config)?;

        for params in param_sets {
            self.db_repo.save_backtest_run(
                Uuid::new_v4(),
                self.job_id,
                &params,
                "Pending",
            ).await?;
        }
        Ok(())
    }
    
    /// This is the core function that runs inside each parallel thread.
    async fn execute_single_backtest(&self, run: DbBacktestRun) -> Result<(), OptimizerError> {
        let run_id = run.run_id;
        
        let analytics_engine = analytics::AnalyticsEngine::new();
        let portfolio = Portfolio::new(self.base_config.backtest.initial_capital);
        let executor = Box::new(SimulatedExecutor::new(self.base_config.simulation.clone()));
        let risk_manager = Box::new(SimpleRiskManager::new(self.base_config.risk_management.clone())?);
        let strategy = self.create_strategy_instance(&run.parameters)?;

        // --- THE KEY CHANGE IS HERE ---
        // We now pass the `run_id` from the database task directly into the Backtester.
        let mut backtester = Backtester::new(
            run_id, // <-- PASS THE RUN ID
            self.config.base_config.symbol.clone(),
            self.config.base_config.interval.clone(),
            self.base_config.clone(), // Pass the full config for stop-loss access
            portfolio,
            strategy,
            risk_manager,
            executor,
            analytics_engine,
            self.db_repo.clone(),
        );
        // --- END OF CHANGE ---

        let backtest_result = backtester.run(
            self.base_config.backtest.start_date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Utc).unwrap(),
            self.base_config.backtest.end_date.and_hms_opt(23,59,59).unwrap().and_local_timezone(Utc).unwrap(),
        ).await;

        match backtest_result {
            Ok(_) => {
                // The backtester now saves its own results, so we only need to update the status.
                self.db_repo.update_run_status(run_id, "Completed").await?;
            }
            Err(e) => {
                tracing::error!(run_id = %run_id, error = ?e, "Backtest run failed.");
                self.db_repo.update_run_status(run_id, "Failed").await?;
            }
        }
        
        Ok(())
    }

    fn parse_decimal_param(val: &JsonValue, param_name: &str) -> Result<Decimal, OptimizerError> {
        if let Some(f64_val) = val.as_f64() {
            Decimal::from_f64(f64_val)
        } else if let Some(str_val) = val.as_str() {
            str_val.parse::<Decimal>().ok()
        } else {
            None
        }.ok_or_else(|| OptimizerError::ParameterGeneration(format!("Cannot parse {}: {:?}", param_name, val)))
    }

    pub fn create_strategy_instance(&self, optimized_params: &JsonValue) -> Result<Box<dyn strategies::Strategy>, OptimizerError> {
        let strategy_id = self.config.base_config.strategy_id;
        let mut temp_config = self.base_config.clone();
        
        let params_map: serde_json::Map<String, JsonValue> = serde_json::from_value(optimized_params.clone())?;

        match strategy_id {
            StrategyId::MACrossover => {
                let mut p = temp_config.strategies.ma_crossover.clone();
                if let Some(val) = params_map.get("ma_fast_period") { 
                    p.ma_fast_period = val.as_u64().ok_or_else(|| OptimizerError::ParameterGeneration("Invalid ma_fast_period".to_string()))? as usize; 
                }
                if let Some(val) = params_map.get("ma_slow_period") { 
                    p.ma_slow_period = val.as_u64().ok_or_else(|| OptimizerError::ParameterGeneration("Invalid ma_slow_period".to_string()))? as usize; 
                }
                if let Some(val) = params_map.get("trend_filter_period") { 
                    p.trend_filter_period = val.as_u64().ok_or_else(|| OptimizerError::ParameterGeneration("Invalid trend_filter_period".to_string()))? as usize; 
                }
                temp_config.strategies.ma_crossover = p;
            },
            StrategyId::SuperTrend => {
                let mut p = temp_config.strategies.super_trend.clone();
                if let Some(val) = params_map.get("atr_period") { 
                    p.atr_period = val.as_u64().ok_or_else(|| OptimizerError::ParameterGeneration("Invalid atr_period".to_string()))? as usize; 
                }
                if let Some(val) = params_map.get("atr_multiplier") { 
                    p.atr_multiplier = Self::parse_decimal_param(val, "atr_multiplier")?;
                }
                if let Some(val) = params_map.get("adx_threshold") { 
                    p.adx_threshold = Self::parse_decimal_param(val, "adx_threshold")?;
                }
                if let Some(val) = params_map.get("adx_period") { 
                    p.adx_period = val.as_u64().ok_or_else(|| OptimizerError::ParameterGeneration("Invalid adx_period".to_string()))? as usize; 
                }
                temp_config.strategies.super_trend = p;
            },
            StrategyId::ProbReversion => {
                let mut p = temp_config.strategies.prob_reversion.clone();
                if let Some(val) = params_map.get("bb_period") { 
                    p.bb_period = val.as_u64().ok_or_else(|| OptimizerError::ParameterGeneration("Invalid bb_period".to_string()))? as usize; 
                }
                if let Some(val) = params_map.get("bb_std_dev") { 
                    p.bb_std_dev = Self::parse_decimal_param(val, "bb_std_dev")?;
                }
                if let Some(val) = params_map.get("rsi_period") { 
                    p.rsi_period = val.as_u64().ok_or_else(|| OptimizerError::ParameterGeneration("Invalid rsi_period".to_string()))? as usize; 
                }
                if let Some(val) = params_map.get("rsi_oversold") { 
                    p.rsi_oversold = Self::parse_decimal_param(val, "rsi_oversold")?;
                }
                if let Some(val) = params_map.get("rsi_overbought") { 
                    p.rsi_overbought = Self::parse_decimal_param(val, "rsi_overbought")?;
                }
                if let Some(val) = params_map.get("adx_threshold") { 
                    p.adx_threshold = Self::parse_decimal_param(val, "adx_threshold")?;
                }
                if let Some(val) = params_map.get("adx_period") { 
                    p.adx_period = val.as_u64().ok_or_else(|| OptimizerError::ParameterGeneration("Invalid adx_period".to_string()))? as usize; 
                }
                temp_config.strategies.prob_reversion = p;
            },
            StrategyId::FundingRateArb => {
                let mut p = temp_config.strategies.funding_rate_arb.clone();
                if let Some(val) = params_map.get("target_rate_threshold") { 
                    p.target_rate_threshold = Self::parse_decimal_param(val, "target_rate_threshold")?;
                }
                if let Some(val) = params_map.get("basis_safety_threshold") { 
                    p.basis_safety_threshold = Self::parse_decimal_param(val, "basis_safety_threshold")?;
                }
                temp_config.strategies.funding_rate_arb = p;
            },
        }
        
        Ok(create_strategy(strategy_id, &temp_config, &self.config.base_config.symbol)?)
    }
}