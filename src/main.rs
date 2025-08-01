use anyhow::Result;
use api_client::{ApiClient, BinanceClient};
use backtester::Backtester;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use clap::{Parser, Subcommand};
use configuration::load_config;
use database::{connect, run_migrations, DbRepository};
use executor::{Portfolio, SimulatedExecutor};
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};
use risk::SimpleRiskManager;
use sqlx::types::Decimal;
use strategies::create_strategy;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().expect(".env file not found");

    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Backfill(args) => handle_backfill(args, db_pool).await?,
        Commands::SingleRun(args) => handle_single_run(args, db_pool).await?,
    }

    Ok(())
}

// ... (CLI struct definitions)

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
    /// Run a single backtest using parameters from the config file.
    SingleRun(SingleRunArgs),
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
    /// Optional start date to override config default (YYYY-MM-DD).
    #[arg(long)]
    from: Option<NaiveDate>,
    /// Optional end date to override config default (YYYY-MM-DD).
    #[arg(long)]
    to: Option<NaiveDate>,
}


async fn handle_single_run(args: SingleRunArgs, db_pool: sqlx::PgPool) -> Result<()> {
    // 1. Load Config
    let config = load_config(None)?; // None means use default config path
    println!("Configuration loaded. Running default single backtest...");

    // 2. Set up parameters - using simulation config since there's no backtest config
    let start_date = args.from.unwrap_or_else(|| {
        // Default to 30 days ago if not specified
        let now = Utc::now().date_naive();
        now.checked_sub_days(chrono::Days::new(30)).unwrap_or(now)
    });
    
    let end_date = args.to.unwrap_or_else(|| {
        // Default to now if not specified
        Utc::now().date_naive()
    });
    
    // Use a default symbol and interval since they're not in the config
    let symbol = "BTCUSDT".to_string();
    let interval = "1h".to_string();

    println!("Period: {} to {}", start_date, end_date);
    println!("Symbol: {}, Interval: {}", symbol, interval);

    // 3. Instantiate all components
    let db_repo = DbRepository::new(db_pool);
    let analytics_engine = analytics::AnalyticsEngine::new();
    
    // Use a default initial capital since it's not in the config
    let initial_capital = Decimal::from(10000);
    let portfolio = Portfolio::new(initial_capital);
    
    // Create a default simulation config if needed
    let simulation_config = configuration::settings::Simulation {
        taker_fee_pct: Decimal::new(1, 3),  // 0.1% taker fee (1 / 10^3)
        slippage_pct: Decimal::new(5, 4),    // 0.05% slippage (5 / 10^4)
    };
    
    let executor = Box::new(SimulatedExecutor::new(simulation_config));
    
    // For now, we'll run a specific strategy. In the future, this could be configurable.
    let strategy = create_strategy(strategies::StrategyId::MACrossover, &config, &symbol)?;
    println!("Strategy: MACrossover");

    // 4. Create and run the backtester
    let mut backtester = Backtester::new(
        symbol,
        interval,
        portfolio,
        strategy,  // Don't box here, it's already a trait object
        Box::new(SimpleRiskManager::new(config.risk_management)?),
        executor,
        analytics_engine,
        db_repo,
    );
    
    let report = backtester.run(
        start_date.and_hms_opt(0,0,0).unwrap().and_local_timezone(Utc).unwrap(),
        end_date.and_hms_opt(23,59,59).unwrap().and_local_timezone(Utc).unwrap()
    ).await?;

    // 5. Print the report
    println!("\n---===[ Backtest Report ]===---");
    println!("{:#?}", report);

    Ok(())
}

// ... (handle_backfill and generate_monthly_ranges functions remain unchanged)
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
        // Calculate the first day of the next month
        let next_month = if from.month() == 12 {
            from.with_year(from.year() + 1)
                .and_then(|d| d.with_month(1))
        } else {
            from.with_month(from.month() + 1)
        };
        
        // Calculate the last day of the current month
        let end_of_month = next_month
            .and_then(|d| d.pred_opt())
            .unwrap_or_else(|| {
                // If we can't get the previous day, use the last day of the current year
                from.with_month(12)
                    .and_then(|d| d.with_day(31))
                    .unwrap_or(from) // Fallback to the original date if all else fails
            });
        
        // Use the minimum of end_of_month and the provided 'to' date
        let end_date = std::cmp::min(end_of_month, to);
        
        // Convert to DateTime<Utc> and add to ranges
        let start_dt = from.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_dt = end_date.and_hms_opt(23, 59, 59).unwrap().and_utc();
        
        ranges.push((start_dt, end_dt));
        
        // Move to the first day of the next month
        from = end_date.succ_opt().unwrap_or(end_date);
    }
    ranges
}