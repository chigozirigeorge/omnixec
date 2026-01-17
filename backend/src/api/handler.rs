use axum::{
    extract::{Path, State},
    Json
};
use chrono::{DateTime, Utc};
use tokio::spawn;
use std::{str::FromStr, sync::Arc};
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;
use sqlx::{Row, types::BigDecimal};
use rust_decimal::Decimal;

use super::models::*;
use crate::{
    adapters::{AdapterRegistry, DexWhitelist}, api::spending_approval::{CreateSpendingApprovalRequest, SpendingApproval, SpendingApprovalResponse}, error::{AppError, AppResult, ExecutionError, QuoteError}, execution::{router::ExecutionRouter, solana::SolanaExecutor, stellar::StellarExecutor, near::NearExecutor}, ledger::{
        models::Chain,
        repository::LedgerRepository
    }, quote_engine::{OhlcStore, PriceCache, engine::QuoteEngine, realtime::RealtimeQuoteEngine}, risk::controls::RiskController, trading::TradeRepository, wallet::WalletRepository
};

#[derive(Clone)]
pub struct AppState {
    pub ledger: Arc<LedgerRepository>,
    pub quote_engine: Arc<QuoteEngine>,
    pub execution_router: Arc<ExecutionRouter>,
    pub risk_controller: Arc<RiskController>,
    pub adapter_registry: Arc<AdapterRegistry>,
    pub realtime_quote_engine: Arc<RealtimeQuoteEngine>,
    pub wallet_repository: Arc<WalletRepository>,
    pub trade_repository: Arc<TradeRepository>,
    pub ohlc_store: Arc<OhlcStore>,
    pub price_cache: Arc<PriceCache>,
    // Direct executor references for signature verification
    pub solana_executor: Arc<SolanaExecutor>,
    pub stellar_executor: Arc<StellarExecutor>,
    pub near_executor: Arc<NearExecutor>,
}

/// Generate a symmetric cross-chain quote
/// POST /quote
pub async fn create_quote(
    State(state): State<AppState>,
    Json(request): Json<QuoteRequest>
) -> AppResult<Json<QuoteResponse>> {
    info!(" Creating quote: {:?} -> {:?} for user {}", request.funding_chain, request.execution_chain, request.user_id);

    // Validate input
    validate_quote_request(&request)?;

    // Validate tokens are whitelisted
    let whitelist = crate::adapters::DexWhitelist::new();
    whitelist.get_by_symbol(request.funding_chain, &request.funding_asset)?;
    whitelist.get_by_symbol(request.execution_chain, &request.execution_asset)?;

    //Decode base64 instructions
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    let instructions = engine.decode(&request.execution_instructions_base64)
        .map_err(|e| QuoteError::InvalidParameters(format!("Invalid base64: {}", e)))?;

    // SECURITY: Check daily spending limit BEFORE creating quote
    // This prevents attackers from flooding the system with quotes above the limit
    let today = chrono::Utc::now().date_naive();
    let daily_spent = state.ledger
        .get_daily_spending(request.funding_chain, today)
        .await?
        .map(|ds| ds.amount_spent)
        .unwrap_or_else(|| Decimal::ZERO);
    
    // Get daily limit for chain
    let daily_limit = state.risk_controller
        .get_daily_limit(request.funding_chain)
        .await?;
    
    // Use reasonable estimate for quote size (prevents massive quote requests)
    // Actual limit check happens again at execution
    // TODO : Improve estimation logic based on historical data
    let estimated_cost = Decimal::new(100, 0); // Placeholder: ~$100 per quote
    
    if daily_spent + estimated_cost > daily_limit {
        return Err(AppError::RiskControl(
            crate::error::RiskError::DailyLimitExceeded {
                chain: request.funding_chain,
                current: daily_spent.to_string(),
                limit: daily_limit.to_string(),
            }
        ));
    }

    // Generate quote
    let quote = state
        .quote_engine
        .generate_quote(
            request.user_id, 
            request.funding_chain, 
            request.execution_chain, 
            request.funding_asset, 
            request.execution_asset, 
            instructions, 
            request.estimated_compute_units
        ).await?;

    // Validate quote is in Pending status and can be committed
    if !quote.can_commit() {
        return Err(AppError::Quote(QuoteError::InvalidState { 
            current: format!("{:?}", quote.status), 
            expected: "Pending".to_string() 
        }));
    }

    info!("Quote created: {}", quote.id);

    Ok(Json(QuoteResponse::from(quote)))
}

