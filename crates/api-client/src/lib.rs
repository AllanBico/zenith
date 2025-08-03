use crate::auth::sign_request;
use crate::error::ApiError;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use configuration::settings::ApiConfig;
use core_types::{Kline, OrderRequest};
use reqwest::header::{HeaderMap, HeaderValue};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::BTreeMap;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

mod auth;
pub mod error;
pub mod responses;
pub mod live_connector;
// --- Public API ---
pub use responses::{BalanceResponse, OrderResponse, PositionResponse, ApiErrorResponse};
pub use live_connector::LiveConnector;
/// The generic, abstract interface for a trading exchange API client.
/// This trait is the contract that the live engine will use, allowing the
/// underlying implementation (live or mock) to be swapped out.
#[async_trait]
pub trait ApiClient: Send + Sync {
    /// Fetches public historical kline data.
    async fn fetch_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<Kline>, ApiError>;

    /// Sets the leverage for a given symbol. (Authenticated)
    async fn set_leverage(&self, symbol: &str, leverage: u8) -> Result<(), ApiError>;

    /// Places a new order on the exchange. (Authenticated)
    async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse, ApiError>;

    /// Fetches the current account balance for all assets. (Authenticated)
    async fn get_account_balance(&self) -> Result<Vec<BalanceResponse>, ApiError>;

    /// Fetches all current open positions. (Authenticated)
    async fn get_open_positions(&self) -> Result<Vec<PositionResponse>, ApiError>;
}

/// A concrete implementation of the `ApiClient` for the Binance exchange.
#[derive(Clone)]
pub struct BinanceClient {
    client: reqwest::Client,
    base_url: String,

    api_secret: String,
}

impl BinanceClient {
    pub fn new(live_mode: bool, api_config: &ApiConfig) -> Self {
        let (base_url, keys) = if live_mode {
            ("https://fapi.binance.com".to_string(), &api_config.production)
        } else {
            (
                "https://testnet.binancefuture.com".to_string(),
                &api_config.testnet,
            )
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-MBX-APIKEY",
            HeaderValue::from_str(&keys.key).expect("Invalid API Key"),
        );

        Self {
            client: reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .expect("Failed to build reqwest client"),
            base_url,

            api_secret: keys.secret.clone(),
        }
    }

    async fn _get_signed<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &mut BTreeMap<&str, String>,
    ) -> Result<T, ApiError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        params.insert("timestamp", timestamp.to_string());

        let query_string = serde_qs::to_string(params).unwrap();
        let signature = sign_request(&self.api_secret, &query_string);

        let url = format!(
            "{}{}?{}&signature={}",
            self.base_url, path, query_string, signature
        );

        let response = self.client.get(&url).send().await?;
        let status = response.status();
        let text = response.text().await?;

        if status.is_success() {
            serde_json::from_str::<T>(&text).map_err(|e| ApiError::Deserialization(e.to_string()))
        } else {
            let api_error: ApiErrorResponse = serde_json::from_str(&text)
                .map_err(|e| ApiError::Deserialization(format!("Failed to deserialize error response: {}. Original text: {}", e, text)))?;
            Err(ApiError::BinanceError(api_error.code, api_error.msg))
        }
    }

    async fn _post_signed<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &mut BTreeMap<&str, String>,
    ) -> Result<T, ApiError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        params.insert("timestamp", timestamp.to_string());

        let query_string = serde_qs::to_string(params).unwrap();
        let signature = sign_request(&self.api_secret, &query_string);

        let url = format!(
            "{}{}?{}&signature={}",
            self.base_url, path, query_string, signature
        );
        
        let response = self.client.post(&url).send().await?;
        let status = response.status();
        let text = response.text().await?;

        if status.is_success() {
            serde_json::from_str::<T>(&text).map_err(|e| ApiError::Deserialization(e.to_string()))
        } else {
            let api_error: ApiErrorResponse = serde_json::from_str(&text)
                .map_err(|e| ApiError::Deserialization(format!("Failed to deserialize error response: {}. Original text: {}", e, text)))?;
            Err(ApiError::BinanceError(api_error.code, api_error.msg))
        }
    }
}

// Intermediate struct for deserializing klines from Binance API
#[derive(Deserialize)]
struct RawKline(i64, String, String, String, String, String, i64, String, i64, String, String, String);

#[async_trait]
impl ApiClient for BinanceClient {
    async fn fetch_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<Kline>, ApiError> {
        let url = format!("{}/fapi/v1/klines", self.base_url);

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
            .json::<Vec<RawKline>>()
            .await?;

        let klines = response
            .into_iter()
            .map(|raw| {
                Ok(Kline {
                    open_time: Utc.timestamp_millis_opt(raw.0).single().ok_or_else(|| ApiError::InvalidData(format!("Invalid open_time: {}", raw.0)))?,
                    open: Decimal::from_str(&raw.1).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    high: Decimal::from_str(&raw.2).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    low: Decimal::from_str(&raw.3).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    close: Decimal::from_str(&raw.4).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    volume: Decimal::from_str(&raw.5).map_err(|e| ApiError::Deserialization(e.to_string()))?,
                    close_time: Utc.timestamp_millis_opt(raw.6).single().ok_or_else(|| ApiError::InvalidData(format!("Invalid close_time: {}", raw.6)))?,
                    interval: interval.to_string(),
                })
            })
            .collect::<Result<Vec<Kline>, ApiError>>()?;

        Ok(klines)
    }

    async fn set_leverage(&self, symbol: &str, leverage: u8) -> Result<(), ApiError> {
        let mut params = BTreeMap::new();
        params.insert("symbol", symbol.to_string());
        params.insert("leverage", leverage.to_string());
        
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct LeverageResponse { 
            leverage: u8, 
            symbol: String 
        }
        self._post_signed::<LeverageResponse>("/fapi/v1/leverage", &mut params).await?;
        Ok(())
    }

    async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse, ApiError> {
        let mut params = BTreeMap::new();
        params.insert("symbol", order.symbol.clone());
        params.insert("side", format!("{:?}", order.side).to_uppercase());
        params.insert("type", format!("{:?}", order.order_type).to_uppercase());
        params.insert("quantity", order.quantity.to_string());
        params.insert("newClientOrderId", order.client_order_id.to_string());
        
        self._post_signed("/fapi/v1/order", &mut params).await
    }

    async fn get_account_balance(&self) -> Result<Vec<BalanceResponse>, ApiError> {
        let mut params = BTreeMap::new();
        self._get_signed("/fapi/v2/balance", &mut params).await
    }

    async fn get_open_positions(&self) -> Result<Vec<PositionResponse>, ApiError> {
        let mut params = BTreeMap::new();
        self._get_signed("/fapi/v2/positionRisk", &mut params).await
    }
}