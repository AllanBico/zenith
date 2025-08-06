use crate::error::AlerterError;
use configuration::TelegramConfig;
use reqwest::Client;
use serde::Serialize;
use events::{LogLevel, WsMessage};
use tokio::sync::broadcast;
pub mod error;

/// The JSON payload for the Telegram `sendMessage` endpoint.
#[derive(Debug, Serialize)]
struct SendMessagePayload<'a> {
    chat_id: &'a str,
    text: &'a str,
    parse_mode: &'a str, // To allow for formatting like bold, italics etc.
}

/// A client for sending messages to the Telegram Bot API.
pub struct TelegramAlerter {
    client: Client,
    token: String,
    chat_id: String,
}

impl TelegramAlerter {
    /// Creates a new `TelegramAlerter`.
    ///
    /// Returns `None` if the token or chat_id is missing from the configuration,
    /// allowing the system to gracefully disable alerting.
    pub fn new(config: &TelegramConfig) -> Option<Self> {
        if config.token.is_empty() || config.chat_id.is_empty() {
            tracing::warn!("Telegram alerter is not configured (missing token or chat_id).");
            return None;
        }
        Some(Self {
            client: Client::new(),
            token: config.token.clone(),
            chat_id: config.chat_id.clone(),
        })
    }

    /// Sends a text message to the configured Telegram chat.
    pub async fn send_message(&self, message: &str) -> Result<(), AlerterError> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);

        let payload = SendMessagePayload {
            chat_id: &self.chat_id,
            text: message,
            parse_mode: "MarkdownV2", // Use Markdown for rich formatting
        };

        let response = self.client.post(&url).json(&payload).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Failed to decode error response".to_string());
            return Err(AlerterError::ApiError(error_text));
        }

        Ok(())
    }
}
/// A long-running service that listens to a broadcast channel of `WsMessage` events
/// and sends Telegram alerts for critical events.
pub async fn run_alerter_service(
    alerter: TelegramAlerter,
    mut event_rx: broadcast::Receiver<WsMessage>,
) {
    tracing::info!("Alerter service started. Listening for critical events.");

    // Send a startup message
    let _ = alerter.send_message("✅ *Zenith Engine Started*").await;

    loop {
        match event_rx.recv().await {
            Ok(event) => {
                // We match on the event type to decide if an alert is needed.
                let message_to_send = match event {
                    WsMessage::Log(log) => {
                        // We only care about high-severity logs
                        match log.level {
                            LogLevel::Error | LogLevel::Warn => {
                                // Extract the most important part of the message
                                let title = if log.message.contains("CRITICAL") { "🚨 CRITICAL" } else { "⚠️ ERROR" };
                                Some(format!("*{}*: {}", title, escape_markdown(&log.message)))
                            }
                            _ => None, // Ignore Info logs
                        }
                    }
                    WsMessage::TradeExecuted(exec) => {
                        let side = format!("{:?}", exec.side).to_uppercase();
                        let icon = if side == "BUY" { "📈" } else { "📉" };
                        Some(format!(
                            "{} *{} {}* `@{}`\n`{:.4}` units",
                            icon, side, escape_markdown(&exec.symbol), exec.price, exec.quantity
                        ))
                    }
                    _ => None, // Ignore PortfolioState, Connected, etc.
                };

                if let Some(msg) = message_to_send {
                    if let Err(e) = alerter.send_message(&msg).await {
                        tracing::error!(error = ?e, "Failed to send Telegram alert.");
                    }
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("Alerter service lagged, skipped {} messages.", n);
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::error!("Broadcast channel closed. Alerter service shutting down.");
                break;
            }
        }
    }
}

/// A helper function to escape characters that have special meaning in Telegram's MarkdownV2.
fn escape_markdown(text: &str) -> String {
    let special_chars = r"_*[]()~`>#+-=|{}.!";
    special_chars.chars().fold(text.to_string(), |s, c| s.replace(c, &format!("\\{}", c)))
}