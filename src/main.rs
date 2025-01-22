mod rdbg;

use env_logger::Env;
use log::{error, info};
use tokio::sync::watch;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Generate a random available port
    let port = rdbg::generate_random_port();

    // Spawn the `rdbg` process with the generated port
    let (tx, rx) = watch::channel(false);
    if let Err(e) = rdbg::spawn_rdbg(port, tx).await {
        error!("Failed to spawn rdbg: {}", e);
        std::process::exit(1);
    }

    // Wait for signal that rdbg is ready
    info!("Waiting for rdbg to be ready...");
    let mut rx = rx;
    while !*rx.borrow() {
        rx.changed().await.unwrap();
    }

    // Connect to the port using TCP
    match rdbg::connect_to_port(port) {
        Ok(_) => info!("Successfully connected to rdbg on port {}", port),
        Err(e) => error!("Failed to connect to rdbg on port {}: {}", port, e),
    }
}
