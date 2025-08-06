use crate::error::ApiError;
use core_types::Kline;
use futures_util::stream::StreamExt;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing;
use url::Url;
use chrono::{TimeZone, Utc};
use serde::de::DeserializeOwned;
// --- Book Ticker Stream Deserialization ---

/// Represents a Book Ticker update from the `<symbol>@bookTicker` stream.
/// Provides the real-time best bid and ask prices and quantities.
#[derive(Debug, Clone, Deserialize)]
pub struct BookTickerUpdate {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "b")]
    pub best_bid_price: Decimal,
    #[serde(rename = "B")]
    pub best_bid_qty: Decimal,
    #[serde(rename = "a")]
    pub best_ask_price: Decimal,
    #[serde(rename = "A")]
    pub best_ask_qty: Decimal,
}

// --- Mark Price Stream Deserialization ---

/// Represents a Mark Price update from the `<symbol>@markPrice@1s` stream.
#[derive(Debug, Clone, Deserialize)]
pub struct MarkPriceUpdate {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub mark_price: Decimal,
    #[serde(rename = "r")]
    pub funding_rate: Decimal,
}
// --- WebSocket Deserialization Structs ---
#[derive(Debug, Deserialize)]
struct WsStreamWrapper<T> {
    stream: String,
    data: T,
}
#[derive(Debug, Deserialize)]
struct WsKlineEvent {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "k")]
    kline: WsKline,
}
#[derive(Debug, Deserialize)]
struct WsKline {
    #[serde(rename = "t")]
    open_time: i64,
    #[serde(rename = "T")]
    close_time: i64,
    #[serde(rename = "i")]
    interval: String,
    #[serde(rename = "o")]
    open: String,
    #[serde(rename = "c")]
    close: String,
    #[serde(rename = "h")]
    high: String,
    #[serde(rename = "l")]
    low: String,
    #[serde(rename = "v")]
    volume: String,
    #[serde(rename = "x")]
    is_closed: bool,
}

/// Handles connection to the Binance WebSocket API and manages data stream subscriptions.
pub struct LiveConnector {
    base_url: Url,
}

impl LiveConnector {
    pub fn new(live_mode: bool) -> Self {
        let base_url = if live_mode {
            "wss://fstream.binance.com"
        } else {
            "wss://stream.binancefuture.com"
        };
        Self {
            base_url: Url::parse(base_url).expect("Failed to parse WebSocket base URL"),
        }
    }
    pub fn subscribe_to_book_tickers(
        &self,
        symbols: &[String],
    ) -> Result<mpsc::Receiver<BookTickerUpdate>, ApiError> {
        let (tx, rx) = mpsc::channel(1024);
        let streams = symbols
            .iter()
            .map(|s| format!("{}@bookTicker", s.to_lowercase()))
            .collect::<Vec<_>>()
            .join("/");
        
        let mut url = self.base_url.clone();
        url.set_path("/stream");
        url.set_query(Some(&format!("streams={}", streams)));

        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = connect_async(url.clone()).await {
                    tracing::info!("[WS-BookTicker] Connection established.");
                    while let Some(msg) = stream.next().await {
                        if let Ok(Message::Text(text)) = msg {
                            if let Ok(wrapper) = serde_json::from_str::<WsStreamWrapper<BookTickerUpdate>>(&text) {
                                if tx.send(wrapper.data).await.is_err() { break; }
                            }
                        }
                    }
                }
                tracing::warn!("[WS-BookTicker] Disconnected. Reconnecting in 5s...");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        Ok(rx)
    }

    /// Subscribes to the Mark Price stream for a list of symbols.
    pub fn subscribe_to_mark_prices(
        &self,
        symbols: &[String],
    ) -> Result<mpsc::Receiver<MarkPriceUpdate>, ApiError> {
        let (tx, rx) = mpsc::channel(1024);
        let streams = symbols
            .iter()
            .map(|s| format!("{}@markPrice@1s", s.to_lowercase()))
            .collect::<Vec<_>>()
            .join("/");
        
        let mut url = self.base_url.clone();
        url.set_path("/stream");
        url.set_query(Some(&format!("streams={}", streams)));

        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = connect_async(url.clone()).await {
                    tracing::info!("[WS-MarkPrice] Connection established.");
                    while let Some(msg) = stream.next().await {
                        if let Ok(Message::Text(text)) = msg {
                            if let Ok(wrapper) = serde_json::from_str::<WsStreamWrapper<MarkPriceUpdate>>(&text) {
                                if tx.send(wrapper.data).await.is_err() { break; }
                            }
                        }
                    }
                }
                tracing::warn!("[WS-MarkPrice] Disconnected. Reconnecting in 5s...");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        Ok(rx)
    }

