use anyhow::Result;
use api_client::{ApiClient, BinanceClient};
use backtester::Backtester;
use chrono::{DateTime, NaiveDate, Utc, Duration, Datelike};
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use configuration::{load_config, load_live_config, load_optimizer_config, load_portfolio_config, PortfolioBotConfig, MACrossoverParams, ProbReversionParams, SuperTrendParams, ExecutionMode};
use database::{connect, run_migrations, DbRepository};
use engine::LiveEngine;
use executor::{Portfolio, SimulatedExecutor, LiveExecutor};
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use optimizer::Optimizer;
use portfolio_backtester::{load_and_prepare_data, PortfolioManager};
use risk::SimpleRiskManager;
use serde_json::{from_value, json, Value as JsonValue};
use strategies::{create_strategy, StrategyId};
use std::collections::HashMap;
use std::net::SocketAddr; // For parsing socket addresses
use std::ops::Add;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;
use tracing;
use analyzer::Analyzer;
use wfo::WfoEngine;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration first to get logging settings
    let config = configuration::load_config(None)?;
    
    // --- TRACING INITIALIZATION ---
    // Initialize tracing based on the configuration file settings
    configuration::init_tracing(&config.logging)?;
    // --- END INITIALIZATION ---

    dotenvy::dotenv().expect(".env file not found");

    // Note: DB connection is now handled by the commands that need it.

    let cli = Cli::parse();

    match cli.command {
        Commands::Backfill(args) => handle_backfill(args).await?,
        Commands::SingleRun(args) => handle_single_run(args).await?,
        Commands::Optimize(args) => handle_optimize(args).await?,
        Commands::Analyze(args) => handle_analyze(args).await?,
        Commands::Wfo(args) => handle_wfo(args).await?,
        Commands::PortfolioRun(args) => handle_portfolio_run(args).await?,
        Commands::Run(args) => handle_run(args).await?,
        Commands::Serve(args) => handle_serve(args).await?, // New command
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
    PortfolioRun(PortfolioRunArgs),
    Run(RunArgs),
    /// Start the web server to serve the API.
    Serve(ServeArgs),
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
    #[arg(long)]
    from: Option<NaiveDate>,
    #[arg(long)]
    to: Option<NaiveDate>,
    #[arg(long, short, default_value = "portfolio.toml")]
    portfolio: PathBuf,
}

#[derive(Parser)]
struct RunArgs {
    /// The execution mode for the engine.
    #[arg(long, value_enum, default_value_t = ExecutionMode::Paper)]
    mode: ExecutionMode,

    /// Path to the live trading configuration file.
    #[arg(long, short, default_value = "live.toml")]
    config: PathBuf,
}

#[derive(Parser)]
struct ServeArgs {
    /// The IP address and port to bind the server to.
    #[arg(long, short, default_value = "0.0.0.0:3000")]
    addr: SocketAddr,
}

// ==============================================================================
// Command Handlers
// ==============================================================================

/// Handler for the `serve` command.
async fn handle_serve(args: ServeArgs) -> Result<()> {
    // We call the library function from our `web-server` crate.
    // Note: We need to modify the `run_server` function to accept the address.
    web_server::run_server(args.addr).await
}

async fn handle_run(args: RunArgs) -> Result<()> {
    // 1. Load Configurations
    let base_config = load_config(None)?;
    let live_config = load_live_config(&args.config)?;

    // 2. Instantiate shared components
    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);

    // --- MASTER EXECUTION LOGIC ---
    let (executor, api_client): (Arc<dyn executor::Executor>, Arc<dyn ApiClient>) =
        match args.mode {
            ExecutionMode::Paper => {
                tracing::info!("INITIALIZING IN PAPER TRADING MODE");
                tracing::info!(">> Live data feed | Simulated local execution <<");

                // In Paper mode, the executor is the SIMULATED one.
                let simulated_executor = Arc::new(SimulatedExecutor::new(base_config.simulation.clone()));

                // We still need an API client for the StateReconciler, which should use the Testnet.
                let api_client = Arc::new(BinanceClient::new(false, &base_config.api)); // false = Testnet

                (simulated_executor, api_client)
            }
            ExecutionMode::Testnet => {
                tracing::warn!("INITIALIZING IN TESTNET TRADING MODE");
                tracing::warn!(">> Live data feed | REAL orders sent to Binance TESTNET <<");

                // In Testnet mode, the executor is the LIVE one, pointing to the Testnet API client.
                let binance_client = BinanceClient::new(false, &base_config.api); // false = Testnet
                let api_client = Arc::new(binance_client) as Arc<dyn ApiClient>;
                let live_executor = Arc::new(LiveExecutor::new(Arc::clone(&api_client)));

                (live_executor, api_client)
            }
            ExecutionMode::Live => {
                tracing::error!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
                tracing::error!("INITIALIZING IN LIVE TRADING MODE");
                tracing::error!(">> REAL MONEY IS AT RISK. PROCEED WITH CAUTION. <<");
                tracing::error!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");

                // OBEY THE MASTER SAFETY SWITCH
                if !live_config.live_trading_enabled {
                    anyhow::bail!("FATAL: Attempted to run in Live mode, but `live_trading_enabled` is false in live.toml. Aborting.");
                }

                // In Live mode, the executor is the LIVE one, pointing to the PRODUCTION API client.
                let binance_client = BinanceClient::new(true, &base_config.api); // true = Production
                let api_client = Arc::new(binance_client) as Arc<dyn ApiClient>;
                let live_executor = Arc::new(LiveExecutor::new(Arc::clone(&api_client)));

                // Add a 5-second countdown to allow the user to abort.
                tracing::info!("Starting in 5 seconds... Press Ctrl+C to cancel.");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                (live_executor, api_client)
            }
        };

    // 4. Create and Run the Engine (this part is now generic)
    // The engine doesn't know or care which executor it was given.
    let risk_manager = Arc::new(SimpleRiskManager::new(base_config.risk_management.clone())?);

    // This is the missing piece from the next task, we add it here now.
    let mut engine = LiveEngine::new(
        live_config,
        base_config,
        api_client,
        executor, // Pass in the generic executor
        db_repo,
        risk_manager,
    );

    engine.run().await?;
    
    tracing::info!("Engine has stopped.");
    Ok(())
}