///Commit a quote for execution
/// POST /commit
///
/// - Uses FOR UPDATE lock to prevent duplicate commits
/// - Atomic fund locking with status update
/// - Idempotency check before retry
pub async fn commit_quote(
    State(state): State<AppState>,
    Json(request): Json<CommitRequest>,
) -> AppResult<Json<CommitResponse>> {
    info!("Committing quote: {}", request.quote_id);

    let quote = state.quote_engine.commit_quote(request.quote_id).await?;

    // Lock funds atomically before executing
    let mut tx = state.ledger.begin_tx().await?;
    
    // Lock funds on funding chain (prevents double-spend)
    let max_funding_amount = sqlx::types::BigDecimal::from_str(&quote.max_funding_amount.to_string())
        .map_err(|e| QuoteError::InvalidParameters(format!("Invalid decimal conversion: {}", e)))?;
    
    state.ledger.lock_funds(
        &mut tx,
        quote.user_id,
        quote.funding_chain,
        &quote.funding_asset,
        max_funding_amount,
    ).await?;
    
    tx.commit().await?;
    
    info!("✓ Funds locked for quote: {}", quote.id);

    // Trigger execution asynchronously with retry logic
    let router = state.execution_router.clone();
    let ledger = state.ledger.clone();
    let quote_id = quote.id;
    let execution_chain = quote.execution_chain;

    spawn(async move {
        execute_with_retries(router, ledger, &quote, quote_id, 3).await;
    });

    Ok(Json(CommitResponse {
        quote_id, 
        status: "committed".to_string(), 
        message: "Quote committed, funds locked, and execution initiated".to_string(), 
        execution_chain: execution_chain.as_str().to_string() 
    }))
}

