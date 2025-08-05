use crate::{error::AppError, AppState};
use analyzer::{Analyzer, RankedReport};
use database::repository::BacktestRunDetails;
use tracing;
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
/// # GET /ws (FIXED)
/// The WebSocket endpoint. Note the corrected argument order.
pub async fn websocket_handler(
    State(state): State<Arc<AppState>>, // State must come before WebSocketUpgrade
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, _state: Arc<AppState>) {
    tracing::info!("[WS] New client connected.");
    if socket.send(Message::Text("Welcome!".to_string())).await.is_err() {
        return;
    }
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(t)) => {
                if socket.send(Message::Text(format!("You sent: {}", t))).await.is_err() { break; }
            }
            Ok(Message::Close(_)) => {
                tracing::info!("[WS] Client disconnected.");
                break;
            }
            Err(e) => {
                tracing::error!(error = %e, "[WS] Error.");
                break;
            }
            _ => {}
        }
    }
    tracing::info!("[WS] Connection closed.");
}