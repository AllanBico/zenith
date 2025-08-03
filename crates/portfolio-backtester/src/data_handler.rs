use crate::error::PortfolioError;
use chrono::{DateTime, Utc};
use configuration::PortfolioConfig;
use core_types::Kline;
use database::DbRepository;
use futures::future::join_all;
use std::collections::HashSet;

/// Represents a single market event in the master chronological stream.
/// For now, it only contains Kline data, but this enum structure allows for
/// future expansion (e.g., funding rate events, order book updates).
#[derive(Debug, Clone)]
pub enum Event {
    Kline(MarketEvent),
}

impl Event {
    /// Helper function to get the timestamp of any event type.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Event::Kline(k) => k.kline.open_time,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MarketEvent {
    pub symbol: String,
    pub kline: Kline,
}


/// Loads all necessary kline data for a portfolio and merges it into a single,
/// chronologically sorted event stream. This is the "Master Clock".
pub async fn load_and_prepare_data(
    portfolio_config: &PortfolioConfig,
    db_repo: &DbRepository,
    interval: &str, // The single interval for the entire portfolio backtest
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
) -> Result<Vec<Event>, PortfolioError> {
    // 1. Concurrently fetch kline data for all unique symbols.
    let unique_symbols: HashSet<_> = portfolio_config.bots.iter().map(|b| &b.symbol).collect();
    
    let fetch_futures = unique_symbols.into_iter().map(|symbol| {
        db_repo.get_klines_by_date_range(symbol, interval, start_date, end_date)
    });

    let results = join_all(fetch_futures).await;
    
    // 2. Collect and transform all klines into a single flat event vector.
    let mut all_events = Vec::new();
    for (i, result) in results.into_iter().enumerate() {
        let klines = result?; // Propagate any DB errors
        let symbol = portfolio_config.bots[i].symbol.clone(); // This is a simplification; a HashMap would be better for robustness
        for kline in klines {
            all_events.push(Event::Kline(MarketEvent {
                symbol: symbol.clone(),
                kline,
            }));
        }
    }

    // 3. Sort the master event stream chronologically. This is the critical step.
    all_events.sort_by_key(|event| event.timestamp());

    Ok(all_events)
}