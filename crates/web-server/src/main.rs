use tokio::sync::broadcast;
use std::net::SocketAddr;
use database::{connect, run_migrations, DbRepository};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // When run directly, it creates its own db connection and a broadcast channel.
    dotenvy::dotenv().ok();
    let db_pool = connect().await?;
    run_migrations(&db_pool).await?;
    let db_repo = DbRepository::new(db_pool);
    let (event_tx, _) = broadcast::channel(1024);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    web_server::run_server(addr, db_repo, event_tx).await
}