use anyhow::Result;
use api_client::{ApiClient, BinanceClient};
use backtester::Backtester;
use chrono::{DateTime, NaiveDate, Utc, Datelike, Duration};
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use configuration::{load_config, load_optimizer_config, PortfolioBotConfig}; // Fixed import
use database::{connect, run_migrations, DbRepository};
use executor::{Portfolio, SimulatedExecutor};
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use optimizer::Optimizer;
use portfolio_backtester::{load_and_prepare_data, PortfolioManager}; // Fixed import
use risk::SimpleRiskManager;
use serde_json::from_value;
use strategies::{create_strategy, StrategyId};
use configuration::{MACrossoverParams, ProbReversionParams, SuperTrendParams}; // Fixed import
use std::collections::HashMap;
use std::ops::Add;
use std::path::PathBuf;
use uuid::Uuid;
use analyzer::Analyzer;
use wfo::WfoEngine;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().expect(".env file not found");

    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Backfill(args) => handle_backfill(args, db_pool).await?,
        Commands::SingleRun(args) => handle_single_run(args, db_pool).await?,
        Commands::Optimize(args) => handle_optimize(args, db_pool).await?,
        Commands::Analyze(args) => handle_analyze(args, db_pool).await?,
        Commands::Wfo(args) => handle_wfo(args, db_pool).await?,
        Commands::PortfolioRun(args) => handle_portfolio_run(args, db_pool).await?, // New command
    }

    Ok(())
}

// ==============================================================================
// CLI Structure
// ==============================================================================

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Backfill(BackfillArgs),
    SingleRun(SingleRunArgs),
    Optimize(OptimizeArgs),
    Analyze(AnalyzeArgs),
    Wfo(WfoArgs),
    /// Run a portfolio-level backtest from a portfolio definition file.
    PortfolioRun(PortfolioRunArgs),
}

// ... (Other arg structs are unchanged) ...
#[derive(Parser)]
struct BackfillArgs {
    #[arg(long)]
    symbol: String,
    #[arg(long)]
    interval: String,
    #[arg(long)]
    from: NaiveDate,
    #[arg(long)]
    to: NaiveDate,
}

#[derive(Parser)]
struct SingleRunArgs {
    #[arg(long)]
    from: Option<NaiveDate>,
    #[arg(long)]
    to: Option<NaiveDate>,
}

#[derive(Parser)]
struct OptimizeArgs {
    #[arg(long, short, default_value = "optimizer.toml")]
    config: PathBuf,
}

#[derive(Parser)]
struct AnalyzeArgs {
    job_id: Uuid,
    #[arg(long, short, default_value = "optimizer.toml")]
    config: PathBuf,
}

#[derive(Parser)]
struct WfoArgs {
    #[arg(long)]
    from: Option<NaiveDate>,
    #[arg(long)]
    to: Option<NaiveDate>,
    #[arg(long, short, default_value = "optimizer.toml")]
    config: PathBuf,
}

#[derive(Parser)]
struct PortfolioRunArgs {
    /// Optional start date to override config default (YYYY-MM-DD).
    #[arg(long)]
    from: Option<NaiveDate>,
    /// Optional end date to override config default (YYYY-MM-DD).
    #[arg(long)]
    to: Option<NaiveDate>,
    /// Path to the portfolio definition file.
    #[arg(long, short, default_value = "portfolio.toml")]
    portfolio: PathBuf,
}

// ==============================================================================
// Command Handlers
// ==============================================================================

async fn handle_portfolio_run(args: PortfolioRunArgs, db_pool: sqlx::PgPool) -> Result<()> {
    println!("---===[ Starting Portfolio-Level Backtest ]===---");

    // 1. Load Configurations
    let base_config = load_config(None)?;
    let portfolio_config = configuration::load_portfolio_config(&args.portfolio)?;
    println!("Loaded portfolio definition with {} bots.", portfolio_config.bots.len());

    // 2. Instantiate Shared Components
    let db_repo = DbRepository::new(db_pool);
    let analytics_engine = analytics::AnalyticsEngine::new();
    let portfolio = Portfolio::new(base_config.backtest.initial_capital);
    let executor = Box::new(SimulatedExecutor::new(base_config.simulation.clone()));
    let risk_manager = Box::new(SimpleRiskManager::new(base_config.risk_management.clone())?);

    // 3. Load and Prepare Master Event Stream
    let start_date = args.from.unwrap_or(base_config.backtest.start_date);
    let end_date = args.to.unwrap_or(base_config.backtest.end_date);
    let interval = &base_config.backtest.interval;
    println!("Loading and merging data from {} to {}...", start_date, end_date);
    let event_stream = load_and_prepare_data(
        &portfolio_config,
        &db_repo,
        interval,
        start_date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Utc).unwrap(),
        end_date.and_hms_opt(23,59,59).unwrap().and_local_timezone(Utc).unwrap(),
    ).await?;
    println!("Master event stream created with {} events.", event_stream.len());

    // 4. Instantiate All Strategies
    let mut strategies = HashMap::<String, Box<dyn strategies::Strategy>>::new();
    for bot_config in portfolio_config.bots {
        let strategy = create_strategy_from_portfolio_config(&base_config, &bot_config)?;
        strategies.insert(bot_config.symbol, strategy);
    }

    // 5. Instantiate and Run the Portfolio Manager
    let mut manager = PortfolioManager::new(
        base_config,
        portfolio,
        risk_manager,
        executor,
        analytics_engine,
        strategies,
    );
    
    let report = manager.run(event_stream).await?;

    // 6. Display the final, unified report
    println!("\n---===[ Portfolio Backtest Report ]===---");
    println!("{:#?}", report);
    
    Ok(())
}

