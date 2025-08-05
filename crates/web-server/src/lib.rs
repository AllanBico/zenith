use axum::{
    extract::DefaultBodyLimit,
    routing::get,
    Router,
};
use tracing;
use database::DbRepository;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use events::WsMessage;
use tower_http::{
    cors::{Any, CorsLayer, AllowOrigin, ExposeHeaders, AllowHeaders},
    trace::TraceLayer, // <-- Import the TraceLayer
};
// Add Mutex for the cache
use tokio::sync::Mutex;
use events::PortfolioState; // We need this type for the cache
// Note: Tracing is now handled by the main application configuration



pub mod error;
pub mod handlers; // <-- ADD THIS

/// The shared application state that all handlers can access.
#[derive(Clone)]
pub struct AppState {
    pub db_repo: DbRepository,
    pub event_tx: broadcast::Sender<WsMessage>,
    /// Caches the most recent portfolio state for new clients.
    pub portfolio_state_cache: Arc<Mutex<Option<PortfolioState>>>,
}




/// The main function to configure and run the web server.
pub async fn run_server(addr: SocketAddr, db_repo: DbRepository, event_tx: broadcast::Sender<WsMessage>) -> anyhow::Result<()> {
    // Note: Tracing is already initialized in main.rs, so we don't need to initialize it again here.
    // This prevents conflicts between different tracing subscribers.
    
    // Create the cache
    let portfolio_state_cache = Arc::new(Mutex::new(None));

    // --- Spawn a task to keep the cache updated ---
    let mut rx = event_tx.subscribe();
    let cache_clone = Arc::clone(&portfolio_state_cache);
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let WsMessage::PortfolioState(state) = msg {
                let mut cache = cache_clone.lock().await;
                *cache = Some(state);
            }
        }
    });
    
    // Create Shared State, now including the cache
    let app_state = Arc::new(AppState {
        db_repo,
        event_tx,
        portfolio_state_cache,
    });
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods(Any)
        .allow_headers(AllowHeaders::any())
        .expose_headers(ExposeHeaders::any());

    // --- DEFINE THE APPLICATION ROUTES ---
    let app = Router::new()
        .route("/api/health", get(|| async { "OK" }))
        .route("/api/cors-test", get(|| async { "CORS is working!" }))
        .route("/api/optimization-jobs", get(handlers::get_optimization_jobs))
        .route("/api/single-runs", get(handlers::get_single_runs))
        .route("/api/optimization-jobs/:job_id", get(handlers::get_optimization_job_details))
        .route("/api/backtest-runs/:run_id", get(handlers::get_backtest_run_details))
        .route("/api/backtest-runs/:run_id/details", get(handlers::get_backtest_run_full_details))
        .route("/api/wfo-jobs", get(handlers::get_wfo_jobs))
        .route("/api/wfo-jobs/:wfo_job_id/runs", get(handlers::get_wfo_job_runs))
        .route("/ws", get(handlers::websocket_handler)) // WebSocket handler for real-time communication
        .with_state(app_state)
        .layer(cors)
        // --- ADD THE TRACE LAYER ---
        // This middleware will automatically log information about every incoming request.
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 50)); // Set a 50MB body limit

    tracing::info!("Web server listening on http://{}", addr);
    tracing::info!("Web server started and listening on {}", addr); // <-- Use tracing::info!

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}