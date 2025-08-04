use axum::{
    extract::DefaultBodyLimit,
    routing::get,
    Router,
};
use database::DbRepository;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};



pub mod error;
pub mod handlers; // <-- ADD THIS

/// The shared application state that all handlers can access.
#[derive(Clone)]
pub struct AppState {
    pub db_repo: DbRepository,
}



/// The main function to configure and run the web server.
pub async fn run_server(addr: SocketAddr) -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let db_pool = database::connect().await?;
    database::run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);

    let app_state = Arc::new(AppState { db_repo });
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // --- DEFINE THE APPLICATION ROUTES ---
    let app = Router::new()
        .route("/api/health", get(|| async { "OK" }))
        .route("/api/optimization-jobs", get(handlers::get_optimization_jobs))
        .route("/api/single-runs", get(handlers::get_single_runs))
        .route("/api/optimization-jobs/:job_id", get(handlers::get_optimization_job_details))
        .route("/api/backtest-runs/:run_id", get(handlers::get_backtest_run_details))
        // .route("/ws", get(handlers::websocket_handler)) // WebSocket handler will be added in the next task
        .with_state(app_state)
        .layer(cors)
        .layer(DefaultBodyLimit::max(1024 * 1024 * 50)); // Set a 50MB body limit

    println!(">> Web server listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}