# Code Architecture & Refactoring Guide

## Current Structure Problem

The `main.rs` file has grown too large with multiple responsibilities:
1. Configuration loading
2. Database initialization
3. Component creation and wiring
4. HTTP server setup
5. Route registration
6. Middleware configuration

**Current main.rs**: ~310 lines

**Solution**: Partition into focused modules

---

## Proposed New Structure

```
src/
├── main.rs                  # Entry point (50 lines)
├── server.rs               # Server & route setup (150 lines)
├── config.rs               # Configuration (refactored)
├── bootstrap.rs            # Initialization (120 lines)
├── middleware/
│   ├── mod.rs
│   ├── error_handler.rs
│   ├── request_logger.rs
│   └── rate_limiter.rs
├── routes/
│   ├── mod.rs
│   ├── quotes.rs           # Quote endpoints
│   ├── charts.rs           # OHLC endpoints
│   ├── webhooks.rs         # Webhook endpoints
│   ├── admin.rs            # Treasury & admin endpoints
│   └── health.rs           # Health check
└── [existing modules...]
```

---

## Step 1: Create bootstrap.rs

This file handles all component initialization:

```rust
// src/bootstrap.rs
use std::sync::Arc;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tracing::info;
use crate::error::AppResult;
use crate::ledger::repository::LedgerRepository;
use crate::quote_engine::{QuoteEngine, PriceCache, OhlcStore};
use crate::execution::router::ExecutionRouter;
use crate::risk::controls::RiskController;
use crate::api::handler::AppState;

pub async fn initialize_app_state(database_url: &str) -> AppResult<AppState> {
    info!("🚀 Initializing application components...");

    // Database pool
    let pool = initialize_database(database_url).await?;
    
    // Core components
    let ledger = Arc::new(LedgerRepository::new(pool.clone()));
    let quote_engine = Arc::new(QuoteEngine::new(ledger.clone()));
    let execution_router = Arc::new(ExecutionRouter::new());
    let risk_controller = Arc::new(RiskController::new());
    
    // Week 2 components
    let price_cache = Arc::new(PriceCache::new(1000));
    let ohlc_store = Arc::new(OhlcStore::new(100));
    
    // Additional components
    let adapter_registry = Arc::new(crate::adapters::AdapterRegistry::new());
    let realtime_quote_engine = Arc::new(crate::quote_engine::RealtimeQuoteEngine::new());
    let wallet_repository = Arc::new(crate::wallet::WalletRepository::new());
    let trade_repository = Arc::new(crate::trading::TradeRepository::new());

    Ok(AppState {
        ledger,
        quote_engine,
        execution_router,
        risk_controller,
        adapter_registry,
        realtime_quote_engine,
        wallet_repository,
        trade_repository,
        ohlc_store,
        price_cache,
    })
}

async fn initialize_database(database_url: &str) -> AppResult<PgPool> {
    info!("📊 Connecting to database...");
    
    let pool = PgPoolOptions::new()
        .max_connections(200)
        .min_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(600))
        .max_lifetime(std::time::Duration::from_secs(1800))
        .connect(database_url)
        .await?;

    info!("✓ Database pool configured: 200 max connections");
    
    // Run migrations
    info!("🔄 Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    
    info!("✓ Database initialized");
    Ok(pool)
}
```

---

## Step 2: Create server.rs

This file sets up the HTTP server and routes:

```rust
// src/server.rs
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::info;
use crate::api::handler::AppState;
use crate::routes;

pub async fn create_app(state: AppState) -> Router {
    info!("⚙️ Setting up HTTP routes...");

    let app = Router::new()
        // Quote routes
        .route("/quote", post(routes::quotes::create_quote))
        .route("/commit", post(routes::quotes::commit_quote))
        
        // Status routes
        .route("/status/:quote_id", get(routes::health::get_status))
        
        // Chart routes
        .route("/chart/:asset/:chain/:timeframe", get(routes::charts::get_ohlc))
        .route("/chart/:asset/:chain/:timeframe/latest", get(routes::charts::get_latest))
        .route("/chart/stats", get(routes::charts::get_stats))
        
        // Webhook routes
        .route("/webhook/payment", post(routes::webhooks::payment_webhook))
        .route("/webhook/stellar", post(routes::webhooks::stellar_webhook))
        .route("/webhook/near", post(routes::webhooks::near_webhook))
        .route("/webhook/solana", post(routes::webhooks::solana_webhook))
        
        // Admin routes
        .route("/admin/treasury", get(routes::admin::get_treasury))
        .route("/admin/treasury/:chain", get(routes::admin::get_chain_treasury))
        
        // Health route
        .route("/health", get(routes::health::health_check))
        
        .layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive())
                .layer(tower_http::trace::TraceLayer::new_for_http())
        )
        .with_state(state);

    info!("✓ HTTP routes configured");
    app
}

pub async fn run_server(
    app: Router,
    bind_address: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(bind_address).await?;
    info!("🌐 Server listening on: {}", bind_address);
    
    axum::serve(listener, app).await?;
    Ok(())
}
```

