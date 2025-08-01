use crate::error::ApiError;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use core_types::Kline;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;

pub mod error;

/// A generic trait for an API client that can fetch market data.
/// Using a trait is crucial for testing, as it allows us to create a mock client.
#[async_trait]
pub trait ApiClient: Send + Sync {
    async fn fetch_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<Kline>, ApiError>;
}

/// A client specifically for the Binance exchange API.
pub struct BinanceClient {
    client: reqwest::Client,
    base_url: String,
}

impl BinanceClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://api.binance.com".to_string(),
        }
    }
}

// This private struct is an intermediate representation that exactly matches
// the structure of the Binance API's kline response (an array of values).
// Serde can deserialize into this automatically.
#[derive(Deserialize)]
#[allow(dead_code)]  // We keep all fields for documentation and future use
struct RawKline(
    i64,    // 0: Open time (milliseconds)
    String, // 1: Open price
    String, // 2: High price
    String, // 3: Low price
    String, // 4: Close price
    String, // 5: Volume
    i64,    // 6: Close time (milliseconds)
    String, // 7: Quote asset volume
    i64,    // 8: Number of trades
    String, // 9: Taker buy base asset volume
    String, // 10: Taker buy quote asset volume
    String, // 11: Ignore
);

#[async_trait]
impl ApiClient for BinanceClient {
    /// Fetches historical klines from the Binance API.
    /// Handles the conversion from the raw API format to our internal `Kline` struct.
    async fn fetch_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<Kline>, ApiError> {
        let url = format!("{}/api/v3/klines", self.base_url);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("symbol", symbol),
                ("interval", interval),
                ("startTime", &start_time.timestamp_millis().to_string()),
                ("endTime", &end_time.timestamp_millis().to_string()),
                ("limit", "1000"),
            ])
            .send()
            .await?
            .json::<Vec<RawKline>>() // Deserialize into a Vec of our intermediate RawKline
            .await?;

        // Convert the Vec<RawKline> into a Vec<Kline>
        let klines = response
            .into_iter()
            .map(|raw| {
                Ok(Kline {
                    open_time: Utc.timestamp_millis_opt(raw.0).single().ok_or_else(|| {
                        ApiError::InvalidData(format!("Invalid open_time timestamp: {}", raw.0))
                    })?,
                    open: Decimal::from_str(&raw.1).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    high: Decimal::from_str(&raw.2).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    low: Decimal::from_str(&raw.3).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    close: Decimal::from_str(&raw.4).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    volume: Decimal::from_str(&raw.5).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    close_time: Utc.timestamp_millis_opt(raw.6).single().ok_or_else(|| {
                        ApiError::InvalidData(format!("Invalid close_time timestamp: {}", raw.6))
                    })?,
                    // The interval is not part of the API response, so we add it here from the request context.
                    interval: interval.to_string(),
                })
            })
            .collect::<Result<Vec<Kline>, ApiError>>()?;

        Ok(klines)
    }
}