use anyhow::Result;
use api_client::{ApiClient, BinanceClient};
use backtester::Backtester;
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table}; // For table output
use configuration::{load_config, load_optimizer_config};
use database::{connect, run_migrations, DbRepository};
use executor::{Portfolio, SimulatedExecutor};
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use optimizer::Optimizer;
use risk::SimpleRiskManager;
use strategies::create_strategy;
use std::path::PathBuf;
use uuid::Uuid; // For parsing Job ID
use analyzer::Analyzer; // Import the Analyzer type

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
        Commands::Analyze(args) => handle_analyze(args, db_pool).await?, // New command
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
    /// Analyze the results of a completed optimization job.
    Analyze(AnalyzeArgs),
}

// ... (BackfillArgs and SingleRunArgs are unchanged)
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
    /// The Job ID of the optimization run to analyze.
    job_id: Uuid,
    /// Optional: Path to the optimizer config file used for the job (to get analysis rules).
    #[arg(long, short, default_value = "optimizer.toml")]
    config: PathBuf,
}


// ==============================================================================
// Command Handlers
// ==============================================================================

async fn handle_analyze(args: AnalyzeArgs, db_pool: sqlx::PgPool) -> Result<()> {
    println!("---===[ Analyzing Optimization Job: {} ]===---", args.job_id);

    // 1. Load config and instantiate components
    let optimizer_config = load_optimizer_config(&args.config)?;
    let db_repo = DbRepository::new(db_pool);
    let analyzer = Analyzer::new(optimizer_config.analysis);

    // 2. Run the analysis
    let ranked_reports = analyzer.run(&db_repo, args.job_id).await?;

    if ranked_reports.is_empty() {
        println!("No reports found for this job, or all were filtered out.");
        return Ok(());
    }

    // 3. Display the results in a formatted table
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Rank", "Score", "Net Profit", "Drawdown %", "Calmar", "Profit Factor", "Trades", "Params",
        ]);

    for (i, ranked) in ranked_reports.iter().take(20).enumerate() { // Show top 20
        table.add_row(vec![
            Cell::new(i + 1),
            Cell::new(format!("{:.4}", ranked.score)),
            Cell::new(match ranked.report.total_net_profit {
                Some(profit) => format!("{:.2}", profit),
                None => "N/A".to_string(),
            }),
            Cell::new(match ranked.report.max_drawdown_pct {
                Some(drawdown) => format!("{:.2}%", drawdown),
                None => "N/A".to_string(),
            }),
            Cell::new(match ranked.report.calmar_ratio {
                Some(ratio) => format!("{:.2}", ratio),
                None => "N/A".to_string(),
            }),
            Cell::new(match ranked.report.profit_factor {
                Some(factor) => format!("{:.2}", factor),
                None => "N/A".to_string(),
            }),
            Cell::new(match ranked.report.total_trades {
                Some(trades) => trades.to_string(),
                None => "N/A".to_string(),
            }),
            Cell::new(ranked.report.parameters.to_string()),
        ]);
    }

    println!("{table}");
    Ok(())
}


// ... (handle_optimize, handle_single_run, handle_backfill, generate_monthly_ranges are unchanged)
async fn handle_optimize(args: OptimizeArgs, db_pool: sqlx::PgPool) -> Result<()> {
    println!("---===[ Starting Optimization Job ]===---");

    println!("Loading base configuration from config.toml...");
    let base_config = load_config(Some("config.toml"))?;
    
    println!("Loading optimization job from: {:?}", &args.config);
    let optimizer_config = load_optimizer_config(&args.config)?;

    let db_repo = DbRepository::new(db_pool);

    let optimizer = Optimizer::new(optimizer_config, base_config, db_repo);
    
    optimizer.run().await?;
    
    println!("\nOptimization process finished.");
    Ok(())
}

async fn handle_single_run(args: SingleRunArgs, db_pool: sqlx::PgPool) -> Result<()> {
    let config = load_config(Some("config.toml"))?;
    println!("Configuration loaded. Running default single backtest...");

    let backtest_config = config.backtest.clone();
    let start_date = args.from.unwrap_or(backtest_config.start_date);
    let end_date = args.to.unwrap_or(backtest_config.end_date);
    let symbol = backtest_config.symbol.clone();
    let interval = backtest_config.interval.clone();

    println!("Period: {} to {}", start_date, end_date);
    println!("Symbol: {}, Interval: {}", symbol, interval);

    let db_repo = DbRepository::new(db_pool);
    let analytics_engine = analytics::AnalyticsEngine::new();
    let portfolio = Portfolio::new(backtest_config.initial_capital);
    let executor = Box::new(SimulatedExecutor::new(config.simulation.clone()));
    let risk_manager = Box::new(SimpleRiskManager::new(config.risk_management.clone())?);
    
    let strategy = create_strategy(strategies::StrategyId::MACrossover, &config, "default_strategy")?;
    println!("Strategy: MACrossover");

    let mut backtester = Backtester::new(
        symbol,
        interval,
        portfolio,
        strategy,
        risk_manager,
        executor,
        analytics_engine,
        db_repo,
    );
    
    let report = backtester.run(
        start_date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Utc).unwrap(),
        end_date.and_hms_opt(23,59,59).unwrap().and_local_timezone(Utc).unwrap()
    ).await?;

    println!("\n---===[ Backtest Report ]===---");
    println!("{:#?}", report);

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
    from: NaiveDate,
    to: NaiveDate,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    use chrono::Datelike;
    
    let mut ranges = Vec::new();
    let mut current = from;
    
    while current <= to {
        // Get the last day of the current month
        let year = current.year();
        let month = current.month();
        
        // Calculate the first day of next month, then subtract 1 day to get end of current month
        let next_month = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
        };
        
        let end_of_month = next_month.pred_opt().unwrap();
        let end_date = std::cmp::min(end_of_month, to);
        
        // Add the range for this month
        ranges.push((
            current.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Utc).unwrap(),
            end_date.and_hms_opt(23, 59, 59).unwrap().and_local_timezone(Utc).unwrap(),
        ));
        
        // Move to the first day of the next month
        current = next_month;
    }
    
    ranges
}