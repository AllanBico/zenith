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

// --- WebSocket Deserialization Structs ---
#[derive(Debug, Deserialize)]
struct WsStreamWrapper {
    stream: String,
    data: WsKlineEvent,
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

    /// Subscribes to kline streams and returns a channel Receiver for `(symbol, Kline)` data.
    pub fn subscribe_to_klines(
        &self,
        symbols: &[String],
        interval: &str,
    ) -> Result<mpsc::Receiver<(String, Kline)>, ApiError> {
        // 1. Create the MPSC channel for communication.
        let (tx, rx) = mpsc::channel(1024);

        // 2. Construct the full stream URL.
        let streams = symbols
            .iter()
            .map(|s| format!("{}@kline_{}", s.to_lowercase(), interval))
            .collect::<Vec<_>>()
            .join("/");
            
        let mut url = self.base_url.clone();
        url.set_path(&format!("/stream"));
        url.set_query(Some(&format!("streams={}", streams)));

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
                            if let Ok(Message::Text(text)) = msg {
                                // We only care about klines that are closed.
                                if let Ok(wrapper) = serde_json::from_str::<WsStreamWrapper>(&text) {
                                    if wrapper.data.event_type == "kline" && wrapper.data.kline.is_closed {
                                        // Convert to our standard Kline type.
                                        let k = wrapper.data.kline;
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

                                        // Send the symbol and kline to the engine. If it fails, the engine is gone, so we exit.
                                        if tx.send((wrapper.data.symbol, kline)).await.is_err() {
                                            tracing::error!("Receiver dropped. Closing WebSocket connection.");
                                            return;
                                        }
                                    }
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