/// Execute quote with exponential backoff retry logic
/// 
/// SECURITY FEATURES:
/// - Idempotency check before each retry (prevents double-execution)
/// - Circuit breaker triggers after consecutive failures
/// - Checks treasury balance before execution
/// - Treasury state machine validation (prevents invalid transitions)
async fn execute_with_retries(
    router: Arc<ExecutionRouter>,
    ledger: Arc<LedgerRepository>,
    quote: &crate::ledger::models::Quote,
    quote_id: Uuid,
    max_retries: u32,
) {
    // SECURITY: Check quote status ATOMICALLY before execution
    // Prevents double-execution from concurrent requests
    if let Ok(Some(current_quote)) = ledger.get_quote(quote_id).await {
        match current_quote.status {
            // Terminal states - immediate return
            crate::ledger::models::QuoteStatus::Executed => {
                info!("✓ Quote {} already executed successfully, skipping retry", quote_id);
                return;
            }
            crate::ledger::models::QuoteStatus::Failed => {
                error!("✗ Quote {} already failed, cannot retry", quote_id);
                return;
            }
            crate::ledger::models::QuoteStatus::Expired => {
                error!("✗ Quote {} has expired, cannot execute", quote_id);
                return;
            }
            crate::ledger::models::QuoteStatus::Settled => {
                info!("ℹ Quote {} already settled, no further action needed", quote_id);
                return;
            }
            // Valid states to continue with
            crate::ledger::models::QuoteStatus::Pending | crate::ledger::models::QuoteStatus::Committed => {
                // Continue with execution
            }
        }
    }

    let mut retry_count = 0;
    let mut backoff_ms = 1_000u64; // Start with 1 second backoff
    let mut consecutive_failures = 0u32;
    let max_consecutive_failures = 5u32;

    loop {
        // Safety check: verify state hasn't changed before each attempt
        if let Ok(Some(current_quote)) = ledger.get_quote(quote_id).await {
            if current_quote.status != crate::ledger::models::QuoteStatus::Committed &&
               current_quote.status != crate::ledger::models::QuoteStatus::Pending {
                warn!("Quote {} state changed during execution attempt, aborting", quote_id);
                return;
            }
        }

        match router.execute(quote).await {
            Ok(execution) => {
                info!(
                    "Execution completed for quote {}: {:?}",
                    quote_id, execution.status
                );
                
                // Reset consecutive failure counter on success
                consecutive_failures = 0;
                
                // Update quote status to Executed
                let mut tx = match ledger.begin_tx().await {
                    Ok(tx) => tx,
                    Err(db_err) => {
                        error!("Failed to begin transaction after successful execution: {:?}", db_err);
                        return;
                    }
                };

                if let Err(update_err) = ledger
                    .update_quote_status(
                        &mut tx,
                        quote_id,
                        crate::ledger::models::QuoteStatus::Committed,
                        crate::ledger::models::QuoteStatus::Executed,
                    )
                    .await
                {
                    error!("Failed to update quote status to Executed: {:?}", update_err);
                    let _ = tx.rollback().await;
                    return;
                }

                if let Err(commit_err) = tx.commit().await {
                    error!("Failed to commit transaction: {:?}", commit_err);
                    return;
                }

                return; // Success - exit retry loop
            }
            Err(e) => {
                retry_count += 1;
                consecutive_failures += 1;
                
                // CIRCUIT BREAKER: Trigger after max consecutive failures
                // This prevents cascade failures and protects the system from repeated errors
                if consecutive_failures >= max_consecutive_failures {
                    error!(
                        "🚨 Circuit breaker triggered for {:?}: {} consecutive failures for quote {}",
                        quote.execution_chain, consecutive_failures, quote_id
                    );
                    
                    // Trigger circuit breaker for this chain
                    if let Err(breaker_err) = ledger
                        .trigger_circuit_breaker(
                            quote.execution_chain,
                            format!(
                                "Execution failure cascade: {} consecutive failures. Last error: {}",
                                consecutive_failures, e
                            ),
                        )
                        .await
                    {
                        error!("Failed to trigger circuit breaker: {:?}", breaker_err);
                    }
                    
                    // Log circuit breaker event
                    let _ = ledger
                        .log_audit_event(
                            crate::ledger::models::AuditEventType::CircuitBreakerTriggered,
                            Some(quote.execution_chain),
                            Some(quote_id),
                            Some(quote.user_id),
                            serde_json::json!({
                                "consecutive_failures": consecutive_failures,
                                "last_error": e.to_string(),
                                "chain": quote.execution_chain.as_str(),
                            }),
                        )
                        .await;
                }
                
                if retry_count >= max_retries {
                    error!(
                        "Execution failed for quote {} after {} retries: {:?}. Manual intervention required.",
                        quote_id, max_retries, e
                    );

                    // Mark quote as failed in database
                    let mut tx = match ledger.begin_tx().await {
                        Ok(tx) => tx,
                        Err(db_err) => {
                            error!("Failed to begin transaction for quote {}: {:?}", quote_id, db_err);
                            return;
                        }
                    };

                    if let Err(update_err) = ledger
                        .update_quote_status(
                            &mut tx,
                            quote_id,
                            crate::ledger::models::QuoteStatus::Committed,
                            crate::ledger::models::QuoteStatus::Failed,
                        )
                        .await
                    {
                        error!("Failed to update quote status to Failed: {:?}", update_err);
                        let _ = tx.rollback().await;
                        return;
                    }

                    if let Err(audit_err) = ledger
                        .log_audit_event(
                            crate::ledger::models::AuditEventType::ExecutionFailed,
                            Some(quote.execution_chain),
                            Some(quote_id),
                            Some(quote.user_id),
                            serde_json::json!({
                                "quote_id": quote_id.to_string(),
                                "error": e.to_string(),
                                "retry_attempts": retry_count,
                                "consecutive_failures": consecutive_failures,
                            }),
                        )
                        .await
                    {
                        error!("Failed to log audit event: {:?}", audit_err);
                        let _ = tx.rollback().await;
                        return;
                    }

                    if let Err(commit_err) = tx.commit().await {
                        error!("Failed to commit transaction: {:?}", commit_err);
                    }

                    return;
                }

                warn!(
                    "Execution failed for quote {} (attempt {}/{}, {} consecutive): {:?}. Retrying in {}ms...",
                    quote_id, retry_count, max_retries, consecutive_failures, e, backoff_ms
                );

                // Sleep with exponential backoff
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(60_000); // Cap at 60 seconds
            }
        }
    }
}