// ... (all other handler functions now need to initialize their own DB connection) ...

// Example modification for one handler:
async fn handle_backfill(args: BackfillArgs) -> Result<()> {
    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);
    // ... rest of the function ...
    tracing::info!(
        "Starting backfill for {} on interval {} from {} to {}",
        args.symbol, args.from, args.interval, args.to
    );

    let api_client = BinanceClient::new(false, &load_config(None)?.api);

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
            let api_client_clone = api_client.clone();
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
            tracing::error!(error = %e, "A task failed.");
        }
    }

    Ok(())
}

async fn handle_portfolio_run(args: PortfolioRunArgs) -> Result<()> {
    tracing::info!("---===[ Starting Portfolio-Level Backtest ]===---");

    let base_config = load_config(None)?;
    let portfolio_config = load_portfolio_config(&args.portfolio)?;
    tracing::info!("Loaded portfolio definition with {} bots.", portfolio_config.bots.len());

    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);
    let analytics_engine = analytics::AnalyticsEngine::new();
    let portfolio = Portfolio::new(base_config.backtest.initial_capital);
    let executor = Box::new(SimulatedExecutor::new(base_config.simulation.clone()));
    let risk_manager = Box::new(SimpleRiskManager::new(base_config.risk_management.clone())?);

    let start_date = args.from.unwrap_or(base_config.backtest.start_date);
    let end_date = args.to.unwrap_or(base_config.backtest.end_date);
    let interval = &base_config.backtest.interval;
    tracing::info!("Loading and merging data from {} to {}...", start_date, end_date);
    let event_stream = load_and_prepare_data(
        &portfolio_config,
        &db_repo,
        interval,
        start_date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Utc).unwrap(),
        end_date.and_hms_opt(23,59,59).unwrap().and_local_timezone(Utc).unwrap(),
    ).await?;
    tracing::info!("Master event stream created with {} events.", event_stream.len());

    let mut strategies = HashMap::<String, Box<dyn strategies::Strategy>>::new();
    for bot_config in portfolio_config.bots {
        let strategy = create_strategy_from_portfolio_config(&base_config, &bot_config)?;
        strategies.insert(bot_config.symbol, strategy);
    }

    let mut manager = PortfolioManager::new(
        base_config,
        portfolio,
        risk_manager,
        executor,
        analytics_engine,
        strategies,
    );
    
    let report = manager.run(event_stream).await?;

    tracing::info!("---===[ Portfolio Backtest Report ]===---");
    tracing::info!("{:#?}", report);
    
    Ok(())
}
fn create_strategy_from_portfolio_config(
    base_config: &configuration::Config,
    bot_config: &PortfolioBotConfig,
) -> Result<Box<dyn strategies::Strategy>> {
    let mut temp_config = base_config.clone();
    
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
async fn handle_wfo(args: WfoArgs) -> Result<()> {
    tracing::info!("---===[ Starting Walk-Forward Optimization Job ]===---");

    let base_config = load_config(None)?;
    let optimizer_config = load_optimizer_config(&args.config)?;

    if optimizer_config.wfo.is_none() {
        anyhow::bail!("The `[wfo]` section is missing from the optimizer config file. Cannot run a WFO job.");
    }

    let start_date = args.from.unwrap_or(base_config.backtest.start_date);
    let end_date = args.to.unwrap_or(base_config.backtest.end_date);
    
    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);
    let wfo_engine = WfoEngine::new(optimizer_config, base_config, db_repo);
    
    wfo_engine.run(
        start_date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Utc).unwrap(),
        end_date.and_hms_opt(23,59,59).unwrap().and_local_timezone(Utc).unwrap()
    ).await?;

    Ok(())
}
async fn handle_analyze(args: AnalyzeArgs) -> Result<()> {
    tracing::info!("---===[ Analyzing Optimization Job: {} ]===---", args.job_id);

    let optimizer_config = load_optimizer_config(&args.config)?;
    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);
    let analyzer = Analyzer::new(optimizer_config.analysis);

    let ranked_reports = analyzer.run(&db_repo, args.job_id).await?;

    if ranked_reports.is_empty() {
        tracing::warn!("No reports found for this job, or all were filtered out.");
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

    tracing::info!("{table}");
    Ok(())
}
async fn handle_optimize(args: OptimizeArgs) -> Result<()> {
    tracing::info!("---===[ Starting Optimization Job ]===---");

    tracing::info!("Loading base configuration from config.toml...");
    let base_config = load_config(None)?;
    
    tracing::info!("Loading optimization job from: {:?}", &args.config);
    let optimizer_config = load_optimizer_config(&args.config)?;

    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);

    let optimizer = Optimizer::new(optimizer_config, base_config, db_repo);
    
    optimizer.run().await?;
    
    tracing::info!("Optimization process finished.");
    Ok(())
}
/// Generates parameters for a strategy based on the configuration.
fn generate_strategy_params(config: &configuration::Config, strategy_id: StrategyId) -> Result<JsonValue> {
    match strategy_id {
        StrategyId::MACrossover => {
            Ok(json!({
                "ma_fast_period": config.strategies.ma_crossover.ma_fast_period,
                "ma_slow_period": config.strategies.ma_crossover.ma_slow_period,
                "trend_filter_period": config.strategies.ma_crossover.trend_filter_period,
            }))
        },
        StrategyId::SuperTrend => {
            Ok(json!({
                "atr_period": config.strategies.super_trend.atr_period,
                "atr_multiplier": config.strategies.super_trend.atr_multiplier,
                "adx_threshold": config.strategies.super_trend.adx_threshold,
                "adx_period": config.strategies.super_trend.adx_period,
            }))
        },
        StrategyId::ProbReversion => {
            Ok(json!({
                "bb_period": config.strategies.prob_reversion.bb_period,
                "bb_std_dev": config.strategies.prob_reversion.bb_std_dev,
                "rsi_period": config.strategies.prob_reversion.rsi_period,
                "rsi_oversold": config.strategies.prob_reversion.rsi_oversold,
                "rsi_overbought": config.strategies.prob_reversion.rsi_overbought,
                "adx_threshold": config.strategies.prob_reversion.adx_threshold,
                "adx_period": config.strategies.prob_reversion.adx_period,
            }))
        },
        StrategyId::FundingRateArb => {
            Ok(json!({
                "target_rate_threshold": config.strategies.funding_rate_arb.target_rate_threshold,
                "basis_safety_threshold": config.strategies.funding_rate_arb.basis_safety_threshold,
            }))
        },
    }
}