---

## Step 3: Refactored main.rs

Clean, minimal entry point:

```rust
// src/main.rs
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
mod middleware;
mod routes;
mod bootstrap;
mod server;

use tracing_subscriber;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load configuration
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/backend".to_string());
    let bind_address = env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    // Initialize application state
    let state = bootstrap::initialize_app_state(&database_url).await?;

    // Create HTTP server
    let app = server::create_app(state).await;

    // Run server
    server::run_server(app, &bind_address).await?;

    Ok(())
}
```

---

## Step 4: Create route modules

### src/routes/mod.rs

```rust
pub mod quotes;
pub mod charts;
pub mod webhooks;
pub mod admin;
pub mod health;
```

### src/routes/quotes.rs

```rust
use axum::{extract::State, Json};
use crate::api::handler::{AppState, create_quote, commit_quote};

// Re-export handlers
pub use crate::api::handler::create_quote;
pub use crate::api::handler::commit_quote;
```

### src/routes/charts.rs

```rust
use axum::{extract::State, Json};
use crate::api::handler::{AppState, get_ohlc_chart, get_latest_candle, get_chart_stats};

pub use crate::api::handler::get_ohlc_chart;
pub use crate::api::handler::get_latest_candle;
pub use crate::api::handler::get_chart_stats;
```

Similar for `webhooks.rs`, `admin.rs`, `health.rs`.

---

## Step 5: Middleware Layer

### src/middleware/mod.rs

```rust
pub mod error_handler;
pub mod request_logger;
pub mod rate_limiter;
```

### src/middleware/error_handler.rs

```rust
use axum::response::{IntoResponse, Response};
use crate::error::AppError;

pub struct ErrorHandler;

impl ErrorHandler {
    pub fn handle_error(err: AppError) -> Response {
        // Convert AppError to HTTP response
        err.into_response()
    }
}
```

---

## File Size Comparison

| File | Before | After | Reduction |
|------|--------|-------|-----------|
| main.rs | 310 lines | 50 lines | **84%** |
| server.rs | N/A | 70 lines | **New** |
| bootstrap.rs | N/A | 110 lines | **New** |
| routes/*.rs | N/A | ~200 lines | **New** |
| middleware/*.rs | N/A | ~150 lines | **New** |

**Total**: 310 lines → ~680 lines (clearer organization, better maintainability)

---

## Benefits

✅ **Separation of Concerns**: Each module has single responsibility
✅ **Testability**: Easier to unit test individual components
✅ **Maintainability**: Finding code is faster
✅ **Scalability**: Easy to add new routes/middleware
✅ **Readability**: main.rs becomes self-documenting
✅ **Reusability**: bootstrap.rs can be used in tests, CLI tools

---

## Migration Steps

1. Create `src/bootstrap.rs` with component initialization
2. Create `src/server.rs` with HTTP setup
3. Create `src/routes/mod.rs` and submodules
4. Create `src/middleware/mod.rs` and submodules
5. Move route handlers into appropriate files
6. Update `src/api/mod.rs` to export routes
7. Replace `main.rs` with minimal version
8. Run `cargo check` and fix imports
9. Run tests to verify functionality

---

## Example: Adding a New Route

Once refactored, adding a new route is simple:

```rust
// 1. Create handler in src/routes/new_feature.rs
pub async fn my_handler(
    State(state): State<AppState>,
    Json(req): Json<MyRequest>,
) -> AppResult<Json<MyResponse>> {
    // Implementation
}

// 2. Add to src/routes/mod.rs
pub mod new_feature;

// 3. Register in src/server.rs
.route("/my-route", post(routes::new_feature::my_handler))
```

Done! No main.rs modifications needed.

---

**Recommendation**: Implement this refactoring before Week 3 deployment for better code health.
