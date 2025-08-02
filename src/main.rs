use anyhow::Result;
use api_client::{ApiClient, BinanceClient};
use backtester::Backtester;
use chrono::{DateTime, NaiveDate, Utc, Datelike, Duration};
use std::ops::Add;
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use configuration::{load_config, load_optimizer_config};
use database::{connect, run_migrations, DbRepository};
use executor::{Portfolio, SimulatedExecutor};
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use optimizer::Optimizer;
use risk::SimpleRiskManager;
use serde_json::json;
use strategies::create_strategy;
use std::path::PathBuf;
use uuid::Uuid;
use analyzer::Analyzer;

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
    }

    Ok(())
}

// ... (CLI struct definitions are unchanged) ...
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
}

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


// ==============================================================================
// Command Handlers
// ==============================================================================

/// The handler for the `single-run` command, now updated to persist its results.
async fn handle_single_run(args: SingleRunArgs, db_pool: sqlx::PgPool) -> Result<()> {
    let config = load_config(None)?;
    let db_repo = DbRepository::new(db_pool);

    println!("---===[ Starting Single Backtest Run ]===---");

    // 1. Create database records for this run
    let job_id = Uuid::new_v4(); // Each single run gets its own "job" for now
    let run_id = Uuid::new_v4();
    let strategy_id = strategies::StrategyId::MACrossover; // Hardcoded for now

    // Create a JSON object of the parameters being used from the config file
    let params = json!({
        "ma_fast_period": config.strategies.ma_crossover.ma_fast_period,
        "ma_slow_period": config.strategies.ma_crossover.ma_slow_period,
        "trend_filter_period": config.strategies.ma_crossover.trend_filter_period,
    });
    
    // Save a placeholder "job" for this single run
    db_repo.save_optimization_job(
        job_id,
        &format!("{:?}", strategy_id),
        &config.backtest.symbol,
        "Single Run",
    ).await?;
    
    // Save the "Pending" run record
    db_repo.save_backtest_run(run_id, job_id, &params, "Pending").await?;
    println!("Created database record for Run ID: {}", run_id);

    // 2. Set up parameters
    let backtest_config = config.backtest.clone();
    let start_date = args.from.unwrap_or(backtest_config.start_date);
    let end_date = args.to.unwrap_or(backtest_config.end_date);
    let symbol = backtest_config.symbol.clone();
    let interval = backtest_config.interval.clone();

    println!("Period: {} to {}", start_date, end_date);
    println!("Symbol: {}, Interval: {}", symbol, interval);

    // 3. Instantiate components
    let analytics_engine = analytics::AnalyticsEngine::new();
    let portfolio = Portfolio::new(backtest_config.initial_capital);
    let executor = Box::new(SimulatedExecutor::new(config.simulation.clone()));
    let risk_manager = Box::new(SimpleRiskManager::new(config.risk_management.clone())?);
    let strategy = create_strategy(strategy_id, &config, &config.backtest.symbol)?;
    println!("Strategy: {:?}", strategy_id);

    // 4. Create and run the backtester, passing in the run_id
    let mut backtester = Backtester::new(
        run_id, // <-- PASS THE RUN ID
        symbol,
        interval,
        portfolio,
        strategy,
        risk_manager,
        executor,
        analytics_engine,
        db_repo.clone(), // Clone the repo for the backtester
    );
    
    let report_result = backtester.run(
        start_date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Utc).unwrap(),
        end_date.and_hms_opt(23,59,59).unwrap().and_local_timezone(Utc).unwrap()
    ).await;

    // 5. Update status and print report
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


// ... (handle_analyze, handle_optimize, handle_backfill, and generate_monthly_ranges are unchanged)
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

async fn handle_backfill(args: BackfillArgs, db_pool: sqlx::PgPool) -> Result<()> {
    println!(
        "Starting backfill for {} on interval {} from {} to {}",
        args.symbol, args.from, args.interval, args.to
    );

    let db_repo = DbRepository::new(db_pool);
    let api_client = BinanceClient::new();

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