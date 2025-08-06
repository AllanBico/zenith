use axum::{
    routing::get,
    Router,
};
use database::DbRepository;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use events::WsMessage;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer, // <-- Import the TraceLayer
};
// Add Mutex for the cache
use tokio::sync::Mutex;
use events::PortfolioState; // We need this type for the cache

// Note: Advanced tracing imports removed - using config-based tracing instead



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
    // Note: Tracing is already initialized in main.rs via config.toml
    // We don't need to initialize it again here to avoid conflicts

    // Create the cache for portfolio state
    let portfolio_state_cache = Arc::new(Mutex::new(None));

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
    
    // Create Shared State
    let app_state = Arc::new(AppState {
        db_repo,
        event_tx,
        portfolio_state_cache,
    });
    
    // Define CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Define the Application Routes
    let app = Router::new()
        .route("/api/health", get(|| async { "OK" }))
        .route("/api/optimization-jobs", get(handlers::get_optimization_jobs))
        .route("/api/single-runs", get(handlers::get_single_runs))
        .route("/api/wfo-jobs", get(handlers::get_wfo_jobs))
        .route("/api/optimization-jobs/:job_id", get(handlers::get_optimization_job_details))
        .route("/api/backtest-runs/:run_id", get(handlers::get_backtest_run_details))
        .route("/ws", get(handlers::websocket_handler))
        .with_state(app_state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Start the Server
    tracing::info!("Web server starting and listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}