/// Helper function to create a strategy instance from a bot configuration.
/// It works by creating a temporary, modified copy of the base config.
fn create_strategy_from_portfolio_config(
    base_config: &configuration::Config,
    bot_config: &PortfolioBotConfig,
) -> Result<Box<dyn strategies::Strategy>> {
    let mut temp_config = base_config.clone();
    
    // Merge the specific bot params into the temporary config
    match bot_config.strategy_id {
        StrategyId::MACrossover => {
            let params: MACrossoverParams = from_value(bot_config.params.clone())?;
            temp_config.strategies.ma_crossover = params;
        }
        StrategyId::SuperTrend => {
            let params: SuperTrendParams = from_value(bot_config.params.clone())?;
            temp_config.strategies.super_trend = params;
        }
        StrategyId::ProbReversion => {
            let params: ProbReversionParams = from_value(bot_config.params.clone())?;
            temp_config.strategies.prob_reversion = params;
        }
        _ => anyhow::bail!("Portfolio backtesting for this strategy is not yet supported."),
    }

    Ok(create_strategy(bot_config.strategy_id, &temp_config, &bot_config.symbol)?)
}


// ... (all other handler functions are unchanged) ...
async fn handle_wfo(args: WfoArgs, db_pool: sqlx::PgPool) -> Result<()> {
    println!("---===[ Starting Walk-Forward Optimization Job ]===---");

    let base_config = load_config(None)?;
    let optimizer_config = load_optimizer_config(&args.config)?;

    if optimizer_config.wfo.is_none() {
        anyhow::bail!("The `[wfo]` section is missing from the optimizer config file. Cannot run a WFO job.");
    }

    let start_date = args.from.unwrap_or(base_config.backtest.start_date);
    let end_date = args.to.unwrap_or(base_config.backtest.end_date);
    
    let db_repo = DbRepository::new(db_pool);
    let wfo_engine = WfoEngine::new(optimizer_config, base_config, db_repo);
    
    wfo_engine.run(
        start_date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Utc).unwrap(),
        end_date.and_hms_opt(23,59,59).unwrap().and_local_timezone(Utc).unwrap()
    ).await?;

    Ok(())
}
async fn handle_analyze(args: AnalyzeArgs, db_pool: sqlx::PgPool) -> Result<()> {
    println!("---===[ Analyzing Optimization Job: {} ]===---", args.job_id);

    let optimizer_config = load_optimizer_config(&args.config)?;
    let db_repo = DbRepository::new(db_pool);
    let analyzer = Analyzer::new(optimizer_config.analysis);

    let ranked_reports = analyzer.run(&db_repo, args.job_id).await?;

    if ranked_reports.is_empty() {
        println!("No reports found for this job, or all were filtered out.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Rank", "Score", "Net Profit", "Drawdown %", "Calmar", "Profit Factor", "Trades", "Params",
        ]);

    for (i, ranked) in ranked_reports.iter().take(20).enumerate() {
        table.add_row(vec![
            Cell::new(i + 1),
            Cell::new(format!("{:.4}", ranked.score)),
            Cell::new(format!("{:.2}", ranked.report.total_net_profit.unwrap_or_default())),
            Cell::new(format!("{:.2}%", ranked.report.max_drawdown_pct.unwrap_or_default())),
            Cell::new(format!("{:.2}", ranked.report.calmar_ratio.unwrap_or_default())),
            Cell::new(format!("{:.2}", ranked.report.profit_factor.unwrap_or_default())),
            Cell::new(ranked.report.total_trades.unwrap_or_default()),
            Cell::new(ranked.report.parameters.to_string()),
        ]);
    }

    println!("{table}");
    Ok(())
}
async fn handle_optimize(args: OptimizeArgs, db_pool: sqlx::PgPool) -> Result<()> {
    println!("---===[ Starting Optimization Job ]===---");

    println!("Loading base configuration from config.toml...");
    let base_config = load_config(None)?;
    
    println!("Loading optimization job from: {:?}", &args.config);
    let optimizer_config = load_optimizer_config(&args.config)?;

    let db_repo = DbRepository::new(db_pool);

    let optimizer = Optimizer::new(optimizer_config, base_config, db_repo);
    
    optimizer.run().await?;
    
    println!("\nOptimization process finished.");
    Ok(())
}
async fn handle_single_run(args: SingleRunArgs, db_pool: sqlx::PgPool) -> Result<()> {
    let config = load_config(None)?;
    let db_repo = DbRepository::new(db_pool);

    println!("---===[ Starting Single Backtest Run ]===---");

    let job_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let strategy_id = strategies::StrategyId::MACrossover;

    let params = serde_json::json!({
        "ma_fast_period": config.strategies.ma_crossover.ma_fast_period,
        "ma_slow_period": config.strategies.ma_crossover.ma_slow_period,
        "trend_filter_period": config.strategies.ma_crossover.trend_filter_period,
    });
    
    db_repo.save_optimization_job(
        job_id,
        &format!("{:?}", strategy_id),
        &config.backtest.symbol,
        "Single Run",
    ).await?;
    
    db_repo.save_backtest_run(run_id, job_id, &params, "Pending").await?;
    println!("Created database record for Run ID: {}", run_id);

    let backtest_config = config.backtest.clone();
    let start_date = args.from.unwrap_or(backtest_config.start_date);
    let end_date = args.to.unwrap_or(backtest_config.end_date);
    let symbol = backtest_config.symbol.clone();
    let interval = backtest_config.interval.clone();

    println!("Period: {} to {}", start_date, end_date);
    println!("Symbol: {}, Interval: {}", symbol, interval);

    let analytics_engine = analytics::AnalyticsEngine::new();
    let portfolio = Portfolio::new(backtest_config.initial_capital);
    let executor = Box::new(SimulatedExecutor::new(config.simulation.clone()));
    let risk_manager = Box::new(SimpleRiskManager::new(config.risk_management.clone())?);
    let strategy = create_strategy(strategy_id, &config, &symbol)?;
    println!("Strategy: {:?}", strategy_id);

    let mut backtester = Backtester::new(
        run_id,
        symbol,
        interval,
        portfolio,
        strategy,
        risk_manager,
        executor,
        analytics_engine,
        db_repo.clone(),
    );
    
    let report_result = backtester.run(
        start_date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Utc).unwrap(),
        end_date.and_hms_opt(23,59,59).unwrap().and_local_timezone(Utc).unwrap()
    ).await;

    match report_result {
        Ok(report) => {
            db_repo.update_run_status(run_id, "Completed").await?;
            println!("\n---===[ Backtest Report (Run ID: {}) ]===---", run_id);
            println!("{:#?}", report);
        }
        Err(e) => {
            db_repo.update_run_status(run_id, "Failed").await?;
            eprintln!("\n---===[ Backtest Failed (Run ID: {}) ]===---", run_id);
            eprintln!("Error: {:?}", e);
        }
    }

    Ok(())
}
async fn handle_backfill(args: BackfillArgs, db_pool: sqlx::PgPool) -> Result<()> {
    println!(
        "Starting backfill for {} on interval {} from {} to {}",
        args.symbol, args.from, args.interval, args.to
    );

    let db_repo = DbRepository::new(db_pool);
    let _api_client = BinanceClient::new();

    let date_ranges = generate_monthly_ranges(args.from, args.to);
    
    let progress_bar = ProgressBar::new(date_ranges.len() as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("#>-"),
    );

    let tasks: Vec<_> = date_ranges
        .into_iter()
        .map(|(start, end)| {
            let api_client_clone = BinanceClient::new();
            let db_repo_clone = db_repo.clone();
            let symbol = args.symbol.clone();
            let interval = args.interval.clone();
            let pb_clone = progress_bar.clone();

            tokio::spawn(async move {
                pb_clone.set_message(format!("Fetching {}...", start.format("%Y-%m")));
                let klines = api_client_clone.fetch_klines(&symbol, &interval, start, end).await?;
                
                for kline in klines {
                    db_repo_clone.save_kline(&symbol, &kline).await?;
                }
                
                pb_clone.inc(1);
                pb_clone.set_message(format!("Done {}!", start.format("%Y-%m")));
                Ok::<(), anyhow::Error>(())
            })
        })
        .collect();

    let results = join_all(tasks).await;

    progress_bar.finish_with_message("Backfill complete!");

    for result in results {
        if let Err(e) = result {
            eprintln!("A task failed: {}", e);
        }
    }

    Ok(())
}

fn generate_monthly_ranges(
    mut from: NaiveDate,
    to: NaiveDate,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut ranges = Vec::new();
    while from <= to {
        let end_of_month = from
            .with_day(1)
            .unwrap()
            .with_month(from.month() + 1)
            .unwrap_or_else(|| from.with_year(from.year() + 1).unwrap().with_month(1).unwrap())
            .pred_opt().unwrap();
        
        let end_date = std::cmp::min(end_of_month, to);
        ranges.push((
            from.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Utc).unwrap(),
            end_date.and_hms_opt(23, 59, 59).unwrap().and_local_timezone(Utc).unwrap(),
        ));
        
        from = end_date.add(Duration::days(1));
    }
    ranges
}