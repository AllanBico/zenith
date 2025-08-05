use crate::error::WfoError;
use analyzer::Analyzer;
use backtester::Backtester;
use chrono::{DateTime, Duration, Utc};
use configuration::optimizer_config::{OptimizerConfig, WfoConfig};
use configuration::Config;
use database::DbRepository;
use executor::{Portfolio, SimulatedExecutor};
use optimizer::Optimizer;
use risk::SimpleRiskManager;
use analytics; // For AnalyticsEngine

use uuid::Uuid;

pub mod error;

/// A struct to hold the date ranges for a single walk-forward period.
struct WalkPeriod {
    is_start: DateTime<Utc>,
    is_end: DateTime<Utc>,
    oos_start: DateTime<Utc>,
    oos_end: DateTime<Utc>,
}

/// The master engine for orchestrating Walk-Forward Optimizations.
pub struct WfoEngine {
    wfo_job_id: Uuid,
    optimizer_config: OptimizerConfig,
    base_config: Config,
    db_repo: DbRepository,
}

impl WfoEngine {
    pub fn new(
        optimizer_config: OptimizerConfig,
        base_config: Config,
        db_repo: DbRepository,
    ) -> Self {
        Self {
            wfo_job_id: Uuid::new_v4(),
            optimizer_config,
            base_config,
            db_repo,
        }
    }

    /// The main entry point to run the entire WFO process.
    pub async fn run(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<(), WfoError> {
        let wfo_config = self.optimizer_config.wfo.as_ref().ok_or(WfoError::ConfigMissing)?;

        // 1. Create and save the master WFO job record
        self.db_repo.save_wfo_job(
            self.wfo_job_id,
            &format!("{:?}", self.optimizer_config.base_config.strategy_id),
            &self.optimizer_config.base_config.symbol,
            wfo_config.in_sample_weeks as i32,
            wfo_config.out_of_sample_weeks as i32,
            "Running",
        ).await?;

        // 2. Generate all the walk-forward periods
        let periods = self.generate_walk_forward_periods(start_date, end_date, wfo_config)?;

        println!("Starting WFO Job {} with {} walk-forward periods.", self.wfo_job_id, periods.len());

        // 3. Loop through each period and execute the walk
        for (i, period) in periods.iter().enumerate() {
            println!("\n--- Starting Walk {}/{} ---", i + 1, periods.len());
            println!("  In-Sample Period: {} -> {}", period.is_start.date_naive(), period.is_end.date_naive());
            println!("  Out-of-Sample Period: {} -> {}", period.oos_start.date_naive(), period.oos_end.date_naive());
            
            self.execute_walk(period).await?;
        }

        println!("\n--- WFO Job {} Completed Successfully! ---", self.wfo_job_id);
        Ok(())
    }

    /// Executes a single walk: Optimize on IS, Analyze, and Backtest on OOS.
    async fn execute_walk(&self, period: &WalkPeriod) -> Result<(), WfoError> {
        // A. Run In-Sample Optimization
        // We need to override the dates in the optimizer's run logic, which currently it doesn't support.
        // For now, we will create a temporary config for this step.
        // This highlights a need for future refactoring to make components more flexible.
        let mut is_temp_base_config = self.base_config.clone();
        is_temp_base_config.backtest.start_date = period.is_start.date_naive();
        is_temp_base_config.backtest.end_date = period.is_end.date_naive();

        // Let's assume an updated Optimizer that can take a date range, for simplicity here.
        // let is_job_id = is_optimizer.run(period.is_start, period.is_end).await?;
        // For now, we'll just run it with our temporary config.
        let is_optimizer = Optimizer::new(self.optimizer_config.clone(), is_temp_base_config, self.db_repo.clone());
        let is_job_id = is_optimizer.job_id(); // Get the job id for analysis
        is_optimizer.run().await?; // Run the optimization

        // B. Analyze IS results to find the best parameters
        let analyzer = Analyzer::new(self.optimizer_config.analysis.clone());
        let ranked_reports = analyzer.run(&self.db_repo, is_job_id).await?;
        
        let best_run = ranked_reports.first().ok_or_else(|| WfoError::NoBestParamsFound {
            start: period.is_start.to_string(),
            end: period.is_end.to_string(),
        })?;
        let best_params = best_run.report.parameters.clone();
        println!("  Found best IS params: {}", best_params);

        // C. Run Out-of-Sample Backtest with the best parameters
        let oos_run_id = Uuid::new_v4();
        
        let portfolio = Portfolio::new(self.base_config.backtest.initial_capital);
        let executor = Box::new(SimulatedExecutor::new(self.base_config.simulation.clone()));
        let risk_manager = Box::new(SimpleRiskManager::new(self.base_config.risk_management.clone())?);
        let analytics_engine = analytics::AnalyticsEngine::new();

        // Create the strategy instance with the BEST params found by the analyzer
        let strategy = is_optimizer.create_strategy_instance(&best_params)?;
        
        self.db_repo.save_backtest_run(oos_run_id, is_job_id, &best_params, "Pending").await?;
        
        let mut oos_backtester = Backtester::new(
            oos_run_id,
            self.optimizer_config.base_config.symbol.clone(),
            self.optimizer_config.base_config.interval.clone(),
            self.base_config.clone(), // Pass the full config for stop-loss access
            portfolio,
            strategy,
            risk_manager,
            executor,
            analytics_engine,
            self.db_repo.clone(),
        );
        
        oos_backtester.run(period.oos_start, period.oos_end).await?;
        self.db_repo.update_run_status(oos_run_id, "Completed").await?;
        println!("  Completed OOS backtest for Run ID: {}", oos_run_id);
        
        // D. Save the WFO run record, linking everything together
        self.db_repo.save_wfo_run(
            Uuid::new_v4(),
            self.wfo_job_id,
            oos_run_id,
            &best_params,
            period.oos_start,
            period.oos_end,
        ).await?;

        Ok(())
    }

    /// Generates a vector of non-overlapping walk-forward periods.
    fn generate_walk_forward_periods(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        config: &WfoConfig,
    ) -> Result<Vec<WalkPeriod>, WfoError> {
        let mut periods = Vec::new();
        let mut current_start = start_date;

        loop {
            let is_start = current_start;
            let is_end = is_start + Duration::weeks(config.in_sample_weeks);
            
            let oos_start = is_end;
            let oos_end = oos_start + Duration::weeks(config.out_of_sample_weeks);

            if oos_end > end_date {
                break; // This period would extend beyond our total range
            }

            periods.push(WalkPeriod { is_start, is_end, oos_start, oos_end });
            
            current_start = oos_start; // The next walk starts where this one's OOS period began
        }
        
        if periods.is_empty() {
             return Err(WfoError::DateError("Total date range is too short to generate a single walk-forward period.".to_string()));
        }

        Ok(periods)
    }
}