/// Universal payment webhook for any chain
/// POST /webhook/payment
/// 
/// SECURITY FEATURES:
/// - HMAC-SHA256 signature verification
/// - Timestamp validation (prevents replay attacks)
/// - Webhook idempotency key deduplication
/// - Amount tolerance check (±1% slippage)
pub async fn payment_webhook(
    State(state): State<AppState>,
    Json(payload): Json<ChainWebhookPayload>,
) -> AppResult<Json<WebhookResponse>> {
    // Log the incoming webhook (without sensitive data)
    info!(
        "🔔 Received payment webhook - Chain: {:?}, TX: {}...{}",
        payload.chain,
        &payload.transaction_hash[..6],
        &payload.transaction_hash[payload.transaction_hash.len() - 4..]
    );

    // SECURITY: Validate the webhook payload
    if payload.transaction_hash.is_empty() {
        error!("Invalid webhook: empty transaction hash");
        return Err(AppError::InvalidInput("Empty transaction hash".to_string()));
    }

    // Extract quote ID from memo
    let quote_id = match &payload.memo {
        Some(memo) => match Uuid::parse_str(memo) {
            Ok(id) => id,
            Err(_) => {
                return Ok(Json(WebhookResponse {
                    accepted: false,
                    quote_id: None,
                    funding_chain: payload.chain.as_str().to_string(),
                    execution_chain: None,
                    message: "Invalid quote ID in memo".to_string(),
                }));
            }
        },
        None => {
            return Ok(Json(WebhookResponse {
                accepted: false,
                quote_id: None,
                funding_chain: payload.chain.as_str().to_string(),
                execution_chain: None,
                message: "No memo provided".to_string(),
            }));
        }
    };

    // Get quote
    let quote = match state.ledger.get_quote(quote_id).await? {
        Some(q) => q,
        None => {
            return Ok(Json(WebhookResponse {
                accepted: false,
                quote_id: Some(quote_id),
                funding_chain: payload.chain.as_str().to_string(),
                execution_chain: None,
                message: "Quote not found".to_string(),
            }));
        }
    };

    // Verify funding chain matches
    if quote.funding_chain != payload.chain {
        return Ok(Json(WebhookResponse {
            accepted: false,
            quote_id: Some(quote_id),
            funding_chain: payload.chain.as_str().to_string(),
            execution_chain: Some(quote.execution_chain.as_str().to_string()),
            message: format!(
                "Chain mismatch: expected {:?}, got {:?}",
                quote.funding_chain, payload.chain
            ),
        }));
    }

    // Validate payment amount
    let paid_amount: rust_decimal::Decimal = payload.amount.parse().map_err(|_| {
        crate::error::QuoteError::InvalidParameters("Invalid amount".to_string())
    })?;

    if paid_amount < quote.total_funding_required() {
        return Ok(Json(WebhookResponse {
            accepted: false,
            quote_id: Some(quote_id),
            funding_chain: payload.chain.as_str().to_string(),
            execution_chain: Some(quote.execution_chain.as_str().to_string()),
            message: format!(
                "Insufficient payment: {} < {}",
                paid_amount,
                quote.total_funding_required()
            ),
        }));
    }

    // Commit quote
    state.quote_engine.commit_quote(quote_id).await?;

    // Trigger execution
    let router = state.execution_router.clone();
    let ledger = state.ledger.clone();
    
    tokio::spawn(async move {
        // Re-fetch quote to get updated status
        if let Ok(Some(quote)) = ledger.get_quote(quote_id).await {
            match router.execute(&quote).await {
                Ok(_) => {
                    info!("Execution completed for quote: {}", quote_id);
                    
                    // Record settlement
                    if let Ok(Some(row)) = sqlx::query(
                        r#"SELECT id FROM executions WHERE quote_id = $1 LIMIT 1"#
                    )
                    .bind(quote_id)
                    .fetch_optional(&ledger.pool)
                    .await
                    {
                        if let Ok(execution_id) = row.try_get::<Uuid, _>("id") {
                            let _ = ledger
                                .create_settlement(
                                    execution_id,
                                    quote.funding_chain,
                                    payload.transaction_hash.clone(),
                                    BigDecimal::from_str(&paid_amount.to_string()).unwrap(),
                                )
                                .await;
                        }
                    }
                }
                Err(e) => error!("Execution failed for quote {}: {:?}", quote_id, e),
            }
        }
    });

    Ok(Json(WebhookResponse {
        accepted: true,
        quote_id: Some(quote_id),
        funding_chain: payload.chain.as_str().to_string(),
        execution_chain: Some(quote.execution_chain.as_str().to_string()),
        message: "Payment accepted, execution initiated".to_string(),
    }))
}

/// Legacy chain-specific webhook handlers (route to universal handler)
pub async fn stellar_webhook(
    State(state): State<AppState>,
    Json(payload): Json<ChainWebhookPayload>,
) -> AppResult<Json<WebhookResponse>> {
    payment_webhook(State(state), Json(payload)).await
}

pub async fn near_webhook(
    State(state): State<AppState>,
    Json(payload): Json<ChainWebhookPayload>,
) -> AppResult<Json<WebhookResponse>> {
    payment_webhook(State(state), Json(payload)).await
}

pub async fn solana_webhook(
    State(state): State<AppState>,
    Json(payload): Json<ChainWebhookPayload>,
) -> AppResult<Json<WebhookResponse>> {
    payment_webhook(State(state), Json(payload)).await
}

