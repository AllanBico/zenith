use anyhow::Result;
use api_client::{ApiClient, BinanceClient};
use backtester::Backtester;
use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc};
use clap::{Parser, Subcommand};
use configuration::{load_config, load_optimizer_config};
use database::{connect, run_migrations, DbRepository};
use executor::{Portfolio, SimulatedExecutor};
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use optimizer::Optimizer;
use risk::SimpleRiskManager;
use strategies::create_strategy;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().expect(".env file not found");

    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Backfill(args) => handle_backfill(args, db_pool).await?,
        Commands::SingleRun(args) => handle_single_run(args, db_pool).await?,
        Commands::Optimize(args) => handle_optimize(args, db_pool).await?, // New command wiring
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
    /// Download historical kline data.
    Backfill(BackfillArgs),
    /// Run a single backtest using parameters from config.toml.
    SingleRun(SingleRunArgs),
    /// Run a full optimization job from a config file.
    Optimize(OptimizeArgs),
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
    /// Path to the optimizer configuration file.
    #[arg(long, short, default_value = "optimizer.toml")]
    config: PathBuf,
}

// ==============================================================================
// Command Handlers
// ==============================================================================

async fn handle_optimize(args: OptimizeArgs, db_pool: sqlx::PgPool) -> Result<()> {
    println!("---===[ Starting Optimization Job ]===---");

    // 1. Load both configurations
    println!("Loading base configuration from config.toml...");
    let base_config = load_config(Some("config.toml"))?;
    
    println!("Loading optimization job from: {:?}", &args.config);
    let optimizer_config = load_optimizer_config(&args.config)?;

    // 2. Instantiate dependencies
    let db_repo = DbRepository::new(db_pool);

    // 3. Create and run the Optimizer
    let optimizer = Optimizer::new(optimizer_config, base_config, db_repo);
    
    optimizer.run().await?;
    
    println!("\nOptimization process finished.");
    Ok(())
}


async fn handle_single_run(args: SingleRunArgs, db_pool: sqlx::PgPool) -> Result<()> {
    let config = load_config(Some("config.toml"))?;
    println!("Configuration loaded. Running default single backtest...");

    // Use command line arguments or default values for dates
    let start_date = args.from.unwrap_or_else(|| {
        // Default to one month ago if no start date provided
        let now = chrono::Utc::now().naive_utc().date();
        now - chrono::TimeDelta::days(30)
    });
    
    let end_date = args.to.unwrap_or_else(|| {
        // Default to now if no end date provided
        chrono::Utc::now().naive_utc().date()
    });
    
    // Use default values for symbol and interval
    let symbol = "BTCUSDT".to_string();
    let interval = "15m".to_string();

    println!("Period: {} to {}", start_date, end_date);
    println!("Symbol: {}, Interval: {}", symbol, interval);

    let db_repo = DbRepository::new(db_pool);
    let analytics_engine = analytics::AnalyticsEngine::new();
    
    // Use a default initial capital since it's not in the config
    let initial_capital = rust_decimal_macros::dec!(1000.0);
    let portfolio = Portfolio::new(initial_capital);
    
    let executor = Box::new(SimulatedExecutor::new(config.simulation.clone()));
    let risk_manager = Box::new(SimpleRiskManager::new(config.risk_management.clone())?);
    
    // Create the strategy using the correct symbol from our local variable
    let strategy = create_strategy(
        strategies::StrategyId::MACrossover, 
        &config,
        &symbol
    )?;
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
        // Get the first day of the current month
        let first_day = from.with_day(1).unwrap();
        
        // Calculate the first day of the next month
        let next_month = if first_day.month() == 12 {
            NaiveDate::from_ymd_opt(first_day.year() + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(first_day.year(), first_day.month() + 1, 1).unwrap()
        };
        
        // The last day of the current month is the day before the first day of the next month
        let end_of_month = next_month.pred_opt().unwrap();
        
        // Use the provided 'to' date if it's earlier than the end of the current month
        let end_date = std::cmp::min(end_of_month, to);
        
        // Add the range to the result
        ranges.push((
            from.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Utc).unwrap(),
            end_date.and_hms_opt(23, 59, 59).unwrap().and_local_timezone(Utc).unwrap(),
        ));
        
        from = end_date + chrono::Duration::days(1);
    }
    ranges
}