use crate::{error::AppError, AppState};
use analyzer::{Analyzer, RankedReport};
use database::repository::BacktestRunDetails;
use tracing;
use chrono::Utc;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path,
        Query,
        State,
    },
    response::IntoResponse,
    Json,
};
use configuration::load_optimizer_config;
use database::{DbOptimizationJob, FullReport, WfoJob, WfoRun};
use futures_util::StreamExt;

use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct RunDetailsPath {
    pub run_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct Pagination {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_page() -> usize { 1 }
fn default_limit() -> usize { 20 }

/// # GET /api/optimization-jobs
pub async fn get_optimization_jobs(
    State(state): State<Arc<AppState>>,
    _pagination: Query<Pagination>,
) -> Result<Json<Vec<DbOptimizationJob>>, AppError> {
    let jobs = state.db_repo.get_all_optimization_jobs().await?;
    Ok(Json(jobs))
}

/// # GET /api/single-runs (NEW ENDPOINT)
/// Fetches a list of all completed single backtest runs.
pub async fn get_single_runs(
    State(state): State<Arc<AppState>>,
    _pagination: Query<Pagination>,
) -> Result<Json<Vec<FullReport>>, AppError> {
    let runs = state.db_repo.get_all_single_runs().await?;
    Ok(Json(runs))
}

/// # GET /api/optimization-jobs/:job_id
pub async fn get_optimization_job_details(
    Path(job_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<RankedReport>>, AppError> {
    let optimizer_config = load_optimizer_config(&PathBuf::from("optimizer.toml"))?;
    let analyzer = Analyzer::new(optimizer_config.analysis);
    let ranked_reports = analyzer.run(&state.db_repo, job_id).await?;
    Ok(Json(ranked_reports))
}

/// # GET /api/backtest-runs/:run_id
pub async fn get_backtest_run_details(
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<FullReport>, AppError> {
    let report = state.db_repo.get_full_report_for_run(run_id).await?;
    Ok(Json(report))
}

/// # GET /api/backtest-runs/:run_id/details
/// Fetches the full details for a single backtest run, including trades and equity curve.
pub async fn get_backtest_run_full_details(
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<BacktestRunDetails>, AppError> {
    let details = state.db_repo.get_run_details(run_id).await?;
    Ok(Json(details))
}

/// # GET /api/wfo-jobs
/// Fetches all WFO jobs.
pub async fn get_wfo_jobs(
    State(state): State<Arc<AppState>>,
    _pagination: Query<Pagination>,
) -> Result<Json<Vec<WfoJob>>, AppError> {
    let jobs = state.db_repo.get_all_wfo_jobs().await?;
    Ok(Json(jobs))
}

/// # GET /api/wfo-jobs/:wfo_job_id/runs
/// Fetches all runs for a specific WFO job.
pub async fn get_wfo_job_runs(
    Path(wfo_job_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<WfoRun>>, AppError> {
    let runs = state.db_repo.get_wfo_runs_for_job(wfo_job_id).await?;
    Ok(Json(runs))
}
/// # GET /ws
/// The WebSocket endpoint for real-time communication.
pub async fn websocket_handler(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// The actual logic for handling a single WebSocket connection.
async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    tracing::info!("[WS] New client connected.");

    // 1. Subscribe this client to the broadcast channel.
    let mut event_rx = state.event_tx.subscribe();

    // 2. Send a test message to confirm connection
    let test_msg = events::WsMessage::Connected;
    let test_payload = serde_json::to_string(&test_msg).unwrap();
    if socket.send(Message::Text(test_payload)).await.is_err() {
        tracing::warn!("[WS] Failed to send test message to new client.");
        return; // Client disconnected immediately
    }
    tracing::info!("[WS] Sent test message to new client");

    // 3. Implement the "Replay Cache" - send the last known state immediately.
    let initial_state = { // Scoped lock
        state.portfolio_state_cache.lock().await.clone()
    };
    if let Some(portfolio_state) = initial_state {
        let msg = events::WsMessage::PortfolioState(portfolio_state);
        let payload = serde_json::to_string(&msg).unwrap();
        if socket.send(Message::Text(payload)).await.is_err() {
            tracing::warn!("[WS] Failed to send initial state to new client.");
            return; // Client disconnected immediately
        }
    }

    // 3. The main concurrent loop.
    // This loop listens for messages from both the client and the broadcast channel.
    let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    loop {
        tokio::select! {
            // Send heartbeat every 30 seconds
            _ = heartbeat_interval.tick() => {
                let heartbeat_msg = events::WsMessage::Log(events::LogMessage {
                    timestamp: chrono::Utc::now(),
                    level: events::LogLevel::Info,
                    message: "WebSocket heartbeat".to_string(),
                });
                let payload = serde_json::to_string(&heartbeat_msg).unwrap();
                if socket.send(Message::Text(payload)).await.is_err() {
                    tracing::error!("[WS] Failed to send heartbeat. Client may have disconnected.");
                    break;
                }
                tracing::debug!("[WS] Sent heartbeat to client");
            }
            // A message was received from the broadcast channel (i.e., from the LiveEngine).
            msg = event_rx.recv() => {
                match msg {
                    Ok(msg) => {
                        tracing::info!("[WS] Received message from broadcast channel: {:?}", msg);
                        let payload = serde_json::to_string(&msg).unwrap();
                        tracing::debug!("[WS] Sending payload to client: {}", payload);
                        match socket.send(Message::Text(payload)).await {
                            Ok(_) => {
                                tracing::debug!("[WS] Successfully sent message to client");
                            }
                            Err(e) => {
                                tracing::error!("[WS] Failed to send message to client: {:?}", e);
                                tracing::info!("[WS] Client disconnected. Breaking send loop.");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("[WS] Broadcast channel error: {:?}", e);
                        tracing::info!("[WS] Breaking send loop due to broadcast channel error.");
                        break;
                    }
                }
            }

            // A message was received from the client (e.g., ping, close).
            Some(Ok(msg)) = socket.next() => {
                match msg {
                    Message::Close(_) => {
                        tracing::info!("[WS] Client sent close frame.");
                        break;
                    }
                    Message::Ping(_) => {
                        // The client is checking if we're alive.
                        // `axum` handles sending the `Pong` frame automatically.
                    }
                    _ => {
                        // We don't process other messages from the client.
                    }
                }
            }
            
            // If either the broadcast channel lags or the client disconnects, we exit.
            else => {
                break;
            }
        }
    }

    tracing::info!("[WS] Connection closed.");
}