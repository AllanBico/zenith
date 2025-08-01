use api_client::{ApiClient, BinanceClient};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use clap::{Parser, Subcommand};
// Import database types directly from the database crate
use database::connection::{connect, run_migrations};
use database::repository::DbRepository;
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};

/// The main entry point for the Zenith trading application.
#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenvy::dotenv().expect(".env file not found");

    // Initialize the database connection and run migrations
    let db_pool = connect()
        .await
        .expect("Failed to connect to the database");
    run_migrations(&db_pool)
        .await
        .expect("Failed to run database migrations");

    // Parse command-line arguments
    let cli = Cli::parse();

    // Execute the appropriate command
    match cli.command {
        Commands::Backfill(args) => {
            if let Err(e) = handle_backfill(args, db_pool).await {
                eprintln!("Error during backfill: {}", e);
            }
        }
    }
}

// ==============================================================================
// CLI Structure (Task 4)
// ==============================================================================

/// A professional-grade, modular trading engine for crypto.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download historical kline data from an exchange.
    Backfill(BackfillArgs),
}

#[derive(Parser)]
struct BackfillArgs {
    /// The symbol to download data for (e.g., "BTCUSDT").
    #[arg(long)]
    symbol: String,

    /// The interval of the klines (e.g., "1h", "4h", "1d").
    #[arg(long)]
    interval: String,

    /// The start date for data download (format: YYYY-MM-DD).
    #[arg(long)]
    from: NaiveDate,

    /// The end date for data download (format: YYYY-MM-DD).
    #[arg(long)]
    to: NaiveDate,
}

// ==============================================================================
// Backfill Command Logic (Task 5)
// ==============================================================================

/// Handles the orchestration of the backfill process.
async fn handle_backfill(args: BackfillArgs, db_pool: sqlx::PgPool) -> anyhow::Result<()> {
    println!(
        "Starting backfill for {} on interval {} from {} to {}",
        args.symbol, args.from, args.interval, args.to
    );

    let db_repo = DbRepository::new(db_pool);
    let api_client = BinanceClient::new();

    // Generate monthly date ranges for fetching data to respect API limits
    let date_ranges = generate_monthly_ranges(args.from, args.to);
    
    // Set up the progress bar
    let progress_bar = ProgressBar::new(date_ranges.len() as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("#>-"),
    );

    // Create concurrent tasks for each date range
    let tasks: Vec<_> = date_ranges
        .into_iter()
        .map(|(start, end)| {
            let api_client_clone = BinanceClient::new(); // Create new client per task
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

    // Wait for all concurrent tasks to complete
    let results = join_all(tasks).await;

    progress_bar.finish_with_message("Backfill complete!");

    // Check for any errors that occurred in the tasks
    for result in results {
        if let Err(e) = result {
            eprintln!("A task failed: {}", e);
        }
    }

    Ok(())
}

/// Generates a vector of (start_date, end_date) tuples for each month between the from and to dates.
fn generate_monthly_ranges(
    mut from: NaiveDate,
    to: NaiveDate,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut ranges = Vec::new();
    
    while from <= to {
        // Calculate the end of the current month
        let year = from.year();
        let month = from.month();
        
        let (next_month_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        
        // Create the first day of next month, then subtract one day to get the last day of current month
        let end_of_month = NaiveDate::from_ymd_opt(next_month_year, next_month, 1)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(next_month_year + 1, 1, 1).unwrap())
            .pred_opt()
            .unwrap();
        
        let end_date = std::cmp::min(end_of_month, to);
        
        // Add the range from start of 'from' to end of 'end_date'
        ranges.push((
            from.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            end_date.and_hms_opt(23, 59, 59).unwrap().and_utc(),
        ));
        
        // Move to the next day after end_date
        from = end_date.succ_opt().unwrap_or(end_date);
    }
    
    ranges
}