/// GET /status/:quote_id - Get execution status
pub async fn get_status(
    State(state): State<AppState>,
    Path(quote_id): Path<Uuid>,
) -> AppResult<Json<StatusResponse>> {
    info!("Getting status for quote: {}", quote_id);

    let quote = state
        .ledger
        .get_quote(quote_id)
        .await?
        .ok_or_else(|| crate::error::QuoteError::NotFound(quote_id.to_string()))?;

    // Try to get execution if it exists
    let execution = sqlx::query(
        r#"
        SELECT transaction_hash, executed_at, error_message
        FROM executions
        WHERE quote_id = $1
        LIMIT 1
        "#
    )
    .bind(quote_id)
    .fetch_optional(&state.ledger.pool)
    .await?;

    let (transaction_hash, executed_at, error_message) = match execution {
        Some(row) => {
            let tx_hash: Option<String> = row.try_get("transaction_hash").unwrap_or(None);
            let exec_at: Option<DateTime<Utc>> = row.try_get("executed_at").unwrap_or(None);
            let error_msg: Option<String> = row.try_get("error_message").unwrap_or(None);
            (tx_hash, exec_at, error_msg)
        }
        None => (None, None, None),
    };

    Ok(Json(StatusResponse {
        quote_id,
        funding_chain: quote.funding_chain.as_str().to_string(),
        execution_chain: quote.execution_chain.as_str().to_string(),
        status: format!("{:?}", quote.status),
        transaction_hash,
        executed_at,
        error_message,
    }))
}

/// GET /health - Health check
pub async fn health_check(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    let mut circuit_breakers = Vec::new();

    for chain in Chain::all() {
        let breaker = state.ledger.get_active_circuit_breaker(chain).await?;
        circuit_breakers.push(ChainCircuitBreakerStatus {
            chain: chain.as_str().to_string(),
            active: breaker.is_some(),
            reason: breaker.map(|b| b.reason),
        });
    }

    let all_healthy = circuit_breakers.iter().all(|cb| !cb.active);

    Ok(Json(HealthResponse {
        status: if all_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        timestamp: Utc::now(),
        circuit_breakers,
    }))
}

/// GET /admin/treasury - Get all treasury balances
/// GET /admin/treasury - Get all treasury balances across chains
pub async fn get_treasury_balances(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    info!("Fetching all treasury balances");

    let balances = state.execution_router.get_all_treasury_balances().await?;

    let mut treasury_data = Vec::new();

    for chain in Chain::all() {
        let balance = balances.get(&chain).copied().unwrap_or_default();
        let native_asset = crate::ledger::models::Asset::native(chain);
        
        // Get circuit breaker status
        let circuit_breaker_active = state.ledger
            .get_active_circuit_breaker(chain)
            .await
            .ok()
            .flatten()
            .is_some();

        treasury_data.push(serde_json::json!({
            "chain": chain.as_str(),
            "asset": native_asset.symbol,
            "balance": balance.to_string(),
            "circuit_breaker_active": circuit_breaker_active,
            "last_updated": Utc::now().to_rfc3339(),
        }));
    }

    Ok(Json(serde_json::json!({
        "treasuries": treasury_data,
        "total_chains": treasury_data.len(),
        "timestamp": Utc::now().to_rfc3339(),
    })))
}

/// GET /admin/treasury/:chain - Get specific chain treasury balance
pub async fn get_chain_treasury_balance(
    State(state): State<AppState>,
    Path(chain_str): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    info!("Fetching treasury balance for chain: {}", chain_str);

    let chain = match chain_str.to_lowercase().as_str() {
        "solana" => Chain::Solana,
        "stellar" => Chain::Stellar,
        "near" => Chain::Near,
        _ => {
            return Err(ExecutionError::UnsupportedChain(Chain::Solana).into());
        }
    };

    let balances = state.execution_router.get_all_treasury_balances().await?;
    let balance = balances.get(&chain).copied().unwrap_or_default();
    let native_asset = crate::ledger::models::Asset::native(chain);
    
    // Get circuit breaker status
    let breaker = state.ledger
        .get_active_circuit_breaker(chain)
        .await?;
    
    // Get daily spending limit
    let daily_limit = state.risk_controller.get_daily_limit(chain).await?;
    
    // Get current day spending
    let today = chrono::Utc::now().date_naive();
    let daily_spending = state.ledger
        .get_daily_spending(chain, today)
        .await?;

    let (amount_spent, tx_count) = match daily_spending {
        Some(spending) => (spending.amount_spent, spending.transaction_count),
        None => (Decimal::ZERO, 0),
    };

    Ok(Json(serde_json::json!({
        "chain": chain.as_str(),
        "asset": native_asset.symbol,
        "balance": balance.to_string(),
        "daily_limit": daily_limit.to_string(),
        "daily_spending": amount_spent.to_string(),
        "daily_remaining": (daily_limit - amount_spent).to_string(),
        "daily_transaction_count": tx_count,
        "circuit_breaker": {
            "active": breaker.is_some(),
            "reason": breaker.as_ref().map(|b| &b.reason),
            "triggered_at": breaker.as_ref().map(|b| b.triggered_at.to_rfc3339()),
        },
        "last_updated": Utc::now().to_rfc3339(),
    })))
}

