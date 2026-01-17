use crate::ledger::models::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ========== REQUEST MODELS ==========

/// Request to create a symmetric cross-chain quote
#[derive(Debug, Deserialize)]
pub struct QuoteRequest {
    pub user_id: Uuid,
    
    // Symmetric chain pair
    pub funding_chain: Chain,
    pub execution_chain: Chain,
    
    // Assets
    pub funding_asset: String,
    pub execution_asset: String,
    
    /// Base64 encoded execution instructions (chain-specific)
    pub execution_instructions_base64: String,
    
    /// Optional compute units (for Solana execution)
    pub estimated_compute_units: Option<i32>,
}

/// Request to commit a quote (after payment detected)
#[derive(Debug, Deserialize)]
pub struct CommitRequest {
    pub quote_id: Uuid,
}

/// Universal webhook payload (any chain can send)
#[derive(Debug, Deserialize)]
pub struct ChainWebhookPayload {
    pub chain: Chain,
    pub transaction_hash: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub asset: String,
    pub memo: Option<String>,
    pub timestamp: DateTime<Utc>,
}

// ========== RESPONSE MODELS ==========

/// Symmetric quote response
#[derive(Debug, Serialize)]
pub struct QuoteResponse {
    pub quote_id: Uuid,
    pub user_id: Uuid,
    
    // Chain pair
    pub funding_chain: String,
    pub execution_chain: String,
    
    // Assets
    pub funding_asset: String,
    pub execution_asset: String,
    
    // Costs
    pub max_funding_amount: String,
    pub execution_cost: String,
    pub service_fee: String,
    
    // Payment details
    pub payment_address: String,
    pub expires_at: DateTime<Utc>,
    pub nonce: String,
}

impl From<Quote> for QuoteResponse {
    fn from(quote: Quote) -> Self {
        Self {
            quote_id: quote.id,
            user_id: quote.user_id,
            funding_chain: quote.funding_chain.as_str().to_string(),
            execution_chain: quote.execution_chain.as_str().to_string(),
            funding_asset: quote.funding_asset,
            execution_asset: quote.execution_asset,
            max_funding_amount: quote.max_funding_amount.to_string(),
            execution_cost: quote.execution_cost.to_string(),
            service_fee: quote.service_fee.to_string(),
            payment_address: quote.payment_address.unwrap_or_default(),
            expires_at: quote.expires_at,
            nonce: quote.nonce,
        }
    }
}

/// Commit response
#[derive(Debug, Serialize)]
pub struct CommitResponse {
    pub quote_id: Uuid,
    pub status: String,
    pub message: String,
    pub execution_chain: String,
}

/// Execution status response
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub quote_id: Uuid,
    pub funding_chain: String,
    pub execution_chain: String,
    pub status: String,
    pub transaction_hash: Option<String>,
    pub executed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

/// Webhook processing response
#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub accepted: bool,
    pub quote_id: Option<Uuid>,
    pub funding_chain: String,
    pub execution_chain: Option<String>,
    pub message: String,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub circuit_breakers: Vec<ChainCircuitBreakerStatus>,
}

/// Per-chain circuit breaker status
#[derive(Debug, Serialize)]
pub struct ChainCircuitBreakerStatus {
    pub chain: String,
    pub active: bool,
    pub reason: Option<String>,
}

/// Treasury balance response
#[derive(Debug, Serialize)]
pub struct TreasuryBalanceResponse {
    pub chain: String,
    pub asset: String,
    pub balance: String,
    pub last_updated: DateTime<Utc>,
}


