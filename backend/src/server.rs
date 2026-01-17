use axum::{
    Router, routing::{get, post},
};
use http::HeaderName;
use reqwest::header::HeaderValue;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    set_header::SetResponseHeaderLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::info;
use crate::{
    api::{discovery::{get_chain_discovery, list_assets_on_dex}, handler::{AppState, commit_quote, create_quote, create_spending_approval, get_chain_treasury_balance, get_settlement_status, get_spending_approval_status, get_status, get_treasury_balances, health_check, list_user_approvals, near_webhook, payment_webhook, solana_webhook, stellar_webhook, submit_spending_approval, get_ohlc_chart_query}, streaming::stream_quotes, token_approval::{create_token_approval, submit_token_approval, get_token_approval_status}},
    routes::{
        charts::{get_chart_stats, get_latest_candle, get_ohlc_chart},
        quotes::{find_routes, get_best_quote, get_price_impact},
        trade::{execute_trade, get_trade_status, get_user_trades_by_chain, initiate_trade},
        wallet::{get_user_portfolio, get_wallet_balance, register_wallet, verify_wallet}
    }, trading::handlers::get_user_trades,
};



pub async fn create_app(state: AppState) -> Router {
    info!("⚙️ Setting up HTTP routes...");

    // Build the application router with all routes and middleware
    let app = Router::new()
        // Public health check endpoint
        .route("/health", get(health_check))
        
        // Chart endpoints at root level (convenience routes)
        .route("/chart/:asset/:chain/:timeframe", get(get_ohlc_chart))
        .route("/chart/:asset/:chain/:timeframe/latest", get(get_latest_candle))
        .route("/chart/stats", get(get_chart_stats))
        
        // Discovery endpoints at root level
        .route("/discovery/chain/:chain", get(get_chain_discovery))
        .route("/discovery/dex/:dex/:chain", get(list_assets_on_dex))
        
        // Quote engine OHLC endpoint (convenience route with query params)
        .route("/quote-engine/ohlc", get(get_ohlc_chart_query))
        
        // API v1 routes with security middleware
        .nest("/api/v1", 
            Router::new()
                // Quote endpoints
                .route("/quote", post(create_quote))
                .route("/commit", post(commit_quote))
                .route("/status/:id", get(get_status))
                
                // Webhook endpoints
                .route("/webhook/payment", post(payment_webhook))
                .route("/webhook/stellar", post(stellar_webhook))
                .route("/webhook/near", post(near_webhook))
                .route("/webhook/solana", post(solana_webhook))
                
                // Discovery endpoints
                .route("/dexes/:chain", get(get_chain_discovery))
                .route("/assets/:dex/:chain", get(list_assets_on_dex))
                
                // Realtime quote endpoints
                .route("/best-quote", post(get_best_quote))
                .route("/price-impact", post(get_price_impact))
                .route("/routes", post(find_routes))
                .route("/stream-quotes", get(stream_quotes))
                
                // Wallet management endpoints
                .route("/wallet/register", post(register_wallet))
                .route("/wallet/verify", post(verify_wallet))
                .route("/wallet/balance", get(get_wallet_balance))
                .route("/wallet/portfolio", get(get_user_portfolio))
                
                // Trading endpoints
                .route("/trade/initiate", post(initiate_trade))
                .route("/trade/execute", post(execute_trade))
                .route("/trade/status/:id", get(get_trade_status))
                .route("/trade/user/:user_id", get(get_user_trades))
                .route("/trade/user/:user_id/chain/:chain", get(get_user_trades_by_chain))
                
                // Chart/OHLC endpoints
                .route("/chart/:asset/:chain/:timeframe", get(get_ohlc_chart))
                .route("/chart/:asset/:chain/:timeframe/latest", get(get_latest_candle))
                .route("/chart/stats", get(get_chart_stats))
                
                // Spending approval endpoints
                .route("/spending-approval/create", post(create_spending_approval))
                .route("/spending-approval/:approval_id/submit", post(submit_spending_approval))
                .route("/spending-approval/:approval_id", get(get_spending_approval_status))
                .route("/spending-approval/user/:user_id", get(list_user_approvals))
                
                // Token approval endpoints (new cryptographic approval flow)
                .route("/approval/create", post(create_token_approval))
                .route("/approval/submit", post(submit_token_approval))
                .route("/approval/status/:approval_id", get(get_token_approval_status))
                
                // Settlement endpoints
                .route("/settlement/:quote_id", get(get_settlement_status))
                
                // Admin endpoints
                .route("/admin/treasury", get(get_treasury_balances))
                .route("/admin/treasury/:chain", get(get_chain_treasury_balance))
        )
        // Apply CORS layer - allow all origins in dev, restrict in prod
        .layer(CompressionLayer::new())
        // .layer(SetResponseHeaderLayer::if_not_present(
        //     HeaderName::from_static("x-frame-options"),
        //     HeaderValue::from_static("DENY"),
        // ))
        // .layer(SetResponseHeaderLayer::if_not_present(
        //     HeaderName::from_static("x-content-type-options"),
        //     HeaderValue::from_static("nosniff"),
        // ))
        // .layer(SetResponseHeaderLayer::if_not_present(
        //     HeaderName::from_static("x-xss-protection"),
        //     HeaderValue::from_static("1; mode=block"),
        // ))
        // .layer(SetResponseHeaderLayer::if_not_present(
        //     HeaderName::from_static("strict-transport-security"),
        //     HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        // ))
        .layer(CorsLayer::very_permissive())
        // Add request tracing
        .layer(TraceLayer::new_for_http())
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