    /// Subscribes to kline streams and returns a channel Receiver for `(symbol, Kline)` data.
    pub fn subscribe_to_klines(
        &self,
        symbols: &[String],
        interval: &str,
    ) -> Result<mpsc::Receiver<(String, Kline)>, ApiError> {
        // 1. Create the MPSC channel for communication.
        let (tx, rx) = mpsc::channel(10000); // Increased capacity to prevent blocking

        // 2. Construct the full stream URL.
        let streams = symbols
            .iter()
            .map(|s| format!("{}@kline_{}", s.to_lowercase(), interval))
            .collect::<Vec<_>>()
            .join("/");
            
        let mut url = self.base_url.clone();
        url.set_path(&format!("/stream"));
        url.set_query(Some(&format!("streams={}", streams)));

        tracing::debug!("WebSocket URL: {}", url);

        // 3. Spawn a background task to manage the connection.
        tokio::spawn(async move {
            // 4. Implement the resilient reconnection loop.
            loop {
                tracing::info!("Connecting to WebSocket...");
                match connect_async(url.clone()).await {
                    Ok((mut stream, _)) => {
                        tracing::info!("WebSocket connection established.");
                        // 5. Enter the message processing loop.
                        while let Some(msg) = stream.next().await {
                            match msg {
                                Ok(Message::Text(text)) => {
                                    // We only care about klines that are closed.
                                    match serde_json::from_str::<WsStreamWrapper<WsKlineEvent>>(&text) {
                                        Ok(wrapper) => {
                                            if wrapper.data.event_type == "kline" {
                                                if wrapper.data.kline.is_closed {
                                                    tracing::debug!("Raw WebSocket message (CLOSED kline): {}", text);
                                                    tracing::debug!("Processing closed kline for symbol: {}", wrapper.data.symbol);
                                                    tracing::debug!("Raw kline data: {:?}", wrapper.data.kline);
                                                    
                                                    // Convert to our standard Kline type.
                                                    let k = wrapper.data.kline;
                                                    
                                                    // Debug the conversion process
                                                    tracing::debug!("Converting kline data:");
                                                    tracing::debug!("  Open time: {} -> {:?}", k.open_time, Utc.timestamp_millis_opt(k.open_time));
                                                    tracing::debug!("  Close time: {} -> {:?}", k.close_time, Utc.timestamp_millis_opt(k.close_time));
                                                    tracing::debug!("  Open: {} -> {:?}", k.open, Decimal::from_str(&k.open));
                                                    tracing::debug!("  High: {} -> {:?}", k.high, Decimal::from_str(&k.high));
                                                    tracing::debug!("  Low: {} -> {:?}", k.low, Decimal::from_str(&k.low));
                                                    tracing::debug!("  Close: {} -> {:?}", k.close, Decimal::from_str(&k.close));
                                                    tracing::debug!("  Volume: {} -> {:?}", k.volume, Decimal::from_str(&k.volume));
                                                    
                                                    let kline = Kline {
                                                        open_time: Utc.timestamp_millis_opt(k.open_time).single().unwrap(),
                                                        open: Decimal::from_str(&k.open).unwrap(),
                                                        high: Decimal::from_str(&k.high).unwrap(),
                                                        low: Decimal::from_str(&k.low).unwrap(),
                                                        close: Decimal::from_str(&k.close).unwrap(),
                                                        volume: Decimal::from_str(&k.volume).unwrap(),
                                                        close_time: Utc.timestamp_millis_opt(k.close_time).single().unwrap(),
                                                        interval: k.interval,
                                                    };

                                                    tracing::debug!("Converted kline: {:?}", kline);

                                                    // Send the symbol and kline to the engine. If it fails, the engine is gone, so we exit.
                                                    match tx.send((wrapper.data.symbol.clone(), kline)).await {
                                                        Ok(_) => {
                                                            tracing::debug!("Successfully sent kline for symbol: {}", wrapper.data.symbol);
                                                        }
                                                        Err(e) => {
                                                            tracing::error!("Failed to send kline for symbol {}: {:?}. Channel may be full or receiver dropped.", wrapper.data.symbol, e);
                                                            tracing::error!("Receiver dropped. Closing WebSocket connection.");
                                                            return;
                                                        }
                                                    }
                                                }
                                                // Skip non-closed klines silently
                                            }
                                            // Skip non-kline events silently
                                        }
                                        Err(e) => {
                                            tracing::warn!("Failed to parse WebSocket message as WsStreamWrapper: {}", e);
                                        }
                                    }
                                }
                                Ok(Message::Binary(data)) => {
                                    tracing::debug!("Received binary message of {} bytes", data.len());
                                }
                                Ok(Message::Ping(data)) => {
                                    tracing::debug!("Received ping with {} bytes", data.len());
                                }
                                Ok(Message::Pong(data)) => {
                                    tracing::debug!("Received pong with {} bytes", data.len());
                                }
                                Ok(Message::Close(frame)) => {
                                    tracing::info!("WebSocket connection closed: {:?}", frame);
                                    break;
                                }
                                Ok(Message::Frame(_)) => {
                                    tracing::debug!("Received raw frame");
                                }
                                Err(e) => {
                                    tracing::error!("WebSocket message error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "WebSocket connection error.");
                    }
                }
                tracing::warn!("WebSocket disconnected. Reconnecting in 5 seconds...");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        // 6. Return the receiver immediately.
        Ok(rx)
    }
}