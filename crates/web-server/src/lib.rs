use axum::{
    extract::DefaultBodyLimit,
    routing::get,
    Router,
};
use tracing;
use database::DbRepository;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer, AllowOrigin, ExposeHeaders, AllowHeaders},
    trace::TraceLayer, // <-- Import the TraceLayer
};
// Note: Tracing is now handled by the main application configuration



pub mod error;
pub mod handlers; // <-- ADD THIS

/// The shared application state that all handlers can access.
#[derive(Clone)]
pub struct AppState {
    pub db_repo: DbRepository,
}




/// The main function to configure and run the web server.
pub async fn run_server(addr: SocketAddr) -> anyhow::Result<()> {
    // Note: Tracing is already initialized in main.rs, so we don't need to initialize it again here.
    // This prevents conflicts between different tracing subscribers.

    dotenvy::dotenv().ok();
    let db_pool = database::connect().await?;
    database::run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);

    let app_state = Arc::new(AppState { db_repo });
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
        // .route("/ws", get(handlers::websocket_handler)) // WebSocket handler will be added in the next task
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