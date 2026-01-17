mod error;
mod api;
mod adapters;
mod execution;
mod funding;
mod ledger;
mod quote_engine;
mod risk;
mod settlement;
mod wallet;
mod trading;
mod server;
mod bootstrap;
mod routes;
mod middleware;
mod config;



use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing::info;


// Initialize logging and tracing
fn init_tracing() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,tower_http=debug,backend=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    init_tracing();
    
    info!("🚀 Starting Symmetric Cross-Chain Inventory Backend");

    // Load configuration
    dotenv::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let bind_address =
        std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    

    let state = bootstrap::initialize_app_state(&database_url)
        .await?;

    // Create HTTP server
    let app = server::create_app(state).await;

    // Run the Server
    server::run_server(app, &bind_address).await?;


    info!("🌐 Server started successfully");

    Ok(())
}