async fn handle_single_run(args: SingleRunArgs) -> Result<()> {
    let config = load_config(None)?;
    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);

    tracing::info!("---===[ Starting Single Backtest Run ]===---");

    let job_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let strategy_id = config.backtest.strategy_id;

    let params = generate_strategy_params(&config, strategy_id)?;
    
    db_repo.save_optimization_job(
        job_id,
        &format!("{:?}", strategy_id),
        &config.backtest.symbol,
        "Single Run",
    ).await?;
    
    db_repo.save_backtest_run(run_id, job_id, &params, "Pending").await?;
    tracing::info!("Created database record for Run ID: {}", run_id);

    let backtest_config = config.backtest.clone();
    let start_date = args.from.unwrap_or(backtest_config.start_date);
    let end_date = args.to.unwrap_or(backtest_config.end_date);
    let symbol = backtest_config.symbol.clone();
    let interval = backtest_config.interval.clone();

    tracing::info!("Period: {} to {}", start_date, end_date);
    tracing::info!("Symbol: {}, Interval: {}", symbol, interval);

    let analytics_engine = analytics::AnalyticsEngine::new();
    let portfolio = Portfolio::new(backtest_config.initial_capital);
    let executor = Box::new(SimulatedExecutor::new(config.simulation.clone()));
    let risk_manager = Box::new(SimpleRiskManager::new(config.risk_management.clone())?);
    let strategy = create_strategy(strategy_id, &config, &config.backtest.symbol)?;
    tracing::info!("Strategy: {:?}", strategy_id);

    let mut backtester = Backtester::new(
        run_id,
        symbol,
        interval,
        config, // Pass the full config for stop-loss access
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
            tracing::info!("---===[ Backtest Report (Run ID: {}) ]===---", run_id);
            tracing::info!("{:#?}", report);
        }
        Err(e) => {
            db_repo.update_run_status(run_id, "Failed").await?;
            tracing::error!(run_id = %run_id, "Backtest Failed.");
            tracing::error!(error = ?e, "Error.");
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