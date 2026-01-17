use std::{sync::Arc, time::Duration};
use solana_sdk::signature::Keypair;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::{info, error};
use crate::{
    adapters::{AdapterRegistry, dex::{NearDexAdapter, PhantomSwapAdapter, RaydiumAdapter}}, api::handler::AppState, error::AppResult, execution::{near::{NearConfig, NearExecutor}, router::ExecutionRouter, solana::{SolanaConfig, SolanaExecutor}, stellar::{StellarConfig, StellarExecutor}}, ledger::{models::Chain, repository::LedgerRepository}, quote_engine::{OhlcStore, PriceCache, PythOracle, QuoteEngine, engine::QuoteConfig, realtime::RealtimeQuoteEngine}, risk::controls::{RiskConfig, RiskController}, trading::TradeRepository, wallet::WalletRepository
};

pub async fn initialize_app_state(database_url: &str) -> AppResult<AppState> {
    info!("Initializing application components ...");

    // Database pool 
    let pool = initialize_database(database_url).await?;

    // Core components
    let ledger = Arc::new(LedgerRepository::new(pool.clone()));

    // Initialize Pyth Price Oracle
    let network = std::env::var("NETWORK").unwrap_or_else(|_| "testnet".to_string());
    let pyth_oracle = Arc::new(PythOracle::new(&network));
    info!("✅ Pyth price oracle initialized for network: {}", network);

    // Initialize quote engine
    let quote_config = QuoteConfig::default();
    let quote_engine = Arc::new(QuoteEngine::new(
        quote_config,
        ledger.clone(),
        pyth_oracle.clone(),
        network.clone(),
    ));

    let execution_route = Arc::new(ExecutionRouter::new());
    
    // Initialize risk controls
    let risk_config = RiskConfig::default();
    let risk_controller = Arc::new(RiskController::new(risk_config, ledger.clone()));

    // Price components
    let price_cache = Arc::new(PriceCache::new(1000));
    let ohlc_store = Arc::new(OhlcStore::new(100));

    // Initialize execution router first so we can pass executors to adapters
    let mut execution_router = ExecutionRouter::new();

    info!("⚙️  Initializing chain executors...");

    // Initialize Solana executor
    let solana_executor = if let Ok(solana_key) = std::env::var("SOLANA_TREASURY_KEY") {
        match Keypair::from_base58_string(&solana_key) {
            keypair => {
                let solana_config = SolanaConfig::default();
                let executor = Arc::new(SolanaExecutor::new(
                    solana_config,
                    ledger.clone(),
                    risk_controller.clone(),
                    keypair,
                ));
                execution_router.register_executor(Chain::Solana, executor.clone());
                info!("✅ Solana executor registered");
                Some(executor)
            }
        }
    } else {
        error!("⚠️  SOLANA_TREASURY_KEY not set - Solana execution disabled");
        None
    };

    // Initialize Stellar executor
    let stellar_executor = if let Ok(stellar_key) = std::env::var("STELLAR_TREASURY_KEY") {
        let stellar_config = StellarConfig::default();
        let executor = Arc::new(StellarExecutor::new(
            stellar_config,
            ledger.clone(),
            risk_controller.clone(),
            stellar_key,
        ));
        execution_router.register_executor(Chain::Stellar, executor.clone());
        info!("✅ Stellar executor registered");
        Some(executor)
    } else {
        error!("⚠️  STELLAR_TREASURY_KEY not set - Stellar execution disabled");
        None
    };

    // Initialize Near executor
    let near_executor = if let Ok(near_key) = std::env::var("NEAR_TREASURY_KEY") {
        let near_config = NearConfig::default();
        let executor = Arc::new(NearExecutor::new(
            near_config,
            ledger.clone(),
            risk_controller.clone(),
            near_key,
        ));
        execution_router.register_executor(Chain::Near, executor.clone());
        info!("✅ Near executor registered");
        Some(executor)
    } else {
        error!("⚠️  NEAR_TREASURY_KEY not set - Near execution disabled");
        None
    };

    let execution_router = Arc::new(execution_router);

    info!(
        "🔗 Execution router initialized with chains: {:?}",
        execution_router.registered_chains()
    );

    // Initialize adapter registry now that we have executors
    let mut adapter_registry = AdapterRegistry::new();
    
    info!("⚙️  Initializing DEX adapters...");
    
    let solana_rpc = std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    
    // Create Raydium adapter and inject Solana executor for smart contract calls
    let raydium_adapter = if let Some(executor) = solana_executor.clone() {
        Arc::new(RaydiumAdapter::new(solana_rpc).with_executor(executor))
    } else {
        Arc::new(RaydiumAdapter::new(solana_rpc))
    };
    adapter_registry.register_dex("Raydium".to_string(), raydium_adapter);
    info!("✅ Raydium adapter registered (with smart contract integration)");
    
    let stellar_horizon = std::env::var("STELLAR_HORIZON_URL")
        .unwrap_or_else(|_| "https://horizon.stellar.org".to_string());
    adapter_registry.register_dex(
        "PhantomSwap".to_string(),
        Arc::new(PhantomSwapAdapter::new(stellar_horizon)),
    );
    info!("✅ PhantomSwap adapter registered");

    let near_rpc = std::env::var("NEAR_RPC_URL")
        .unwrap_or_else(|_| "https://rpc.mainnet.near.org".to_string());
    adapter_registry.register_dex(
        "Ref Finance".to_string(),
        Arc::new(NearDexAdapter::new(near_rpc)),
    );
    info!("✅ Ref Finance (NEAR) adapter registered");

    let adapter_registry = Arc::new(adapter_registry);

    // Initialize realtime quote engine
    let realtime_quote_engine = Arc::new(RealtimeQuoteEngine::new(adapter_registry.clone()));
    info!("✅ Realtime quote engine initialized");

    // Initialize wallet repository
    let wallet_repository = Arc::new(WalletRepository::new());
    info!("✅ Wallet repository initialized");

    // Initialize trade repository
    let trade_repository = Arc::new(TradeRepository::new());
    info!("✅ Trade repository initialized");

    // Initialize OHLC store (100 candles max per series)
    let ohlc_store = Arc::new(OhlcStore::new(100));
    info!("✅ OHLC store initialized (max 100 candles per series)");

    // Initialize price cache (1 second TTL)
    let price_cache = Arc::new(PriceCache::new(1000));
    info!("✅ Price cache initialized (<1ms per quote)");

    // Build application state
    let state = AppState {
        ledger: ledger.clone(),
        quote_engine: quote_engine.clone(),
        execution_router: execution_router.clone(),
        risk_controller: risk_controller.clone(),
        adapter_registry: adapter_registry.clone(),
        realtime_quote_engine: realtime_quote_engine.clone(),
        wallet_repository: wallet_repository.clone(),
        trade_repository: trade_repository.clone(),
        ohlc_store: ohlc_store.clone(),
        price_cache: price_cache.clone(),
        solana_executor: solana_executor.unwrap_or_else(|| {
            panic!("SOLANA_TREASURY_KEY must be set for token approval operations");
        }),
        stellar_executor: stellar_executor.unwrap_or_else(|| {
            panic!("STELLAR_TREASURY_KEY must be set for token approval operations");
        }),
        near_executor: near_executor.unwrap_or_else(|| {
            panic!("NEAR_TREASURY_KEY must be set for token approval operations");
        }),
    };

    // Display supported chain pairs
    info!("📋 Supported chain pairs:");
    for funding in Chain::all() {
        for execution in Chain::all() {
            if Chain::is_pair_supported(funding, execution) {
                info!("   {:?} → {:?}", funding, execution);
            }
        }
    }

    // Start background task to clean expired quotes (every hour)
    let ledger_cleanup = ledger.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            
            match ledger_cleanup.expire_old_pending_quotes().await {
                Ok(count) => {
                    if count > 0 {
                        info!("🗑️  Expired {} pending quotes", count);
                    }
                }
                Err(e) => error!("Failed to expire pending quotes: {:?}", e),
            }
            
            match ledger_cleanup.expire_old_committed_quotes().await {
                Ok(count) => {
                    if count > 0 {
                        info!("🗑️  Expired {} committed quotes", count);
                    }
                }
                Err(e) => error!("Failed to expire committed quotes: {:?}", e),
            }
        }
    });
    info!("✅ Quote expiration cleanup task started (hourly)");

    Ok(state)
}


async fn initialize_database(database_url: &str) -> AppResult<PgPool> {
    info!("📊 Connecting to database...");

    let pool = PgPoolOptions::new()
        .max_connections(300)
        .min_connections(30)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .connect(database_url)
        .await?;

    info!("✓ Database pool configured: 200 max connections");

    // Run migrations
    info!("🔄 Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    info!("✓ Database initialized");
    Ok(pool)
}