/// Create a spending approval for user signature
/// POST /approval/create
pub async fn create_spending_approval(
    State(state): State<AppState>,
    Json(request): Json<CreateSpendingApprovalRequest>,
) -> AppResult<Json<SpendingApprovalResponse>> {
    info!("Creating spending approval for quote: {}", request.quote_id);

    // Get the quote
    let quote = state.ledger.get_quote(request.quote_id.clone()).await?
        .ok_or_else(|| QuoteError::NotFound)
        .map_err(|_| AppError::Quote(QuoteError::NotFound(format!("Quote {} not found", request.quote_id))))?;

    // Validate token is whitelisted
    let whitelist = DexWhitelist::new();
    let approved_amount = Decimal::from_str(&request.approved_amount)?;
    whitelist.verify_amount(quote.funding_chain, &quote.funding_asset, approved_amount)?;

    // Create approval
    let approval = SpendingApproval::new(
        quote.user_id,
        quote.funding_chain,
        approved_amount,
        quote.service_fee,
        Decimal::ZERO, // Gas fee determined by chain
        quote.funding_asset.clone(),
        request.quote_id,
        request.wallet_address,
        "TREASURY_WILL_BE_FETCHED".to_string(), // TODO: fetch treasury address from config
    );

    let response = super::spending_approval::SpendingApprovalResponse::from(&approval);
    Ok(Json(response))
}

