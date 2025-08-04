use std::net::SocketAddr;

// This main function is the entry point when running `cargo run -p web-server`.
// Its only job is to call the `run_server` function from the crate's library.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    web_server::run_server(addr).await
}