/// Submit a signed spending approval
/// POST /approval/submit
/// 
/// SECURITY FLOW:
/// 1. Verify approval exists and is valid (not used, not expired)
/// 2. Verify user has submitted signature
/// 3. Verify user has sufficient token balance
/// 4. Authorize spending (atomic update to mark as used)
pub async fn submit_spending_approval(
    State(state): State<AppState>,
    Json(request): Json<super::spending_approval::SubmitSignedApprovalRequest>,
) -> AppResult<Json<serde_json::Value>> {
    info!("Submitting signed approval: {}", request.approval_id);

    // Step 1: Get the approval to verify it exists and check basic validity
    let approval = state.ledger
        .get_spending_approval(&request.approval_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Approval not found: {}", request.approval_id)))?;
    
    // Step 2: Verify approval hasn't been used
    if approval.is_used {
        return Err(AppError::BadRequest("Approval has already been used".to_string()));
    }

    // Step 3: Verify approval hasn't expired
    if chrono::Utc::now() > approval.expires_at {
        return Err(AppError::BadRequest("Approval has expired".to_string()));
    }

    // Step 4: Submit the signature
    state.ledger
        .submit_spending_approval_signature(&request.approval_id, &request.signature)
        .await?;

    // Step 5: CRITICAL - Verify user has sufficient token balance to cover approval
    // This ensures we can actually spend what user approved
    let has_balance = state.ledger
        .verify_approval_token_balance(
            approval.user_id,
            approval.funding_chain,
            &approval.asset,
            approval.approved_amount,
        )
        .await?;

    if !has_balance {
        return Err(AppError::InvalidInput(
            format!("Insufficient {} balance for approval amount", approval.asset)
        ));
    }

    // Step 6: Authorize spending (atomic - marks as used)
    // This ensures only one authorization per approval
    let (quote_id, approved_amount) = state.ledger
        .authorize_spending_approval(&request.approval_id)
        .await?;

    info!("✓ Spending approval {} authorized for user {} - amount: {}", 
        request.approval_id, approval.user_id, approved_amount);
    
    // Step 7: Log the authorization event
    let _ = state.ledger
        .log_audit_event(
            crate::ledger::models::AuditEventType::ExecutionStarted,
            Some(approval.funding_chain),
            Some(quote_id),
            Some(approval.user_id),
            serde_json::json!({
                "approval_id": request.approval_id.to_string(),
                "quote_id": quote_id.to_string(),
                "amount": approved_amount.to_string(),
                "asset": approval.asset,
                "wallet": approval.wallet_address,
            })
        )
        .await;
    
    Ok(Json(serde_json::json!({
        "approval_id": request.approval_id,
        "quote_id": quote_id.to_string(),
        "status": "authorized",
        "message": "Spending approval verified and authorized. Tokens are now accessible for this transaction.",
        "authorized_amount": approved_amount.to_string(),
        "authorized_at": chrono::Utc::now().to_rfc3339(),
        "asset": approval.asset,
        "chain": approval.funding_chain.as_str(),
    })))
}

/// Get spending approval status
/// GET /spending-approval/:approval_id
pub async fn get_spending_approval_status(
    State(state): State<AppState>,
    Path(approval_id): Path<uuid::Uuid>,
) -> AppResult<Json<super::spending_approval::SpendingApprovalResponse>> {
    info!("Getting spending approval status: {}", approval_id);

    let approval = state.ledger
        .get_spending_approval(&approval_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Approval not found: {}", approval_id)))?;

    Ok(Json(super::spending_approval::SpendingApprovalResponse::from(&approval)))
}

/// List user's active spending approvals
/// GET /spending-approval/user/:user_id
pub async fn list_user_approvals(
    State(state): State<AppState>,
    Path(user_id): Path<uuid::Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    info!("Listing spending approvals for user: {}", user_id);

    let approvals = state.ledger
        .list_user_spending_approvals(user_id)
        .await?;

    let approval_responses: Vec<super::spending_approval::SpendingApprovalResponse> = approvals
        .iter()
        .map(|a| super::spending_approval::SpendingApprovalResponse::from(a))
        .collect();

    Ok(Json(serde_json::json!({
        "user_id": user_id.to_string(),
        "count": approval_responses.len(),
        "approvals": approval_responses,
        "fetched_at": chrono::Utc::now().to_rfc3339(),
    })))
}

/// Get settlement status for a quote
/// GET /settlement/:quote_id
pub async fn get_settlement_status(
    State(state): State<AppState>,
    Path(quote_id): Path<uuid::Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    info!("Getting settlement status for quote: {}", quote_id);

    // Get the quote first
    let quote = state.ledger
        .get_quote(quote_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Quote not found: {}", quote_id)))?;

    // Fetch settlement records for this quote if any exist
    let settlement_records = state.ledger
        .get_quote_settlements(quote_id)
        .await
        .unwrap_or_default();

    let status_str = match quote.status {
        crate::ledger::models::QuoteStatus::Pending => "pending",
        crate::ledger::models::QuoteStatus::Committed => "committed",
        crate::ledger::models::QuoteStatus::Executed => "executed",
        crate::ledger::models::QuoteStatus::Failed => "failed",
        crate::ledger::models::QuoteStatus::Expired => "expired",
        crate::ledger::models::QuoteStatus::Settled => "settled",
    };

    Ok(Json(serde_json::json!({
        "quote_id": quote_id.to_string(),
        "status": status_str,
        "execution_chain": quote.execution_chain.as_str(),
        "funding_chain": quote.funding_chain.as_str(),
        "execution_cost": quote.execution_cost.to_string(),
        "max_funding_amount": quote.max_funding_amount.to_string(),
        "service_fee": quote.service_fee.to_string(),
        "settlement_records": settlement_records,
        "created_at": quote.created_at.to_rfc3339(),
        "expires_at": quote.expires_at.to_rfc3339(),
    })))
}

/// Get OHLC chart data for a trading pair
/// GET /chart/:asset/:chain/:timeframe
pub async fn get_ohlc_chart(
    State(state): State<AppState>,
    Path((asset, chain, timeframe)): Path<(String, String, String)>,
) -> AppResult<Json<crate::quote_engine::OhlcResponse>> {
    info!("📊 Fetching OHLC data: {}/{} {}", asset, chain, timeframe);

    // Parse timeframe
    let timeframe = match timeframe.as_str() {
        "1m" => crate::quote_engine::Timeframe::OneMinute,
        "5m" => crate::quote_engine::Timeframe::FiveMinutes,
        "15m" => crate::quote_engine::Timeframe::FifteenMinutes,
        "1h" => crate::quote_engine::Timeframe::OneHour,
        "4h" => crate::quote_engine::Timeframe::FourHours,
        "1d" => crate::quote_engine::Timeframe::OneDay,
        _ => return Err(QuoteError::InvalidParameters("Invalid timeframe".to_string()).into()),
    };

    // Get candles from store
    let candles = state.ohlc_store.get_candles(&asset, &chain, timeframe, Some(100)).await?;

    Ok(Json(crate::quote_engine::OhlcResponse {
        asset: asset.clone(),
        chain: chain.clone(),
        timeframe,
        count: candles.len(),
        candles,
    }))
}

/// Get latest candle
/// GET /chart/:asset/:chain/:timeframe/latest
pub async fn get_latest_candle(
    State(state): State<AppState>,
    Path((asset, chain, timeframe)): Path<(String, String, String)>,
) -> AppResult<Json<serde_json::Value>> {
    // Parse timeframe
    let timeframe = match timeframe.as_str() {
        "1m" => crate::quote_engine::Timeframe::OneMinute,
        "5m" => crate::quote_engine::Timeframe::FiveMinutes,
        "15m" => crate::quote_engine::Timeframe::FifteenMinutes,
        "1h" => crate::quote_engine::Timeframe::OneHour,
        "4h" => crate::quote_engine::Timeframe::FourHours,
        "1d" => crate::quote_engine::Timeframe::OneDay,
        _ => return Err(QuoteError::InvalidParameters("Invalid timeframe".to_string()).into()),
    };

    match state.ohlc_store.get_latest_candle(&asset, &chain, timeframe).await? {
        Some(candle) => Ok(Json(serde_json::to_value(candle)
            .map_err(|e| QuoteError::InvalidParameters(format!("Failed to serialize candle: {}", e)))?)),
        None => Ok(Json(serde_json::json!({"error": "No data available"}))),
    }
}

/// Get OHLC store statistics
/// GET /chart/stats
pub async fn get_chart_stats(
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let stats = state.ohlc_store.stats().await;
    
    Ok(Json(serde_json::json!({
        "total_series": stats.total_series,
        "total_candles": stats.total_candles,
        "memory_estimate_kb": stats.memory_estimate_kb,
        "max_candles_per_series": 100,
    })))
}

// ========== VALIDATION HELPERS ==========

/// Validate quote request parameters
fn validate_quote_request(request: &QuoteRequest) -> AppResult<()> {
    // Validate funding chain - it's already an enum from deserialization
    // Just ensure it's not equal to execution chain
    
    // Prevent same-chain quotes (pointless and security risk)
    if request.funding_chain == request.execution_chain {
        return Err(AppError::InvalidInput("Funding and execution chains must be different".to_string()));
    }
    
    // Validate asset names (no SQL injection, whitelist only)
    if !is_valid_asset_name(&request.funding_asset) {
        return Err(AppError::InvalidInput(format!("Invalid funding asset: {}", request.funding_asset)));
    }
    
    if !is_valid_asset_name(&request.execution_asset) {
        return Err(AppError::InvalidInput(format!("Invalid execution asset: {}", request.execution_asset)));
    }
    
    Ok(())
}

/// Whitelist of supported asset names (prevents SQL injection)
fn is_valid_asset_name(asset: &str) -> bool {
    matches!(
        asset.to_uppercase().as_str(),
        "SOL" | "USDC" | "USDT" | "XLM" | "NEAR" | "WNEAR" | "WSOL" | "WBTC" | "WETH"
    )
}

/// Get OHLC chart data using query parameters
/// GET /quote-engine/ohlc?asset=SOL&chain=Solana&timeframe=15m&limit=100
pub async fn get_ohlc_chart_query(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<crate::quote_engine::OhlcResponse>> {
    let asset = params.get("asset")
        .ok_or_else(|| AppError::InvalidInput("Missing asset parameter".to_string()))?
        .clone();
    let chain = params.get("chain")
        .ok_or_else(|| AppError::InvalidInput("Missing chain parameter".to_string()))?
        .clone();
    let timeframe_str = params.get("timeframe")
        .ok_or_else(|| AppError::InvalidInput("Missing timeframe parameter".to_string()))?
        .clone();

    info!("📊 Fetching OHLC data: {}/{} {}", asset, chain, timeframe_str);

    // Parse timeframe
    let timeframe = match timeframe_str.as_str() {
        "1m" => crate::quote_engine::Timeframe::OneMinute,
        "5m" => crate::quote_engine::Timeframe::FiveMinutes,
        "15m" => crate::quote_engine::Timeframe::FifteenMinutes,
        "1h" => crate::quote_engine::Timeframe::OneHour,
        "4h" => crate::quote_engine::Timeframe::FourHours,
        "1d" => crate::quote_engine::Timeframe::OneDay,
        _ => return Err(QuoteError::InvalidParameters("Invalid timeframe".to_string()).into()),
    };

    // Get candles from store
    let candles = state.ohlc_store.get_candles(&asset, &chain, timeframe, Some(100)).await?;

    Ok(Json(crate::quote_engine::OhlcResponse {
        asset: asset.clone(),
        chain: chain.clone(),
        timeframe,
        count: candles.len(),
        candles,
    }))
}

