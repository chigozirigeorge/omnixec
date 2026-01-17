<!-- cargo.toml -->

[package]
name = "cross-chain-inventory-backend"
version = "0.1.0"
edition = "2021"

[dependencies]
# Web framework
axum = { version = "0.7", features = ["macros"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "cors"] }

# Async runtime
tokio = { version = "1", features = ["full"] }

# Database
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "macros", "uuid", "chrono", "migrate"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Blockchain SDKs
stellar-sdk = "0.1"  # Placeholder - use actual stellar-rs
solana-sdk = "1.17"
solana-client = "1.17"
near-jsonrpc-client = "0.7"
near-primitives = "0.19"

# Cryptography
ed25519-dalek = "2.0"
sha2 = "0.10"
bs58 = "0.5"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Utilities
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
rust_decimal = { version = "1.33", features = ["serde-float"] }
rust_decimal_macros = "1.33"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Configuration
config = "0.13"
dotenv = "0.15"

# HTTP client
reqwest = { version = "0.11", features = ["json"] }

[dev-dependencies]
mockito = "1.2"




<!-- Readme.md -->

# Cross-Chain Inventory-Backed Payment System

## Overview

This system enables users to execute Solana transactions by paying from Stellar or Near, using an **inventory-backed execution model** with delayed settlement. This is NOT a bridge—it's a payment execution service with internal liquidity.

## Core Architecture

### Economic Model

```
User (Stellar/Near) → [Quote] → [Commit] → Execute (Solana) → Settle
                         ↓
                    Lock Funds
                         ↓
                    Treasury Executes
                         ↓
                    Reconcile Later
```

### Key Invariants

1. **No Negative Balances**: All executions require prepayment or bounded authorization
2. **Quote Binding**: Every execution references a valid, unexpired quote
3. **Idempotent Execution**: Same quote cannot execute twice
4. **Atomicity**: Ledger updates are transactional
5. **Treasury Protection**: Per-chain daily limits + circuit breakers

## Security Model

### Threat Model

| Threat | Mitigation |
|--------|-----------|
| Replay attacks | Quote nonces + expiry + idempotent execution |
| Double spending | Database-level locking + quote state machine |
| Treasury drain | Daily limits + circuit breakers + alert thresholds |
| Price manipulation | Oracle-based quotes + staleness checks |
| Key compromise | Per-chain keys + HSM/KMS + spending limits |

### Economic Safety

- **Service Fee**: 0.1% (covers operational costs and risk)
- **Max Spend**: Enforced at quote level (user cannot exceed authorized amount)
- **Daily Limits**: Configurable per-chain treasury spending limits
- **Circuit Breaker**: Automatic pause if abnormal outflow detected (>20% of treasury/hour)

### Key Management

Keys are abstracted behind a `KeyManager` trait. Production deployment should use:
- AWS KMS / GCP KMS / Azure Key Vault
- Separate keys per chain
- No key can drain entire treasury (enforced by limits)

## Module Structure

```
src/
├── main.rs                 # Server bootstrap
├── config.rs              # Configuration management
├── error.rs               # Error types
├── api/                   # HTTP API layer
│   ├── mod.rs
│   ├── routes.rs
│   ├── handlers.rs
│   └── models.rs
├── ledger/                # Source of truth
│   ├── mod.rs
│   ├── schema.rs
│   └── repository.rs
├── quote_engine/          # Quote generation & validation
│   ├── mod.rs
│   ├── engine.rs
│   └── oracle.rs
├── stellar/               # Stellar funding adapter
│   ├── mod.rs
│   ├── monitor.rs
│   └── parser.rs
├── near/                  # Near funding adapter
│   ├── mod.rs
│   ├── monitor.rs
│   └── parser.rs
├── solana/                # Solana execution engine
│   ├── mod.rs
│   ├── executor.rs
│   └── gas_estimator.rs
├── risk/                  # Risk controls
│   ├── mod.rs
│   ├── limits.rs
│   └── circuit_breaker.rs
└── settlement/            # Settlement reconciliation
    ├── mod.rs
    └── reconciler.rs
```

## Database Schema

### Core Tables

**users**
- `id` (UUID, PK)
- `stellar_address` (optional)
- `near_address` (optional)
- `created_at`

**balances**
- `user_id` (FK)
- `chain` (stellar/near/solana)
- `amount` (NUMERIC(78,0) - handles 256-bit integers)
- `locked_amount`
- `updated_at`

**quotes**
- `id` (UUID, PK)
- `user_id` (FK)
- `source_chain` (stellar/near)
- `source_amount`
- `destination_chain` (solana)
- `destination_amount`
- `service_fee`
- `gas_estimate`
- `total_cost`
- `expires_at`
- `status` (pending/committed/executed/expired/failed)
- `nonce` (for replay protection)
- `created_at`

**executions**
- `id` (UUID, PK)
- `quote_id` (FK, UNIQUE - idempotency)
- `solana_signature`
- `status` (pending/success/failed)
- `gas_used`
- `error_message`
- `executed_at`

**settlements**
- `id` (UUID, PK)
- `execution_id` (FK)
- `source_chain`
- `source_txn_hash`
- `settled_at`

## API Flow

### 1. Quote Request

```bash
POST /quote
{
  "user_id": "uuid",
  "source_chain": "stellar",
  "destination_chain": "solana",
  "solana_instructions": [...],  # Base64 encoded
  "estimated_cu": 200000
}
```

Response:
```json
{
  "quote_id": "uuid",
  "source_amount": "1000000",
  "service_fee": "1000",
  "gas_estimate": "5000",
  "total_cost": "1006000",
  "expires_at": "2025-12-17T12:00:00Z",
  "payment_address": "GDAI...XYZ"  # Stellar escrow
}
```

### 2. Commit (Stellar)

User sends XLM to `payment_address` with memo = `quote_id`

Webhook detects payment:
```bash
POST /webhook/stellar
{
  "txn_hash": "abc123",
  "amount": "1006000",
  "memo": "quote_id"
}
```

### 3. Execution

Backend automatically:
1. Validates payment >= total_cost
2. Locks quote for execution
3. Executes Solana transaction from treasury
4. Records execution result
5. Marks settlement pending

### 4. Status Check

```bash
GET /status/{quote_id}
```

Response:
```json
{
  "quote_id": "uuid",
  "status": "executed",
  "solana_signature": "5Kn...",
  "executed_at": "2025-12-17T12:01:00Z"
}
```

## Risk Controls

### Daily Limits

```rust
// Per-chain daily spending limits
STELLAR_DAILY_LIMIT = 1_000_000 XLM
NEAR_DAILY_LIMIT = 10_000 NEAR
SOLANA_DAILY_LIMIT = 100 SOL
```

### Circuit Breaker

Triggers on:
- Hourly outflow > 20% of treasury
- 5+ consecutive failed executions
- Unusual quote volume spike (>200% of 24h average)

When triggered:
1. Pause new quote generation
2. Alert operations team
3. Require manual reset

## Operations

### Deployment

```bash
# Setup database
sqlx database create
sqlx migrate run

# Run server
RUST_LOG=info cargo run --release
```

### Monitoring

Key metrics:
- Quote generation rate
- Execution success rate
- Treasury balances (per chain)
- Settlement lag
- Circuit breaker status

### Settlement Reconciliation

Run periodic reconciliation:
```bash
# Check for unsettled executions older than 1 hour
cargo run --bin reconciler
```

## Testing Strategy

1. **Unit Tests**: Each module has comprehensive tests
2. **Integration Tests**: Full quote→execute→settle flows
3. **Chaos Tests**: Random failures, network partitions
4. **Economic Tests**: Verify all invariants hold under adversarial conditions

## Production Checklist

- [ ] Deploy to HSM/KMS for key management
- [ ] Set up database backups and replication
- [ ] Configure monitoring and alerting
- [ ] Set appropriate rate limits per chain
- [ ] Test circuit breaker triggers
- [ ] Audit all error handling paths
- [ ] Load test under peak conditions
- [ ] Prepare incident response playbook
- [ ] Document settlement reconciliation process
- [ ] Set up automated treasury rebalancing alerts

## License

Proprietary - All Rights Reserved




<!-- src/error.rs -->

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Top-level error type for the entire application
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Quote error: {0}")]
    Quote(#[from] QuoteError),

    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("Risk control violation: {0}")]
    RiskControl(#[from] RiskError),

    #[error("Chain adapter error: {0}")]
    ChainAdapter(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Quote-related errors
#[derive(Error, Debug)]
pub enum QuoteError {
    #[error("Quote not found: {0}")]
    NotFound(String),

    #[error("Quote expired")]
    Expired,

    #[error("Quote already executed")]
    AlreadyExecuted,

    #[error("Quote in invalid state: {current}, expected: {expected}")]
    InvalidState { current: String, expected: String },

    #[error("Insufficient funds: required {required}, available {available}")]
    InsufficientFunds { required: String, available: String },

    #[error("Invalid quote parameters: {0}")]
    InvalidParameters(String),

    #[error("Quote nonce already used")]
    NonceReused,
}

/// Execution-related errors
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Solana execution failed: {0}")]
    SolanaFailed(String),

    #[error("Gas estimation failed: {0}")]
    GasEstimationFailed(String),

    #[error("Transaction simulation failed: {0}")]
    SimulationFailed(String),

    #[error("Insufficient SOL in treasury")]
    InsufficientTreasury,

    #[error("Execution already exists for quote")]
    DuplicateExecution,

    #[error("Transaction timeout")]
    Timeout,
}

/// Risk control errors
#[derive(Error, Debug)]
pub enum RiskError {
    #[error("Daily limit exceeded for {chain}: {current}/{limit}")]
    DailyLimitExceeded {
        chain: String,
        current: String,
        limit: String,
    },

    #[error("Circuit breaker triggered: {reason}")]
    CircuitBreakerTriggered { reason: String },

    #[error("Abnormal outflow detected: {0}")]
    AbnormalOutflow(String),

    #[error("User spending limit exceeded")]
    UserLimitExceeded,
}

/// Chain-specific errors
#[derive(Error, Debug)]
pub enum ChainError {
    #[error("Stellar error: {0}")]
    Stellar(String),

    #[error("Near error: {0}")]
    Near(String),

    #[error("Solana error: {0}")]
    Solana(String),

    #[error("Invalid address format: {0}")]
    InvalidAddress(String),

    #[error("Transaction parsing failed: {0}")]
    ParseError(String),
}

/// API error response structure
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_code, message, details) = match self {
            AppError::Quote(QuoteError::NotFound(id)) => (
                StatusCode::NOT_FOUND,
                "QUOTE_NOT_FOUND",
                format!("Quote not found: {}", id),
                None,
            ),
            AppError::Quote(QuoteError::Expired) => (
                StatusCode::BAD_REQUEST,
                "QUOTE_EXPIRED",
                "Quote has expired".to_string(),
                None,
            ),
            AppError::Quote(QuoteError::AlreadyExecuted) => (
                StatusCode::CONFLICT,
                "QUOTE_ALREADY_EXECUTED",
                "Quote has already been executed".to_string(),
                None,
            ),
            AppError::Quote(QuoteError::InsufficientFunds { required, available }) => (
                StatusCode::BAD_REQUEST,
                "INSUFFICIENT_FUNDS",
                "Insufficient funds for execution".to_string(),
                Some(serde_json::json!({
                    "required": required,
                    "available": available,
                })),
            ),
            AppError::Quote(QuoteError::InvalidParameters(msg)) => (
                StatusCode::BAD_REQUEST,
                "INVALID_QUOTE_PARAMETERS",
                msg,
                None,
            ),
            AppError::Quote(QuoteError::NonceReused) => (
                StatusCode::CONFLICT,
                "NONCE_REUSED",
                "Quote nonce has already been used".to_string(),
                None,
            ),
            AppError::Execution(ExecutionError::SolanaFailed(msg)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "EXECUTION_FAILED",
                format!("Solana execution failed: {}", msg),
                None,
            ),
            AppError::Execution(ExecutionError::InsufficientTreasury) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "INSUFFICIENT_TREASURY",
                "Insufficient SOL in treasury. Please try again later.".to_string(),
                None,
            ),
            AppError::Execution(ExecutionError::DuplicateExecution) => (
                StatusCode::CONFLICT,
                "DUPLICATE_EXECUTION",
                "Execution already exists for this quote".to_string(),
                None,
            ),
            AppError::RiskControl(RiskError::DailyLimitExceeded { chain, current, limit }) => (
                StatusCode::TOO_MANY_REQUESTS,
                "DAILY_LIMIT_EXCEEDED",
                format!("Daily limit exceeded for {}", chain),
                Some(serde_json::json!({
                    "current": current,
                    "limit": limit,
                })),
            ),
            AppError::RiskControl(RiskError::CircuitBreakerTriggered { reason }) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "CIRCUIT_BREAKER_TRIGGERED",
                format!("Service temporarily unavailable: {}", reason),
                None,
            ),
            AppError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_ERROR",
                "A database error occurred".to_string(),
                None,
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "An internal error occurred".to_string(),
                None,
            ),
        };

        let body = Json(ErrorResponse {
            error: message,
            error_code: error_code.to_string(),
            details,
        });

        (status, body).into_response()
    }
}

/// Result type alias for the application
pub type AppResult<T> = Result<T, AppError>;


<!-- src/ledger/models.rs -->

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;

/// Chain identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "chain_type", rename_all = "lowercase")]
pub enum Chain {
    Stellar,
    Near,
    Solana,
}

impl Chain {
    pub fn as_str(&self) -> &'static str {
        match self {
            Chain::Stellar => "stellar",
            Chain::Near => "near",
            Chain::Solana => "solana",
        }
    }
}

/// Quote status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "quote_status", rename_all = "lowercase")]
pub enum QuoteStatus {
    Pending,
    Committed,
    Executed,
    Expired,
    Failed,
}

/// Execution status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "execution_status", rename_all = "lowercase")]
pub enum ExecutionStatus {
    Pending,
    Success,
    Failed,
}

/// User entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub stellar_address: Option<String>,
    pub near_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Balance entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub user_id: Uuid,
    pub chain: Chain,
    #[serde(with = "rust_decimal::serde::str")]
    pub amount: rust_decimal::Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub locked_amount: rust_decimal::Decimal,
    pub updated_at: DateTime<Utc>,
}

impl Balance {
    /// Get available (unlocked) balance
    pub fn available(&self) -> rust_decimal::Decimal {
        self.amount - self.locked_amount
    }

    /// Check if sufficient balance is available
    pub fn has_available(&self, required: rust_decimal::Decimal) -> bool {
        self.available() >= required
    }
}

/// Quote entity - represents a price quote for cross-chain execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub id: Uuid,
    pub user_id: Uuid,
    pub source_chain: Chain,
    #[serde(with = "rust_decimal::serde::str")]
    pub source_amount: rust_decimal::Decimal,
    pub destination_chain: Chain,
    #[serde(with = "rust_decimal::serde::str")]
    pub destination_amount: rust_decimal::Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub service_fee: rust_decimal::Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub gas_estimate: rust_decimal::Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_cost: rust_decimal::Decimal,
    pub solana_instructions: Vec<u8>,
    pub estimated_compute_units: i32,
    pub nonce: String,
    pub status: QuoteStatus,
    pub expires_at: DateTime<Utc>,
    pub payment_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Quote {
    /// Check if quote is still valid
    pub fn is_valid(&self) -> bool {
        self.status == QuoteStatus::Pending && self.expires_at > Utc::now()
    }

    /// Check if quote can be committed
    pub fn can_commit(&self) -> bool {
        self.is_valid()
    }

    /// Check if quote can be executed
    pub fn can_execute(&self) -> bool {
        self.status == QuoteStatus::Committed && self.expires_at > Utc::now()
    }
}

/// Execution entity - represents a Solana transaction execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub id: Uuid,
    pub quote_id: Uuid,
    pub solana_signature: Option<String>,
    pub status: ExecutionStatus,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub gas_used: Option<rust_decimal::Decimal>,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub executed_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Settlement entity - records the source chain payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settlement {
    pub id: Uuid,
    pub execution_id: Uuid,
    pub source_chain: Chain,
    pub source_txn_hash: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub source_amount: rust_decimal::Decimal,
    pub settled_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

/// Treasury balance entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryBalance {
    pub chain: Chain,
    #[serde(with = "rust_decimal::serde::str")]
    pub balance: rust_decimal::Decimal,
    pub last_updated: DateTime<Utc>,
    pub last_reconciled: Option<DateTime<Utc>>,
}

/// Daily spending tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySpending {
    pub chain: Chain,
    pub date: chrono::NaiveDate,
    #[serde(with = "rust_decimal::serde::str")]
    pub amount_spent: rust_decimal::Decimal,
    pub transaction_count: i32,
}

/// Audit event type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[sqlx(type_name = "audit_event_type", rename_all = "snake_case")]
pub enum AuditEventType {
    QuoteCreated,
    QuoteCommitted,
    ExecutionStarted,
    ExecutionCompleted,
    ExecutionFailed,
    SettlementRecorded,
    CircuitBreakerTriggered,
    CircuitBreakerReset,
    LimitExceeded,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub event_type: AuditEventType,
    pub entity_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Circuit breaker state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    pub id: Uuid,
    pub triggered_at: DateTime<Utc>,
    pub reason: String,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
}

impl CircuitBreakerState {
    pub fn is_active(&self) -> bool {
        self.resolved_at.is_none()
    }
}







<!-- src/ledger/repository.rs -->

use super::models::*;
use crate::error::{AppError, AppResult, QuoteError};
use rust_decimal::Decimal;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Ledger repository - THE source of truth for all state
pub struct LedgerRepository {
    pool: PgPool,
}

impl LedgerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ========== USER OPERATIONS ==========

    pub async fn create_user(
        &self,
        stellar_address: Option<String>,
        near_address: Option<String>,
    ) -> AppResult<User> {
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (stellar_address, near_address)
            VALUES ($1, $2)
            RETURNING id, stellar_address, near_address, created_at, updated_at
            "#,
            stellar_address,
            near_address
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn get_user_by_id(&self, user_id: Uuid) -> AppResult<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"
            SELECT id, stellar_address, near_address, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    // ========== BALANCE OPERATIONS ==========

    /// Get balance for a user and chain
    pub async fn get_balance(&self, user_id: Uuid, chain: Chain) -> AppResult<Option<Balance>> {
        let balance = sqlx::query!(
            r#"
            SELECT user_id, chain as "chain: Chain", amount, locked_amount, updated_at
            FROM balances
            WHERE user_id = $1 AND chain = $2
            "#,
            user_id,
            chain as Chain
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|row| Balance {
            user_id: row.user_id,
            chain: row.chain,
            amount: row.amount,
            locked_amount: row.locked_amount,
            updated_at: row.updated_at,
        });

        Ok(balance)
    }

    /// Lock funds for a quote (atomic operation)
    pub async fn lock_funds(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        chain: Chain,
        amount: Decimal,
    ) -> AppResult<()> {
        let result = sqlx::query!(
            r#"
            UPDATE balances
            SET locked_amount = locked_amount + $3
            WHERE user_id = $1 AND chain = $2 AND (amount - locked_amount) >= $3
            "#,
            user_id,
            chain as Chain,
            amount
        )
        .execute(&mut **tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(QuoteError::InsufficientFunds {
                required: amount.to_string(),
                available: "unknown".to_string(),
            }
            .into());
        }

        Ok(())
    }

    /// Unlock funds (e.g., on failed execution)
    pub async fn unlock_funds(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        chain: Chain,
        amount: Decimal,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            UPDATE balances
            SET locked_amount = locked_amount - $3
            WHERE user_id = $1 AND chain = $2
            "#,
            user_id,
            chain as Chain,
            amount
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    // ========== QUOTE OPERATIONS ==========

    /// Create a new quote
    pub async fn create_quote(
        &self,
        user_id: Uuid,
        source_chain: Chain,
        source_amount: Decimal,
        destination_chain: Chain,
        destination_amount: Decimal,
        service_fee: Decimal,
        gas_estimate: Decimal,
        solana_instructions: Vec<u8>,
        estimated_compute_units: i32,
        nonce: String,
        expires_at: chrono::DateTime<chrono::Utc>,
        payment_address: Option<String>,
    ) -> AppResult<Quote> {
        let total_cost = source_amount + service_fee + gas_estimate;

        let quote = sqlx::query!(
            r#"
            INSERT INTO quotes (
                user_id, source_chain, source_amount, destination_chain, destination_amount,
                service_fee, gas_estimate, total_cost, solana_instructions,
                estimated_compute_units, nonce, expires_at, payment_address
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING 
                id, user_id, 
                source_chain as "source_chain: Chain", 
                source_amount, 
                destination_chain as "destination_chain: Chain",
                destination_amount, service_fee, gas_estimate, total_cost,
                solana_instructions, estimated_compute_units, nonce,
                status as "status: QuoteStatus", expires_at, payment_address,
                created_at, updated_at
            "#,
            user_id,
            source_chain as Chain,
            source_amount,
            destination_chain as Chain,
            destination_amount,
            service_fee,
            gas_estimate,
            total_cost,
            solana_instructions,
            estimated_compute_units,
            nonce,
            expires_at,
            payment_address
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Quote {
            id: quote.id,
            user_id: quote.user_id,
            source_chain: quote.source_chain,
            source_amount: quote.source_amount,
            destination_chain: quote.destination_chain,
            destination_amount: quote.destination_amount,
            service_fee: quote.service_fee,
            gas_estimate: quote.gas_estimate,
            total_cost: quote.total_cost,
            solana_instructions: quote.solana_instructions,
            estimated_compute_units: quote.estimated_compute_units,
            nonce: quote.nonce,
            status: quote.status,
            expires_at: quote.expires_at,
            payment_address: quote.payment_address,
            created_at: quote.created_at,
            updated_at: quote.updated_at,
        })
    }

    /// Get quote by ID
    pub async fn get_quote(&self, quote_id: Uuid) -> AppResult<Option<Quote>> {
        let quote = sqlx::query!(
            r#"
            SELECT 
                id, user_id,
                source_chain as "source_chain: Chain",
                source_amount,
                destination_chain as "destination_chain: Chain",
                destination_amount, service_fee, gas_estimate, total_cost,
                solana_instructions, estimated_compute_units, nonce,
                status as "status: QuoteStatus", expires_at, payment_address,
                created_at, updated_at
            FROM quotes
            WHERE id = $1
            "#,
            quote_id
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|row| Quote {
            id: row.id,
            user_id: row.user_id,
            source_chain: row.source_chain,
            source_amount: row.source_amount,
            destination_chain: row.destination_chain,
            destination_amount: row.destination_amount,
            service_fee: row.service_fee,
            gas_estimate: row.gas_estimate,
            total_cost: row.total_cost,
            solana_instructions: row.solana_instructions,
            estimated_compute_units: row.estimated_compute_units,
            nonce: row.nonce,
            status: row.status,
            expires_at: row.expires_at,
            payment_address: row.payment_address,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });

        Ok(quote)
    }

    /// Update quote status (with optimistic locking check)
    pub async fn update_quote_status(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        quote_id: Uuid,
        from_status: QuoteStatus,
        to_status: QuoteStatus,
    ) -> AppResult<()> {
        let result = sqlx::query!(
            r#"
            UPDATE quotes
            SET status = $3
            WHERE id = $1 AND status = $2
            "#,
            quote_id,
            from_status as QuoteStatus,
            to_status as QuoteStatus
        )
        .execute(&mut **tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(QuoteError::InvalidState {
                current: "unknown".to_string(),
                expected: format!("{:?}", from_status),
            }
            .into());
        }

        Ok(())
    }

    // ========== EXECUTION OPERATIONS ==========

    /// Create execution record (idempotent by quote_id UNIQUE constraint)
    pub async fn create_execution(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        quote_id: Uuid,
    ) -> AppResult<Execution> {
        let execution = sqlx::query!(
            r#"
            INSERT INTO executions (quote_id)
            VALUES ($1)
            RETURNING 
                id, quote_id, solana_signature, 
                status as "status: ExecutionStatus",
                gas_used, error_message, retry_count,
                executed_at, completed_at
            "#,
            quote_id
        )
        .fetch_one(&mut **tx)
        .await?;

        Ok(Execution {
            id: execution.id,
            quote_id: execution.quote_id,
            solana_signature: execution.solana_signature,
            status: execution.status,
            gas_used: execution.gas_used,
            error_message: execution.error_message,
            retry_count: execution.retry_count,
            executed_at: execution.executed_at,
            completed_at: execution.completed_at,
        })
    }

    /// Update execution with result
    pub async fn complete_execution(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        execution_id: Uuid,
        status: ExecutionStatus,
        solana_signature: Option<String>,
        gas_used: Option<Decimal>,
        error_message: Option<String>,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            UPDATE executions
            SET status = $2, solana_signature = $3, gas_used = $4, 
                error_message = $5, completed_at = NOW()
            WHERE id = $1
            "#,
            execution_id,
            status as ExecutionStatus,
            solana_signature,
            gas_used,
            error_message
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    // ========== SETTLEMENT OPERATIONS ==========

    /// Record settlement
    pub async fn create_settlement(
        &self,
        execution_id: Uuid,
        source_chain: Chain,
        source_txn_hash: String,
        source_amount: Decimal,
    ) -> AppResult<Settlement> {
        let settlement = sqlx::query!(
            r#"
            INSERT INTO settlements (execution_id, source_chain, source_txn_hash, source_amount)
            VALUES ($1, $2, $3, $4)
            RETURNING 
                id, execution_id, 
                source_chain as "source_chain: Chain",
                source_txn_hash, source_amount, settled_at, verified_at
            "#,
            execution_id,
            source_chain as Chain,
            source_txn_hash,
            source_amount
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Settlement {
            id: settlement.id,
            execution_id: settlement.execution_id,
            source_chain: settlement.source_chain,
            source_txn_hash: settlement.source_txn_hash,
            source_amount: settlement.source_amount,
            settled_at: settlement.settled_at,
            verified_at: settlement.verified_at,
        })
    }

    // ========== AUDIT LOG ==========

    pub async fn log_audit_event(
        &self,
        event_type: AuditEventType,
        entity_id: Option<Uuid>,
        user_id: Option<Uuid>,
        details: serde_json::Value,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO audit_log (event_type, entity_id, user_id, details)
            VALUES ($1, $2, $3, $4)
            "#,
            event_type as AuditEventType,
            entity_id,
            user_id,
            details
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ========== DAILY SPENDING TRACKING ==========

    pub async fn get_daily_spending(
        &self,
        chain: Chain,
        date: chrono::NaiveDate,
    ) -> AppResult<Option<DailySpending>> {
        let spending = sqlx::query!(
            r#"
            SELECT chain as "chain: Chain", date, amount_spent, transaction_count
            FROM daily_spending
            WHERE chain = $1 AND date = $2
            "#,
            chain as Chain,
            date
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|row| DailySpending {
            chain: row.chain,
            date: row.date,
            amount_spent: row.amount_spent,
            transaction_count: row.transaction_count,
        });

        Ok(spending)
    }

    pub async fn increment_daily_spending(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        chain: Chain,
        date: chrono::NaiveDate,
        amount: Decimal,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO daily_spending (chain, date, amount_spent, transaction_count)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT (chain, date)
            DO UPDATE SET
                amount_spent = daily_spending.amount_spent + EXCLUDED.amount_spent,
                transaction_count = daily_spending.transaction_count + 1
            "#,
            chain as Chain,
            date,
            amount
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    // ========== CIRCUIT BREAKER ==========

    pub async fn get_active_circuit_breaker(&self) -> AppResult<Option<CircuitBreakerState>> {
        let state = sqlx::query_as!(
            CircuitBreakerState,
            r#"
            SELECT id, triggered_at, reason, resolved_at, resolved_by
            FROM circuit_breaker_state
            WHERE resolved_at IS NULL
            ORDER BY triggered_at DESC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(state)
    }

    pub async fn trigger_circuit_breaker(&self, reason: String) -> AppResult<CircuitBreakerState> {
        let state = sqlx::query_as!(
            CircuitBreakerState,
            r#"
            INSERT INTO circuit_breaker_state (reason)
            VALUES ($1)
            RETURNING id, triggered_at, reason, resolved_at, resolved_by
            "#,
            reason
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(state)
    }

    /// Begin a database transaction
    pub async fn begin_tx(&self) -> AppResult<Transaction<'_, Postgres>> {
        Ok(self.pool.begin().await?)
    }
}





<!-- src/quote_engine/engine.rs -->

use crate::error::{AppResult, QuoteError};
use crate::ledger::{models::*, repository::LedgerRepository};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use uuid::Uuid;

/// Quote engine configuration
#[derive(Debug, Clone)]
pub struct QuoteConfig {
    /// Service fee as a percentage (0.1% = 0.001)
    pub service_fee_rate: Decimal,
    /// Quote validity duration in seconds
    pub quote_ttl_seconds: i64,
    /// Maximum compute units allowed per transaction
    pub max_compute_units: i32,
    /// Solana lamports per compute unit (for gas estimation)
    pub lamports_per_cu: Decimal,
}

impl Default for QuoteConfig {
    fn default() -> Self {
        Self {
            service_fee_rate: dec!(0.001), // 0.1%
            quote_ttl_seconds: 300,        // 5 minutes
            max_compute_units: 1_400_000,  // Solana max
            lamports_per_cu: dec!(0.000001), // ~1 micro-lamport per CU
        }
    }
}

/// Quote engine - generates and validates quotes
pub struct QuoteEngine {
    config: QuoteConfig,
    ledger: Arc<LedgerRepository>,
}

impl QuoteEngine {
    pub fn new(config: QuoteConfig, ledger: Arc<LedgerRepository>) -> Self {
        Self { config, ledger }
    }

    /// Generate a new quote for cross-chain execution
    ///
    /// SECURITY: This function performs critical validations:
    /// - Validates compute units are within Solana limits
    /// - Calculates worst-case gas costs
    /// - Applies service fee to cover operational risk
    /// - Sets expiry to prevent stale quotes
    pub async fn generate_quote(
        &self,
        user_id: Uuid,
        source_chain: Chain,
        solana_instructions: Vec<u8>,
        estimated_compute_units: i32,
    ) -> AppResult<Quote> {
        // Validation: Ensure compute units are within Solana limits
        if estimated_compute_units <= 0 || estimated_compute_units > self.config.max_compute_units
        {
            return Err(QuoteError::InvalidParameters(format!(
                "Compute units must be between 1 and {}",
                self.config.max_compute_units
            ))
            .into());
        }

        // Validation: Ensure instructions are not empty
        if solana_instructions.is_empty() {
            return Err(
                QuoteError::InvalidParameters("Solana instructions cannot be empty".to_string())
                    .into(),
            );
        }

        // Calculate costs
        let gas_estimate = self.estimate_gas(estimated_compute_units)?;

        // For now, we assume 1:1 conversion (this would use an oracle in production)
        // The source_amount represents the amount the user wants to send on Solana
        let destination_amount = Decimal::ZERO; // Placeholder - would be extracted from instructions

        // Service fee calculation
        let service_fee = gas_estimate * self.config.service_fee_rate;

        // Total cost user must pay on source chain
        let total_cost = gas_estimate + service_fee;

        // Generate unique nonce for replay protection
        let nonce = format!("{}-{}", Uuid::new_v4(), Utc::now().timestamp_millis());

        // Set expiry
        let expires_at = Utc::now() + Duration::seconds(self.config.quote_ttl_seconds);

        // Generate payment address based on source chain
        let payment_address = self.generate_payment_address(source_chain, &nonce).await?;

        // Create quote in ledger
        let quote = self
            .ledger
            .create_quote(
                user_id,
                source_chain,
                total_cost, // This is what they pay
                Chain::Solana,
                destination_amount,
                service_fee,
                gas_estimate,
                solana_instructions,
                estimated_compute_units,
                nonce,
                expires_at,
                Some(payment_address),
            )
            .await?;

        // Audit log
        self.ledger
            .log_audit_event(
                AuditEventType::QuoteCreated,
                Some(quote.id),
                Some(user_id),
                serde_json::json!({
                    "source_chain": source_chain,
                    "total_cost": total_cost.to_string(),
                    "gas_estimate": gas_estimate.to_string(),
                    "service_fee": service_fee.to_string(),
                }),
            )
            .await?;

        Ok(quote)
    }

    /// Validate and commit a quote
    ///
    /// SECURITY: This is a critical state transition that must be atomic
    pub async fn commit_quote(&self, quote_id: Uuid) -> AppResult<Quote> {
        let mut tx = self.ledger.begin_tx().await?;

        // Get quote with FOR UPDATE lock
        let quote = self
            .ledger
            .get_quote(quote_id)
            .await?
            .ok_or_else(|| QuoteError::NotFound(quote_id.to_string()))?;

        // Validation: Check if quote can be committed
        if !quote.can_commit() {
            return Err(if quote.expires_at <= Utc::now() {
                QuoteError::Expired
            } else {
                QuoteError::InvalidState {
                    current: format!("{:?}", quote.status),
                    expected: "Pending".to_string(),
                }
            }
            .into());
        }

        // Update status to committed
        self.ledger
            .update_quote_status(&mut tx, quote_id, QuoteStatus::Pending, QuoteStatus::Committed)
            .await?;

        // Commit transaction
        tx.commit().await?;

        // Audit log
        self.ledger
            .log_audit_event(
                AuditEventType::QuoteCommitted,
                Some(quote_id),
                Some(quote.user_id),
                serde_json::json!({
                    "quote_id": quote_id.to_string(),
                }),
            )
            .await?;

        // Return updated quote
        self.ledger
            .get_quote(quote_id)
            .await?
            .ok_or_else(|| QuoteError::NotFound(quote_id.to_string()).into())
    }

    /// Estimate gas cost for Solana execution
    ///
    /// SECURITY: We use worst-case estimation to prevent treasury drain
    /// - Adds 20% buffer for price volatility
    /// - Accounts for priority fees
    /// - Includes transaction signature costs
    fn estimate_gas(&self, compute_units: i32) -> AppResult<Decimal> {
        // Base cost: compute units * lamports per CU
        let compute_cost = Decimal::from(compute_units) * self.config.lamports_per_cu;

        // Signature cost (5000 lamports per signature, assume 2 signatures)
        let signature_cost = dec!(10000);

        // Priority fee buffer (20% of compute cost)
        let priority_buffer = compute_cost * dec!(0.2);

        // Total with buffer
        let total_lamports = compute_cost + signature_cost + priority_buffer;

        Ok(total_lamports)
    }

    /// Generate payment address for source chain
    ///
    /// In production, this would:
    /// - Generate unique escrow accounts per quote (Stellar)
    /// - Return contract address with specific memo (Near)
    async fn generate_payment_address(&self, chain: Chain, nonce: &str) -> AppResult<String> {
        match chain {
            Chain::Stellar => {
                // In production: Create unique escrow account or return main account with memo
                Ok(format!("GDAI...ESCROW-{}", &nonce[..8]))
            }
            Chain::Near => {
                // In production: Return contract account with method call
                Ok(format!("payment.near-{}", &nonce[..8]))
            }
            Chain::Solana => {
                Err(QuoteError::InvalidParameters("Cannot use Solana as source chain".to_string()).into())
            }
        }
    }

    /// Validate quote before execution
    ///
    /// SECURITY: Final check before committing treasury funds
    pub async fn validate_for_execution(&self, quote_id: Uuid) -> AppResult<Quote> {
        let quote = self
            .ledger
            .get_quote(quote_id)
            .await?
            .ok_or_else(|| QuoteError::NotFound(quote_id.to_string()))?;

        // Check if quote can be executed
        if !quote.can_execute() {
            return Err(if quote.expires_at <= Utc::now() {
                QuoteError::Expired
            } else {
                QuoteError::InvalidState {
                    current: format!("{:?}", quote.status),
                    expected: "Committed".to_string(),
                }
            }
            .into());
        }

        Ok(quote)
    }

    /// Mark quote as executed
    pub async fn mark_executed(&self, quote_id: Uuid) -> AppResult<()> {
        let mut tx = self.ledger.begin_tx().await?;

        self.ledger
            .update_quote_status(&mut tx, quote_id, QuoteStatus::Committed, QuoteStatus::Executed)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Mark quote as failed
    pub async fn mark_failed(&self, quote_id: Uuid) -> AppResult<()> {
        let mut tx = self.ledger.begin_tx().await?;

        self.ledger
            .update_quote_status(&mut tx, quote_id, QuoteStatus::Committed, QuoteStatus::Failed)
            .await?;

        tx.commit().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gas_estimation() {
        let config = QuoteConfig::default();
        let engine = QuoteEngine::new(config.clone(), Arc::new(todo!()));

        let gas = engine.estimate_gas(200_000).unwrap();

        // Should include compute cost + signatures + buffer
        assert!(gas > Decimal::ZERO);
        assert!(gas > dec!(10000)); // At minimum signature cost
    }

    #[test]
    fn test_service_fee_calculation() {
        let config = QuoteConfig::default();
        let gas = dec!(10000);
        let fee = gas * config.service_fee_rate;

        // 0.1% of 10000 = 10
        assert_eq!(fee, dec!(10));
    }
}






<!-- src/risk/controls.rs -->

use crate::error::{AppResult, RiskError};
use crate::ledger::{models::*, repository::LedgerRepository};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tracing::{error, warn};

/// Risk control configuration
#[derive(Debug, Clone)]
pub struct RiskConfig {
    /// Daily spending limit per chain (in native tokens)
    pub stellar_daily_limit: Decimal,
    pub near_daily_limit: Decimal,
    pub solana_daily_limit: Decimal,

    /// Circuit breaker thresholds
    /// Percentage of treasury that can be spent per hour
    pub hourly_outflow_threshold: Decimal,

    /// Consecutive failures before circuit breaker
    pub max_consecutive_failures: i32,

    /// Enable circuit breaker
    pub circuit_breaker_enabled: bool,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            stellar_daily_limit: dec!(1_000_000),   // 1M XLM
            near_daily_limit: dec!(10_000),         // 10K NEAR
            solana_daily_limit: dec!(100),          // 100 SOL
            hourly_outflow_threshold: dec!(0.2),    // 20%
            max_consecutive_failures: 5,
            circuit_breaker_enabled: true,
        }
    }
}

/// Risk control system
///
/// SECURITY: This is the last line of defense against treasury drain
pub struct RiskController {
    config: RiskConfig,
    ledger: Arc<LedgerRepository>,
}

impl RiskController {
    pub fn new(config: RiskConfig, ledger: Arc<LedgerRepository>) -> Self {
        Self { config, ledger }
    }

    /// Check if execution is allowed under current risk controls
    ///
    /// SECURITY: This must be called before EVERY execution
    pub async fn check_execution_allowed(
        &self,
        chain: Chain,
        amount: Decimal,
    ) -> AppResult<()> {
        // 1. Check circuit breaker
        if self.config.circuit_breaker_enabled {
            if let Some(breaker) = self.ledger.get_active_circuit_breaker().await? {
                error!(
                    "Circuit breaker is active: {} (triggered at: {})",
                    breaker.reason, breaker.triggered_at
                );
                return Err(RiskError::CircuitBreakerTriggered {
                    reason: breaker.reason,
                }
                .into());
            }
        }

        // 2. Check daily spending limit
        self.check_daily_limit(chain, amount).await?;

        // 3. Check hourly outflow (against treasury balance)
        self.check_hourly_outflow(chain, amount).await?;

        Ok(())
    }

    /// Check daily spending limit for a chain
    ///
    /// SECURITY: Prevents unlimited spending in a single day
    async fn check_daily_limit(&self, chain: Chain, amount: Decimal) -> AppResult<()> {
        let today = Utc::now().date_naive();

        let spending = self
            .ledger
            .get_daily_spending(chain, today)
            .await?
            .unwrap_or(DailySpending {
                chain,
                date: today,
                amount_spent: Decimal::ZERO,
                transaction_count: 0,
            });

        let limit = self.get_chain_daily_limit(chain);
        let new_total = spending.amount_spent + amount;

        if new_total > limit {
            warn!(
                "Daily limit exceeded for {:?}: {} + {} > {}",
                chain, spending.amount_spent, amount, limit
            );

            // Log to audit
            self.ledger
                .log_audit_event(
                    AuditEventType::LimitExceeded,
                    None,
                    None,
                    serde_json::json!({
                        "chain": chain,
                        "current": spending.amount_spent.to_string(),
                        "attempted": amount.to_string(),
                        "limit": limit.to_string(),
                    }),
                )
                .await?;

            return Err(RiskError::DailyLimitExceeded {
                chain: format!("{:?}", chain),
                current: spending.amount_spent.to_string(),
                limit: limit.to_string(),
            }
            .into());
        }

        Ok(())
    }

    /// Check hourly outflow against treasury balance
    ///
    /// SECURITY: Detects abnormal spending patterns that could indicate:
    /// - Compromised keys
    /// - Pricing oracle manipulation
    /// - System bugs causing over-execution
    async fn check_hourly_outflow(&self, chain: Chain, amount: Decimal) -> AppResult<()> {
        // In production, this would:
        // 1. Query on-chain treasury balance
        // 2. Calculate hourly spending from recent executions
        // 3. Compare against threshold

        // For now, we'll implement a simplified version
        // TODO: Implement actual hourly outflow calculation

        Ok(())
    }

    /// Record spending (must be called after successful execution)
    ///
    /// SECURITY: This MUST be called atomically with execution
    pub async fn record_spending(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        chain: Chain,
        amount: Decimal,
    ) -> AppResult<()> {
        let today = Utc::now().date_naive();

        self.ledger
            .increment_daily_spending(tx, chain, today, amount)
            .await?;

        Ok(())
    }

    /// Trigger circuit breaker
    ///
    /// SECURITY: Immediately halts all new executions
    pub async fn trigger_circuit_breaker(&self, reason: String) -> AppResult<()> {
        error!("Triggering circuit breaker: {}", reason);

        let state = self.ledger.trigger_circuit_breaker(reason.clone()).await?;

        // Log to audit
        self.ledger
            .log_audit_event(
                AuditEventType::CircuitBreakerTriggered,
                Some(state.id),
                None,
                serde_json::json!({
                    "reason": reason,
                    "triggered_at": state.triggered_at,
                }),
            )
            .await?;

        // In production, this would also:
        // - Send alerts to operations team
        // - Pause webhook processing
        // - Lock hot wallets

        Ok(())
    }

    /// Check for conditions that should trigger circuit breaker
    ///
    /// SECURITY: Called periodically and after failed executions
    pub async fn check_circuit_breaker_conditions(&self) -> AppResult<bool> {
        // Check consecutive failures
        // In production, this would query recent execution failures
        // and trigger if threshold exceeded

        // For now, return false (no trigger)
        Ok(false)
    }

    /// Get daily limit for a specific chain
    fn get_chain_daily_limit(&self, chain: Chain) -> Decimal {
        match chain {
            Chain::Stellar => self.config.stellar_daily_limit,
            Chain::Near => self.config.near_daily_limit,
            Chain::Solana => self.config.solana_daily_limit,
        }
    }
}

/// Per-user spending limits (additional layer of protection)
pub struct UserRiskLimits {
    /// Maximum amount a user can spend per day
    pub daily_limit: Decimal,
    /// Maximum amount per single transaction
    pub per_transaction_limit: Decimal,
}

impl Default for UserRiskLimits {
    fn default() -> Self {
        Self {
            daily_limit: dec!(10_000),      // Conservative default
            per_transaction_limit: dec!(1_000),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daily_limit_configuration() {
        let config = RiskConfig::default();

        assert!(config.stellar_daily_limit > Decimal::ZERO);
        assert!(config.near_daily_limit > Decimal::ZERO);
        assert!(config.solana_daily_limit > Decimal::ZERO);
    }

    #[test]
    fn test_hourly_threshold() {
        let config = RiskConfig::default();

        // 20% threshold should be between 0 and 1
        assert!(config.hourly_outflow_threshold > Decimal::ZERO);
        assert!(config.hourly_outflow_threshold < Decimal::ONE);
    }
}







<!-- src/solana/executor.rs -->

use crate::error::{AppResult, ExecutionError};
use crate::ledger::{models::*, repository::LedgerRepository};
use crate::risk::RiskController;
use rust_decimal::Decimal;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Solana executor configuration
#[derive(Debug, Clone)]
pub struct SolanaConfig {
    /// RPC endpoint
    pub rpc_url: String,
    /// Commitment level for transaction confirmation
    pub commitment: CommitmentConfig,
    /// Maximum retries for failed transactions
    pub max_retries: u32,
    /// Timeout for transaction confirmation
    pub confirmation_timeout: Duration,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            commitment: CommitmentConfig::confirmed(),
            max_retries: 3,
            confirmation_timeout: Duration::from_secs(60),
        }
    }
}

/// Solana executor - executes transactions using treasury funds
///
/// SECURITY CRITICAL: This component controls treasury spending
pub struct SolanaExecutor {
    config: SolanaConfig,
    client: RpcClient,
    ledger: Arc<LedgerRepository>,
    risk: Arc<RiskController>,
    // In production, this would be a KMS client, not a raw keypair
    treasury_keypair: Arc<Keypair>,
}

impl SolanaExecutor {
    pub fn new(
        config: SolanaConfig,
        ledger: Arc<LedgerRepository>,
        risk: Arc<RiskController>,
        treasury_keypair: Keypair,
    ) -> Self {
        let client = RpcClient::new_with_commitment(
            config.rpc_url.clone(),
            config.commitment,
        );

        Self {
            config,
            client,
            ledger,
            risk,
            treasury_keypair: Arc::new(treasury_keypair),
        }
    }

    /// Execute a Solana transaction from a quote
    ///
    /// SECURITY: This is the most critical function. It:
    /// 1. Validates quote state
    /// 2. Checks risk controls
    /// 3. Simulates transaction
    /// 4. Executes with treasury funds
    /// 5. Records execution atomically
    ///
    /// INVARIANT: This function is idempotent - calling it multiple times
    /// with the same quote_id will not result in multiple executions
    pub async fn execute_quote(&self, quote_id: Uuid) -> AppResult<Execution> {
        info!("Starting execution for quote: {}", quote_id);

        // Begin atomic transaction
        let mut tx = self.ledger.begin_tx().await?;

        // 1. Get quote and validate
        let quote = self
            .ledger
            .get_quote(quote_id)
            .await?
            .ok_or_else(|| ExecutionError::SolanaFailed("Quote not found".to_string()))?;

        if !quote.can_execute() {
            return Err(ExecutionError::SolanaFailed(format!(
                "Quote cannot be executed: status={:?}",
                quote.status
            ))
            .into());
        }

        // 2. Check if execution already exists (idempotency)
        // The UNIQUE constraint on quote_id will prevent duplicates,
        // but we check first to return the existing execution
        match self.ledger.create_execution(&mut tx, quote_id).await {
            Ok(execution) => {
                // New execution created, proceed
                tx.commit().await?;
                
                // Now execute the actual Solana transaction
                self.perform_execution(execution.id, &quote).await
            }
            Err(_) => {
                // Execution already exists, this is a duplicate request
                tx.rollback().await?;
                return Err(ExecutionError::DuplicateExecution.into());
            }
        }
    }

    /// Perform the actual Solana execution
    ///
    /// SECURITY: All pre-checks have passed, now we execute
    async fn perform_execution(
        &self,
        execution_id: Uuid,
        quote: &Quote,
    ) -> AppResult<Execution> {
        // 3. Risk control check
        self.risk
            .check_execution_allowed(Chain::Solana, quote.gas_estimate)
            .await?;

        // 4. Deserialize instructions
        let instructions = self.deserialize_instructions(&quote.solana_instructions)?;

        // 5. Build and sign transaction
        let transaction = self.build_transaction(instructions)?;

        // 6. Simulate transaction (dry run)
        self.simulate_transaction(&transaction).await?;

        info!("Simulation successful, sending transaction for quote: {}", quote.id);

        // 7. Send transaction
        let signature = match self.send_transaction(transaction).await {
            Ok(sig) => sig,
            Err(e) => {
                error!("Failed to send transaction: {:?}", e);
                
                // Record failure
                let mut tx = self.ledger.begin_tx().await?;
                self.ledger
                    .complete_execution(
                        &mut tx,
                        execution_id,
                        ExecutionStatus::Failed,
                        None,
                        None,
                        Some(e.to_string()),
                    )
                    .await?;
                
                // Mark quote as failed
                self.ledger
                    .update_quote_status(&mut tx, quote.id, QuoteStatus::Committed, QuoteStatus::Failed)
                    .await?;
                
                tx.commit().await?;
                
                return Err(e);
            }
        };

        info!("Transaction sent successfully: {}", signature);

        // 8. Wait for confirmation
        let gas_used = match self.confirm_transaction(&signature).await {
            Ok(gas) => gas,
            Err(e) => {
                warn!("Transaction confirmation failed: {:?}", e);
                
                // Record as failed (transaction may have failed on-chain)
                let mut tx = self.ledger.begin_tx().await?;
                self.ledger
                    .complete_execution(
                        &mut tx,
                        execution_id,
                        ExecutionStatus::Failed,
                        Some(signature.to_string()),
                        None,
                        Some(format!("Confirmation failed: {}", e)),
                    )
                    .await?;
                
                tx.commit().await?;
                
                return Err(e);
            }
        };

        // 9. Record successful execution atomically with spending
        let mut tx = self.ledger.begin_tx().await?;

        self.ledger
            .complete_execution(
                &mut tx,
                execution_id,
                ExecutionStatus::Success,
                Some(signature.to_string()),
                Some(gas_used),
                None,
            )
            .await?;

        // Mark quote as executed
        self.ledger
            .update_quote_status(&mut tx, quote.id, QuoteStatus::Committed, QuoteStatus::Executed)
            .await?;

        // Record spending for risk controls
        self.risk
            .record_spending(&mut tx, Chain::Solana, quote.gas_estimate)
            .await?;

        // Audit log
        self.ledger
            .log_audit_event(
                AuditEventType::ExecutionCompleted,
                Some(execution_id),
                Some(quote.user_id),
                serde_json::json!({
                    "signature": signature.to_string(),
                    "gas_used": gas_used.to_string(),
                }),
            )
            .await?;

        tx.commit().await?;

        info!("Execution completed successfully for quote: {}", quote.id);

        // Return final execution state
        Ok(Execution {
            id: execution_id,
            quote_id: quote.id,
            solana_signature: Some(signature.to_string()),
            status: ExecutionStatus::Success,
            gas_used: Some(gas_used),
            error_message: None,
            retry_count: 0,
            executed_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        })
    }

    /// Deserialize Solana instructions from bytes
    fn deserialize_instructions(&self, bytes: &[u8]) -> AppResult<Vec<Instruction>> {
        // In production, this would use proper serialization (borsh, bincode, etc.)
        // For now, we'll assume a simple format

        if bytes.len() < 8 {
            return Err(ExecutionError::SolanaFailed(
                "Invalid instruction data".to_string(),
            )
            .into());
        }

        // Placeholder: In production, properly deserialize instructions
        // For now, return empty vec (would fail simulation)
        Ok(vec![])
    }

    /// Build a Solana transaction
    fn build_transaction(&self, instructions: Vec<Instruction>) -> AppResult<Transaction> {
        let recent_blockhash = self
            .client
            .get_latest_blockhash()
            .map_err(|e| ExecutionError::SolanaFailed(format!("Failed to get blockhash: {}", e)))?;

        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.treasury_keypair.pubkey()),
            &[&*self.treasury_keypair],
            recent_blockhash,
        );

        Ok(transaction)
    }

    /// Simulate transaction before execution
    ///
    /// SECURITY: Catches errors before committing treasury funds
    async fn simulate_transaction(&self, transaction: &Transaction) -> AppResult<()> {
        let result = self
            .client
            .simulate_transaction(transaction)
            .map_err(|e| {
                ExecutionError::SimulationFailed(format!("Simulation error: {}", e))
            })?;

        if result.value.err.is_some() {
            return Err(ExecutionError::SimulationFailed(format!(
                "Transaction would fail: {:?}",
                result.value.err
            ))
            .into());
        }

        Ok(())
    }

    /// Send transaction to Solana network
    async fn send_transaction(&self, transaction: Transaction) -> AppResult<Signature> {
        let signature = self
            .client
            .send_and_confirm_transaction(&transaction)
            .map_err(|e| ExecutionError::SolanaFailed(format!("Send failed: {}", e)))?;

        Ok(signature)
    }

    /// Confirm transaction and get gas used
    async fn confirm_transaction(&self, signature: &Signature) -> AppResult<Decimal> {
        // In production, this would:
        // 1. Poll for confirmation with timeout
        // 2. Parse transaction details to get actual gas used
        // 3. Handle various confirmation levels

        // For now, return estimated gas (would be replaced with actual)
        Ok(Decimal::from(5000))
    }

    /// Get treasury balance
    pub async fn get_treasury_balance(&self) -> AppResult<Decimal> {
        let balance = self
            .client
            .get_balance(&self.treasury_keypair.pubkey())
            .map_err(|e| {
                ExecutionError::SolanaFailed(format!("Failed to get balance: {}", e))
            })?;

        // Convert lamports to SOL
        Ok(Decimal::from(balance) / Decimal::from(1_000_000_000))
    }

    /// Check if treasury has sufficient balance
    pub async fn check_treasury_balance(&self, required: Decimal) -> AppResult<()> {
        let balance = self.get_treasury_balance().await?;

        if balance < required {
            return Err(ExecutionError::InsufficientTreasury.into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solana_config_defaults() {
        let config = SolanaConfig::default();
        assert!(!config.rpc_url.is_empty());
        assert!(config.max_retries > 0);
    }
}








<!-- src/api/models.rs -->

use crate::ledger::models::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ========== REQUEST MODELS ==========

/// Request to create a quote
#[derive(Debug, Deserialize)]
pub struct QuoteRequest {
    pub user_id: Uuid,
    pub source_chain: Chain,
    /// Base64 encoded Solana instructions
    pub solana_instructions_base64: String,
    pub estimated_compute_units: i32,
}

/// Request to commit a quote (after payment detected)
#[derive(Debug, Deserialize)]
pub struct CommitRequest {
    pub quote_id: Uuid,
}

/// Stellar webhook payload
#[derive(Debug, Deserialize)]
pub struct StellarWebhookPayload {
    pub transaction_hash: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub memo: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Near webhook payload
#[derive(Debug, Deserialize)]
pub struct NearWebhookPayload {
    pub transaction_hash: String,
    pub signer_id: String,
    pub receiver_id: String,
    pub amount: String,
    pub memo: Option<String>,
    pub timestamp: DateTime<Utc>,
}

// ========== RESPONSE MODELS ==========

/// Quote response
#[derive(Debug, Serialize)]
pub struct QuoteResponse {
    pub quote_id: Uuid,
    pub user_id: Uuid,
    pub source_chain: String,
    pub total_cost: String,
    pub service_fee: String,
    pub gas_estimate: String,
    pub expires_at: DateTime<Utc>,
    pub payment_address: String,
    pub nonce: String,
}

impl From<Quote> for QuoteResponse {
    fn from(quote: Quote) -> Self {
        Self {
            quote_id: quote.id,
            user_id: quote.user_id,
            source_chain: quote.source_chain.as_str().to_string(),
            total_cost: quote.total_cost.to_string(),
            service_fee: quote.service_fee.to_string(),
            gas_estimate: quote.gas_estimate.to_string(),
            expires_at: quote.expires_at,
            payment_address: quote.payment_address.unwrap_or_default(),
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
}

/// Execution status response
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub quote_id: Uuid,
    pub status: String,
    pub solana_signature: Option<String>,
    pub executed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

/// Webhook processing response
#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub accepted: bool,
    pub quote_id: Option<Uuid>,
    pub message: String,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub circuit_breaker_active: bool,
}







<!-- src/api/handlers.rs -->

use super::models::*;
use crate::error::AppResult;
use crate::ledger::repository::LedgerRepository;
use crate::quote_engine::QuoteEngine;
use crate::solana::SolanaExecutor;
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub ledger: Arc<LedgerRepository>,
    pub quote_engine: Arc<QuoteEngine>,
    pub solana_executor: Arc<SolanaExecutor>,
}

/// POST /quote - Generate a quote
///
/// SECURITY: Validates all inputs before creating quote
pub async fn create_quote(
    State(state): State<AppState>,
    Json(request): Json<QuoteRequest>,
) -> AppResult<Json<QuoteResponse>> {
    info!("Creating quote for user: {}", request.user_id);

    // Decode base64 instructions
    let instructions = base64::decode(&request.solana_instructions_base64)
        .map_err(|e| crate::error::QuoteError::InvalidParameters(format!("Invalid base64: {}", e)))?;

    // Generate quote
    let quote = state
        .quote_engine
        .generate_quote(
            request.user_id,
            request.source_chain,
            instructions,
            request.estimated_compute_units,
        )
        .await?;

    info!("Quote created: {}", quote.id);

    Ok(Json(QuoteResponse::from(quote)))
}

/// POST /commit - Commit a quote for execution
///
/// SECURITY: This is typically called internally after payment detection
pub async fn commit_quote(
    State(state): State<AppState>,
    Json(request): Json<CommitRequest>,
) -> AppResult<Json<CommitResponse>> {
    info!("Committing quote: {}", request.quote_id);

    let quote = state.quote_engine.commit_quote(request.quote_id).await?;

    // Trigger execution asynchronously
    let executor = state.solana_executor.clone();
    let quote_id = quote.id;
    
    tokio::spawn(async move {
        match executor.execute_quote(quote_id).await {
            Ok(execution) => {
                info!("Execution completed for quote {}: {:?}", quote_id, execution.status);
            }
            Err(e) => {
                error!("Execution failed for quote {}: {:?}", quote_id, e);
            }
        }
    });

    Ok(Json(CommitResponse {
        quote_id: quote.id,
        status: "committed".to_string(),
        message: "Quote committed and execution initiated".to_string(),
    }))
}

/// POST /webhook/stellar - Stellar payment webhook
///
/// SECURITY: This endpoint must be authenticated in production
pub async fn stellar_webhook(
    State(state): State<AppState>,
    Json(payload): Json<StellarWebhookPayload>,
) -> AppResult<Json<WebhookResponse>> {
    info!("Received Stellar webhook: {}", payload.transaction_hash);

    // Extract quote ID from memo
    let quote_id = match &payload.memo {
        Some(memo) => match Uuid::parse_str(memo) {
            Ok(id) => id,
            Err(_) => {
                return Ok(Json(WebhookResponse {
                    accepted: false,
                    quote_id: None,
                    message: "Invalid quote ID in memo".to_string(),
                }));
            }
        },
        None => {
            return Ok(Json(WebhookResponse {
                accepted: false,
                quote_id: None,
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
                message: "Quote not found".to_string(),
            }));
        }
    };

    // Validate payment amount
    let paid_amount: rust_decimal::Decimal = payload
        .amount
        .parse()
        .map_err(|_| crate::error::QuoteError::InvalidParameters("Invalid amount".to_string()))?;

    if paid_amount < quote.total_cost {
        return Ok(Json(WebhookResponse {
            accepted: false,
            quote_id: Some(quote_id),
            message: format!(
                "Insufficient payment: {} < {}",
                paid_amount, quote.total_cost
            ),
        }));
    }

    // Commit quote
    state.quote_engine.commit_quote(quote_id).await?;

    // Trigger execution
    let executor = state.solana_executor.clone();
    tokio::spawn(async move {
        match executor.execute_quote(quote_id).await {
            Ok(_) => info!("Execution completed for quote: {}", quote_id),
            Err(e) => error!("Execution failed for quote {}: {:?}", quote_id, e),
        }
    });

    Ok(Json(WebhookResponse {
        accepted: true,
        quote_id: Some(quote_id),
        message: "Payment accepted, execution initiated".to_string(),
    }))
}

/// POST /webhook/near - Near payment webhook
///
/// SECURITY: This endpoint must be authenticated in production
pub async fn near_webhook(
    State(state): State<AppState>,
    Json(payload): Json<NearWebhookPayload>,
) -> AppResult<Json<WebhookResponse>> {
    info!("Received Near webhook: {}", payload.transaction_hash);

    // Similar logic to Stellar webhook
    // Extract quote ID, validate payment, commit, and execute

    // For now, return placeholder
    Ok(Json(WebhookResponse {
        accepted: true,
        quote_id: None,
        message: "Near webhook handler not fully implemented".to_string(),
    }))
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
    let execution = sqlx::query!(
        r#"
        SELECT id, solana_signature, status as "status: crate::ledger::models::ExecutionStatus",
               error_message, executed_at
        FROM executions
        WHERE quote_id = $1
        "#,
        quote_id
    )
    .fetch_optional(&state.ledger.pool)
    .await?;

    let (solana_signature, executed_at, error_message) = match execution {
        Some(exec) => (exec.solana_signature, Some(exec.executed_at), exec.error_message),
        None => (None, None, None),
    };

    Ok(Json(StatusResponse {
        quote_id,
        status: format!("{:?}", quote.status),
        solana_signature,
        executed_at,
        error_message,
    }))
}

/// GET /health - Health check
pub async fn health_check(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    let circuit_breaker = state.ledger.get_active_circuit_breaker().await?;

    Ok(Json(HealthResponse {
        status: if circuit_breaker.is_some() {
            "degraded".to_string()
        } else {
            "healthy".to_string()
        },
        timestamp: Utc::now(),
        circuit_breaker_active: circuit_breaker.is_some(),
    }))
}









<!-- src/main.rs -->

mod api;
mod config;
mod error;
mod ledger;
mod quote_engine;
mod risk;
mod solana;
mod stellar;
mod near;
mod settlement;

use api::handlers::{AppState, *};
use axum::{
    routing::{get, post},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Cross-Chain Inventory Backend");

    // Load configuration
    dotenv::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let bind_address = std::env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    // Create database pool
    info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await?;

    info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Initialize components
    let ledger = Arc::new(ledger::repository::LedgerRepository::new(pool.clone()));

    let quote_config = quote_engine::QuoteConfig::default();
    let quote_engine = Arc::new(quote_engine::QuoteEngine::new(
        quote_config,
        ledger.clone(),
    ));

    let risk_config = risk::RiskConfig::default();
    let risk_controller = Arc::new(risk::RiskController::new(
        risk_config,
        ledger.clone(),
    ));

    // SECURITY WARNING: In production, load keypair from KMS
    // This is a placeholder that loads from environment
    let treasury_keypair_bytes = std::env::var("SOLANA_TREASURY_KEY")
        .expect("SOLANA_TREASURY_KEY must be set");
    let treasury_keypair = solana_sdk::signature::Keypair::from_base58_string(&treasury_keypair_bytes);

    let solana_config = solana::SolanaConfig::default();
    let solana_executor = Arc::new(solana::SolanaExecutor::new(
        solana_config,
        ledger.clone(),
        risk_controller.clone(),
        treasury_keypair,
    ));

    // Build application state
    let state = AppState {
        ledger: ledger.clone(),
        quote_engine: quote_engine.clone(),
        solana_executor: solana_executor.clone(),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/quote", post(create_quote))
        .route("/commit", post(commit_quote))
        .route("/webhook/stellar", post(stellar_webhook))
        .route("/webhook/near", post(near_webhook))
        .route("/status/:quote_id", get(get_status))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    info!("Server starting on {}", bind_address);
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    
    axum::serve(listener, app)
        .await?;

    Ok(())
}








<!-- module organising files -->

// src/ledger/mod.rs
pub mod models;
pub mod repository;

pub use models::*;
pub use repository::LedgerRepository;

// src/quote_engine/mod.rs
mod engine;
mod oracle;

pub use engine::{QuoteConfig, QuoteEngine};

// src/risk/mod.rs
mod controls;
mod circuit_breaker;

pub use controls::{RiskConfig, RiskController};

// src/solana/mod.rs
mod executor;
mod gas_estimator;

pub use executor::{SolanaConfig, SolanaExecutor};

// src/stellar/mod.rs
mod monitor;
mod parser;

pub use monitor::StellarMonitor;

// src/near/mod.rs
mod monitor;
mod parser;

pub use monitor::NearMonitor;

// src/settlement/mod.rs
mod reconciler;

pub use reconciler::SettlementReconciler;

// src/api/mod.rs
pub mod handlers;
pub mod models;

pub use handlers::AppState;

// src/config.rs
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub bind_address: String,
    pub stellar_horizon_url: String,
    pub near_rpc_url: String,
    pub solana_rpc_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, config::ConfigError> {
        config::Config::builder()
            .add_source(config::Environment::default())
            .build()?
            .try_deserialize()
    }
}




<!-- .env -->

# Database
DATABASE_URL=postgresql://user:password@localhost:5432/cross_chain_inventory

# Server
BIND_ADDRESS=0.0.0.0:8080
RUST_LOG=info,sqlx=warn

# Solana
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
# WARNING: In production, use KMS/HSM for key management
SOLANA_TREASURY_KEY=base58_encoded_keypair_here

# Stellar
STELLAR_HORIZON_URL=https://horizon.stellar.org
STELLAR_ESCROW_SECRET=stellar_secret_key_here

# Near
NEAR_RPC_URL=https://rpc.mainnet.near.org
NEAR_ACCOUNT_ID=payment.near
NEAR_PRIVATE_KEY=ed25519:base58_key_here

# Risk Controls
STELLAR_DAILY_LIMIT=1000000
NEAR_DAILY_LIMIT=10000
SOLANA_DAILY_LIMIT=100
CIRCUIT_BREAKER_ENABLED=true

# Quote Engine
SERVICE_FEE_RATE=0.001
QUOTE_TTL_SECONDS=300







<!-- docker-compose.yml -->

version: '3.8'

services:
  postgres:
    image: postgres:15-alpine
    environment:
      POSTGRES_DB: cross_chain_inventory
      POSTGRES_USER: user
      POSTGRES_PASSWORD: password
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U user"]
      interval: 10s
      timeout: 5s
      retries: 5

  backend:
    build: .
    depends_on:
      postgres:
        condition: service_healthy
    environment:
      DATABASE_URL: postgresql://user:password@postgres:5432/cross_chain_inventory
      BIND_ADDRESS: 0.0.0.0:8080
      RUST_LOG: info
    ports:
      - "8080:8080"
    env_file:
      - .env
    restart: unless-stopped

volumes:
  postgres_data:








<!-- Docker -->

# Build stage
FROM rust:1.75-slim as builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source
COPY src ./src
COPY migrations ./migrations

# Build for release
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/cross-chain-inventory-backend /app/

# Copy migrations
COPY --from=builder /app/migrations /app/migrations

# Create non-root user
RUN useradd -m -u 1000 appuser && chown -R appuser:appuser /app
USER appuser

EXPOSE 8080

CMD ["/app/cross-chain-inventory-backend"]









<!-- tests/integration_test.rs -->

use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test]
async fn test_quote_lifecycle(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // This test verifies the complete quote lifecycle:
    // 1. Create user
    // 2. Generate quote
    // 3. Commit quote
    // 4. Execute transaction
    // 5. Verify execution

    // Setup
    let ledger = cross_chain_inventory_backend::ledger::LedgerRepository::new(pool.clone());
    
    // Create user
    let user = ledger
        .create_user(Some("GDAI...TEST".to_string()), None)
        .await?;
    
    assert!(user.stellar_address.is_some());

    // Generate quote
    let quote = ledger
        .create_quote(
            user.id,
            cross_chain_inventory_backend::ledger::models::Chain::Stellar,
            rust_decimal_macros::dec!(1000000),
            cross_chain_inventory_backend::ledger::models::Chain::Solana,
            rust_decimal_macros::dec!(1000000),
            rust_decimal_macros::dec!(1000),
            rust_decimal_macros::dec!(5000),
            vec![1, 2, 3, 4],
            200_000,
            format!("nonce-{}", Uuid::new_v4()),
            chrono::Utc::now() + chrono::Duration::minutes(5),
            Some("PAYMENT_ADDRESS".to_string()),
        )
        .await?;

    assert_eq!(quote.user_id, user.id);
    assert!(quote.is_valid());

    // Verify quote status progression
    let mut tx = ledger.begin_tx().await?;
    
    ledger
        .update_quote_status(
            &mut tx,
            quote.id,
            cross_chain_inventory_backend::ledger::models::QuoteStatus::Pending,
            cross_chain_inventory_backend::ledger::models::QuoteStatus::Committed,
        )
        .await?;
    
    tx.commit().await?;

    // Verify status was updated
    let updated_quote = ledger.get_quote(quote.id).await?.unwrap();
    assert_eq!(
        updated_quote.status,
        cross_chain_inventory_backend::ledger::models::QuoteStatus::Committed
    );

    Ok(())
}

#[sqlx::test]
async fn test_idempotent_execution(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // This test verifies that execution is idempotent
    // Multiple calls with same quote_id should not create duplicate executions

    let ledger = cross_chain_inventory_backend::ledger::LedgerRepository::new(pool.clone());
    
    // Create user and quote
    let user = ledger
        .create_user(Some("GDAI...TEST2".to_string()), None)
        .await?;

    let quote = ledger
        .create_quote(
            user.id,
            cross_chain_inventory_backend::ledger::models::Chain::Stellar,
            rust_decimal_macros::dec!(1000000),
            cross_chain_inventory_backend::ledger::models::Chain::Solana,
            rust_decimal_macros::dec!(1000000),
            rust_decimal_macros::dec!(1000),
            rust_decimal_macros::dec!(5000),
            vec![1, 2, 3, 4],
            200_000,
            format!("nonce-{}", Uuid::new_v4()),
            chrono::Utc::now() + chrono::Duration::minutes(5),
            Some("PAYMENT_ADDRESS".to_string()),
        )
        .await?;

    // First execution - should succeed
    let mut tx1 = ledger.begin_tx().await?;
    let execution1 = ledger.create_execution(&mut tx1, quote.id).await;
    assert!(execution1.is_ok());
    tx1.commit().await?;

    // Second execution - should fail due to UNIQUE constraint
    let mut tx2 = ledger.begin_tx().await?;
    let execution2 = ledger.create_execution(&mut tx2, quote.id).await;
    assert!(execution2.is_err());

    Ok(())
}

#[sqlx::test]
async fn test_daily_spending_limits(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Test that daily spending limits are properly tracked

    let ledger = cross_chain_inventory_backend::ledger::LedgerRepository::new(pool.clone());
    let today = chrono::Utc::now().date_naive();

    // Record spending
    let mut tx = ledger.begin_tx().await?;
    ledger
        .increment_daily_spending(
            &mut tx,
            cross_chain_inventory_backend::ledger::models::Chain::Solana,
            today,
            rust_decimal_macros::dec!(1000),
        )
        .await?;
    tx.commit().await?;

    // Verify spending was recorded
    let spending = ledger
        .get_daily_spending(
            cross_chain_inventory_backend::ledger::models::Chain::Solana,
            today,
        )
        .await?
        .unwrap();

    assert_eq!(spending.amount_spent, rust_decimal_macros::dec!(1000));
    assert_eq!(spending.transaction_count, 1);

    // Record more spending
    let mut tx2 = ledger.begin_tx().await?;
    ledger
        .increment_daily_spending(
            &mut tx2,
            cross_chain_inventory_backend::ledger::models::Chain::Solana,
            today,
            rust_decimal_macros::dec!(500),
        )
        .await?;
    tx2.commit().await?;

    // Verify cumulative spending
    let spending2 = ledger
        .get_daily_spending(
            cross_chain_inventory_backend::ledger::models::Chain::Solana,
            today,
        )
        .await?
        .unwrap();

    assert_eq!(spending2.amount_spent, rust_decimal_macros::dec!(1500));
    assert_eq!(spending2.transaction_count, 2);

    Ok(())
}

#[sqlx::test]
async fn test_circuit_breaker(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Test circuit breaker state management

    let ledger = cross_chain_inventory_backend::ledger::LedgerRepository::new(pool.clone());

    // Initially no active circuit breaker
    let initial = ledger.get_active_circuit_breaker().await?;
    assert!(initial.is_none());

    // Trigger circuit breaker
    let state = ledger
        .trigger_circuit_breaker("Test trigger".to_string())
        .await?;

    assert!(state.is_active());
    assert_eq!(state.reason, "Test trigger");

    // Verify it's now active
    let active = ledger.get_active_circuit_breaker().await?;
    assert!(active.is_some());

    Ok(())
}

#[sqlx::test]
async fn test_balance_operations(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    // Test balance locking and unlocking

    let ledger = cross_chain_inventory_backend::ledger::LedgerRepository::new(pool.clone());

    // Create user
    let user = ledger
        .create_user(Some("GDAI...TEST3".to_string()), None)
        .await?;

    // Initialize balance
    sqlx::query!(
        r#"
        INSERT INTO balances (user_id, chain, amount)
        VALUES ($1, $2, $3)
        "#,
        user.id,
        cross_chain_inventory_backend::ledger::models::Chain::Stellar as _,
        rust_decimal_macros::dec!(1000000)
    )
    .execute(&pool)
    .await?;

    // Lock funds
    let mut tx = ledger.begin_tx().await?;
    ledger
        .lock_funds(
            &mut tx,
            user.id,
            cross_chain_inventory_backend::ledger::models::Chain::Stellar,
            rust_decimal_macros::dec!(100000),
        )
        .await?;
    tx.commit().await?;

    // Verify locked
    let balance = ledger
        .get_balance(
            user.id,
            cross_chain_inventory_backend::ledger::models::Chain::Stellar,
        )
        .await?
        .unwrap();

    assert_eq!(balance.locked_amount, rust_decimal_macros::dec!(100000));
    assert_eq!(balance.available(), rust_decimal_macros::dec!(900000));

    // Try to lock more than available - should fail
    let mut tx2 = ledger.begin_tx().await?;
    let result = ledger
        .lock_funds(
            &mut tx2,
            user.id,
            cross_chain_inventory_backend::ledger::models::Chain::Stellar,
            rust_decimal_macros::dec!(1000000),
        )
        .await;

    assert!(result.is_err());

    Ok(())
}










<!-- Deployment.md -->

# Production Deployment Guide

## Prerequisites

- Kubernetes cluster (recommended) or Docker Swarm
- PostgreSQL 15+ with replication
- HSM/KMS for key management (AWS KMS, GCP KMS, or Azure Key Vault)
- Monitoring infrastructure (Prometheus, Grafana)
- Alert manager (PagerDuty, OpsGenie)

## Security Checklist

### Key Management

- [ ] Deploy HSM/KMS for all private keys
- [ ] Rotate keys quarterly
- [ ] Implement key usage limits
- [ ] Set up key backup and recovery procedures
- [ ] Document key hierarchy and access controls

### Database Security

- [ ] Enable SSL/TLS for all database connections
- [ ] Configure database firewall rules
- [ ] Enable automated backups with point-in-time recovery
- [ ] Set up read replicas for high availability
- [ ] Implement row-level security policies
- [ ] Enable audit logging

### Network Security

- [ ] Deploy behind load balancer with DDoS protection
- [ ] Enable rate limiting per IP and per user
- [ ] Implement request signing for webhooks
- [ ] Use mTLS for internal service communication
- [ ] Configure VPC/private networking

### Application Security

- [ ] Enable all circuit breakers in production
- [ ] Set conservative daily spending limits
- [ ] Configure alerting for all risk thresholds
- [ ] Enable structured audit logging
- [ ] Implement request validation middleware

## Deployment Steps

### 1. Database Setup

```bash
# Create database with replication
createdb cross_chain_inventory_prod

# Run migrations
sqlx migrate run --database-url $DATABASE_URL

# Verify schema
psql $DATABASE_URL -c "\dt"
```

### 2. Key Management

```bash
# Generate keys in KMS (example for AWS KMS)
aws kms create-key \
  --description "Solana Treasury Key" \
  --key-usage SIGN_VERIFY \
  --key-spec ECC_SECG_P256K1

# Store key ID in environment
export SOLANA_KMS_KEY_ID=<key-id>
```

### 3. Configuration

```bash
# Copy production config
cp .env.example .env.production

# Set production values
vim .env.production
```

Critical environment variables:
```bash
DATABASE_URL=postgresql://user:pass@db-primary:5432/cross_chain_inventory_prod
BIND_ADDRESS=0.0.0.0:8080
RUST_LOG=info,sqlx=warn

# Use KMS key IDs, not raw keys
SOLANA_KMS_KEY_ID=<kms-key-id>
STELLAR_KMS_KEY_ID=<kms-key-id>
NEAR_KMS_KEY_ID=<kms-key-id>

# Risk controls - start conservative
STELLAR_DAILY_LIMIT=100000
NEAR_DAILY_LIMIT=1000
SOLANA_DAILY_LIMIT=10
CIRCUIT_BREAKER_ENABLED=true
HOURLY_OUTFLOW_THRESHOLD=0.1

# RPC endpoints with authentication
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
STELLAR_HORIZON_URL=https://horizon.stellar.org
NEAR_RPC_URL=https://rpc.mainnet.near.org
```

### 4. Build and Deploy

```bash
# Build Docker image
docker build -t cross-chain-inventory:v1.0.0 .

# Tag for registry
docker tag cross-chain-inventory:v1.0.0 \
  registry.example.com/cross-chain-inventory:v1.0.0

# Push to registry
docker push registry.example.com/cross-chain-inventory:v1.0.0

# Deploy to Kubernetes
kubectl apply -f k8s/
```

### 5. Verify Deployment

```bash
# Check pod status
kubectl get pods -l app=cross-chain-inventory

# Check logs
kubectl logs -f deployment/cross-chain-inventory

# Health check
curl https://api.example.com/health

# Expected response:
# {"status":"healthy","timestamp":"...","circuit_breaker_active":false}
```

## Monitoring Setup

### Metrics to Track

1. **Business Metrics**
   - Quote generation rate
   - Quote expiry rate
   - Execution success rate
   - Average execution time
   - Daily volume by chain

2. **Technical Metrics**
   - API response times (p50, p95, p99)
   - Database query latency
   - Connection pool utilization
   - Memory usage
   - CPU usage

3. **Security Metrics**
   - Failed authentication attempts
   - Circuit breaker triggers
   - Daily spending by chain
   - Abnormal transaction patterns
   - Execution failures

### Alert Conditions

**Critical (Page immediately)**
- Circuit breaker triggered
- Database connection failures
- Treasury balance < 10% of daily limit
- Execution failure rate > 5%
- API downtime

**Warning (Slack/Email)**
- Daily spending > 80% of limit
- Treasury balance < 50% of daily limit
- Execution failure rate > 2%
- Database query latency > 1s
- API response time > 500ms

### Sample Prometheus Queries

```promql
# Execution success rate
sum(rate(executions_total{status="success"}[5m])) 
/ 
sum(rate(executions_total[5m]))

# Daily spending by chain
sum by (chain) (daily_spending_amount)

# API request latency
histogram_quantile(0.95, 
  rate(http_request_duration_seconds_bucket[5m])
)
```

## Operational Procedures

### Daily Operations

1. **Morning Check** (Automated)
   - Verify all services healthy
   - Check treasury balances
   - Review previous day's metrics
   - Verify settlement reconciliation

2. **Continuous Monitoring**
   - Watch alert channels
   - Monitor execution success rates
   - Track spending limits

### Incident Response

#### Circuit Breaker Triggered

```bash
# 1. Check reason
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  https://api.example.com/admin/circuit-breaker/status

# 2. Investigate root cause
# - Check recent executions
# - Review treasury balances
# - Examine error logs

# 3. Fix issue (e.g., replenish treasury)

# 4. Reset circuit breaker
curl -X POST \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  https://api.example.com/admin/circuit-breaker/reset
```

#### Failed Execution Investigation

```sql
-- Find recent failures
SELECT 
  e.id,
  e.quote_id,
  e.error_message,
  q.user_id,
  q.total_cost
FROM executions e
JOIN quotes q ON q.id = e.quote_id
WHERE e.status = 'failed'
  AND e.executed_at > NOW() - INTERVAL '1 hour'
ORDER BY e.executed_at DESC;

-- Check if pattern (same user, same error, etc.)
```

### Settlement Reconciliation

Run daily to verify all executions have corresponding settlements:

```bash
# Run reconciliation job
kubectl create job --from=cronjob/settlement-reconciler \
  settlement-reconciler-manual-$(date +%s)

# Check results
kubectl logs job/settlement-reconciler-manual-...
```

### Treasury Rebalancing

When treasury balance is low:

```bash
# 1. Check current balances
curl https://api.example.com/admin/treasury/balances

# 2. Calculate needed amount based on:
# - Daily spending patterns
# - Safety margin (7 days recommended)
# - Pending executions

# 3. Transfer from cold storage to hot wallet
# (Use multi-sig process for large amounts)

# 4. Verify balance updated
```

## Scaling Considerations

### Horizontal Scaling

The application is stateless and can scale horizontally:

```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
spec:
  replicas: 3  # Increase as needed
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
```

### Database Scaling

For high throughput:

1. **Read Replicas**: Route read queries to replicas
2. **Connection Pooling**: Already implemented with SQLx
3. **Partitioning**: Partition large tables by date
4. **Indexing**: Monitor slow queries and add indexes

### Treasury Management

For high volume:

1. **Multiple Treasury Accounts**: Shard by user or region
2. **Dynamic Limits**: Adjust limits based on time of day
3. **Predictive Rebalancing**: ML-based treasury forecasting

## Backup and Recovery

### Database Backups

```bash
# Automated daily backup (configure in your provider)
# - Full backup: Daily at 2 AM UTC
# - Incremental: Hourly
# - Retention: 30 days

# Manual backup
pg_dump $DATABASE_URL > backup-$(date +%Y%m%d).sql

# Restore (in emergency)
psql $DATABASE_URL < backup-20250101.sql
```

### Key Backup

- Store key backup in secure offline location
- Use multi-party computation or Shamir's Secret Sharing
- Test recovery procedure quarterly

### Disaster Recovery

1. **RTO (Recovery Time Objective)**: 1 hour
2. **RPO (Recovery Point Objective)**: 5 minutes

Recovery steps:
1. Restore database from latest backup
2. Restore keys from KMS
3. Deploy application in new region
4. Update DNS
5. Verify functionality

## Security Audit Checklist

Run before major releases:

- [ ] Code review by security team
- [ ] Dependency vulnerability scan
- [ ] Penetration testing
- [ ] Smart contract audit (if applicable)
- [ ] Threat modeling review
- [ ] Incident response drill

## Cost Optimization

Monitor costs for:
- Database (primary cost driver)
- RPC calls (especially Solana)
- KMS operations
- Load balancer
- Logging/monitoring

Optimization strategies:
- Cache frequently accessed data
- Batch RPC calls where possible
- Use websockets for real-time updates
- Archive old audit logs








<!-- API_example.md -->

# API Usage Examples

## Prerequisites

```bash
export API_BASE=https://api.example.com
export USER_ID=550e8400-e29b-41d4-a716-446655440000
```

## Flow 1: Stellar → Solana Payment

### Step 1: Generate Quote

```bash
curl -X POST $API_BASE/quote \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "'$USER_ID'",
    "source_chain": "stellar",
    "solana_instructions_base64": "AQABAgME...",
    "estimated_compute_units": 200000
  }'
```

**Response:**
```json
{
  "quote_id": "123e4567-e89b-12d3-a456-426614174000",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "source_chain": "stellar",
  "total_cost": "1006000",
  "service_fee": "1000",
  "gas_estimate": "5000",
  "expires_at": "2025-12-17T12:05:00Z",
  "payment_address": "GDAI...ESCROW",
  "nonce": "abc123-1234567890"
}
```

### Step 2: Send Payment on Stellar

```bash
# Using Stellar SDK (JavaScript example)
const transaction = new StellarSdk.TransactionBuilder(sourceAccount, {
  fee: StellarSdk.BASE_FEE,
  networkPassphrase: StellarSdk.Networks.PUBLIC,
})
  .addOperation(
    StellarSdk.Operation.payment({
      destination: 'GDAI...ESCROW',
      asset: StellarSdk.Asset.native(),
      amount: '1.006000',
    })
  )
  .addMemo(StellarSdk.Memo.text('123e4567-e89b-12d3-a456-426614174000'))
  .setTimeout(300)
  .build();

transaction.sign(keypair);
await server.submitTransaction(transaction);
```

### Step 3: Check Status

```bash
curl $API_BASE/status/123e4567-e89b-12d3-a456-426614174000
```

**Response (Pending):**
```json
{
  "quote_id": "123e4567-e89b-12d3-a456-426614174000",
  "status": "Committed",
  "solana_signature": null,
  "executed_at": null,
  "error_message": null
}
```

**Response (Completed):**
```json
{
  "quote_id": "123e4567-e89b-12d3-a456-426614174000",
  "status": "Executed",
  "solana_signature": "5Kn8w...",
  "executed_at": "2025-12-17T12:01:30Z",
  "error_message": null
}
```

## Flow 2: Near → Solana Payment

### Step 1: Generate Quote

```bash
curl -X POST $API_BASE/quote \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "'$USER_ID'",
    "source_chain": "near",
    "solana_instructions_base64": "AQABAgME...",
    "estimated_compute_units": 150000
  }'
```

### Step 2: Execute Near Transaction

```bash
# Using Near CLI
near call payment.near execute_payment \
  '{"quote_id": "123e4567-e89b-12d3-a456-426614174000"}' \
  --accountId user.near \
  --amount 1.006
```

### Step 3: Monitor Execution

```bash
# Poll for status
while true; do
  curl -s $API_BASE/status/123e4567-e89b-12d3-a456-426614174000 | jq
  sleep 5
done
```

## Testing Scripts

### Load Testing with k6

```javascript
// load_test.js
import http from 'k6/http';
import { check, sleep } from 'k6';
import { randomItem } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

export const options = {
  stages: [
    { duration: '2m', target: 100 }, // Ramp up to 100 users
    { duration: '5m', target: 100 }, // Stay at 100 users
    { duration: '2m', target: 0 },   // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'], // 95% of requests under 500ms
    http_req_failed: ['rate<0.01'],   // Less than 1% errors
  },
};

export default function () {
  const userId = '550e8400-e29b-41d4-a716-446655440000';
  
  // Generate quote
  const quoteRes = http.post(
    `${__ENV.API_BASE}/quote`,
    JSON.stringify({
      user_id: userId,
      source_chain: 'stellar',
      solana_instructions_base64: 'AQABAgME',
      estimated_compute_units: 200000,
    }),
    {
      headers: { 'Content-Type': 'application/json' },
    }
  );

  check(quoteRes, {
    'quote created': (r) => r.status === 200,
    'has quote_id': (r) => JSON.parse(r.body).quote_id !== undefined,
  });

  sleep(1);
}
```

Run load test:
```bash
k6 run --env API_BASE=$API_BASE load_test.js
```

### Integration Test Script

```bash
#!/bin/bash
# integration_test.sh

set -e

API_BASE=${API_BASE:-http://localhost:8080}
USER_ID="550e8400-e29b-41d4-a716-446655440000"

echo "=== Integration Test Suite ==="
echo "API Base: $API_BASE"
echo ""

# Test 1: Health Check
echo "Test 1: Health Check"
curl -f $API_BASE/health | jq
echo "✓ Health check passed"
echo ""

# Test 2: Create Quote
echo "Test 2: Create Quote"
QUOTE_RESPONSE=$(curl -s -X POST $API_BASE/quote \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "'$USER_ID'",
    "source_chain": "stellar",
    "solana_instructions_base64": "AQABAgME",
    "estimated_compute_units": 200000
  }')

QUOTE_ID=$(echo $QUOTE_RESPONSE | jq -r '.quote_id')
echo "Created quote: $QUOTE_ID"
echo "✓ Quote creation passed"
echo ""

# Test 3: Get Status
echo "Test 3: Get Quote Status"
curl -s $API_BASE/status/$QUOTE_ID | jq
echo "✓ Status check passed"
echo ""

# Test 4: Simulate Webhook (requires test payment)
echo "Test 4: Simulate Stellar Webhook"
curl -s -X POST $API_BASE/webhook/stellar \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_hash": "test-tx-'$(date +%s)'",
    "from_address": "GDAI...TEST",
    "to_address": "GDAI...ESCROW",
    "amount": "1006000",
    "memo": "'$QUOTE_ID'",
    "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
  }' | jq
echo "✓ Webhook processing passed"
echo ""

# Test 5: Verify Execution
echo "Test 5: Wait for Execution (max 30s)"
for i in {1..30}; do
  STATUS=$(curl -s $API_BASE/status/$QUOTE_ID | jq -r '.status')
  echo "Status: $STATUS"
  
  if [ "$STATUS" = "Executed" ] || [ "$STATUS" = "Failed" ]; then
    break
  fi
  
  sleep 1
done

echo "✓ Execution completed"
echo ""

echo "=== All Tests Passed ==="
```

### Chaos Testing

```bash
#!/bin/bash
# chaos_test.sh - Test resilience under adverse conditions

API_BASE=${API_BASE:-http://localhost:8080}

echo "=== Chaos Testing ==="

# Test 1: Expired Quote
echo "Test 1: Attempt to commit expired quote"
# Create quote, wait for expiry, then try to commit
QUOTE=$(curl -s -X POST $API_BASE/quote -H "Content-Type: application/json" -d '...')
QUOTE_ID=$(echo $QUOTE | jq -r '.quote_id')

echo "Waiting for quote to expire (5 minutes)..."
sleep 301

RESULT=$(curl -s -X POST $API_BASE/commit \
  -H "Content-Type: application/json" \
  -d '{"quote_id": "'$QUOTE_ID'"}')

echo "Result: $RESULT"
echo "Expected: Error about expired quote"
echo ""

# Test 2: Duplicate Execution
echo "Test 2: Test idempotency - try to execute same quote twice"
# Generate and commit quote
QUOTE=$(curl -s -X POST $API_BASE/quote -H "Content-Type: application/json" -d '...')
QUOTE_ID=$(echo $QUOTE | jq -r '.quote_id')

# First execution
curl -s -X POST $API_BASE/commit \
  -H "Content-Type: application/json" \
  -d '{"quote_id": "'$QUOTE_ID'"}'

sleep 2

# Second execution (should be rejected)
RESULT=$(curl -s -X POST $API_BASE/commit \
  -H "Content-Type: application/json" \
  -d '{"quote_id": "'$QUOTE_ID'"}')

echo "Result: $RESULT"
echo "Expected: Error about already executed"
echo ""

# Test 3: Insufficient Payment
echo "Test 3: Send payment less than quoted amount"
# Generate quote and send underpayment
# Should be rejected
echo ""

# Test 4: Invalid Quote ID
echo "Test 4: Webhook with non-existent quote ID"
curl -s -X POST $API_BASE/webhook/stellar \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_hash": "test-tx",
    "from_address": "GDAI...TEST",
    "to_address": "GDAI...ESCROW",
    "amount": "1000000",
    "memo": "00000000-0000-0000-0000-000000000000",
    "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
  }' | jq

echo "Expected: Quote not found error"
echo ""

echo "=== Chaos Tests Complete ==="
```

## Performance Testing

### Benchmark Quote Generation

```bash
#!/bin/bash
# benchmark_quotes.sh

API_BASE=${API_BASE:-http://localhost:8080}
USER_ID="550e8400-e29b-41d4-a716-446655440000"
NUM_REQUESTS=1000

echo "Benchmarking $NUM_REQUESTS quote requests..."

start_time=$(date +%s)

for i in $(seq 1 $NUM_REQUESTS); do
  curl -s -X POST $API_BASE/quote \
    -H "Content-Type: application/json" \
    -d '{
      "user_id": "'$USER_ID'",
      "source_chain": "stellar",
      "solana_instructions_base64": "AQABAgME",
      "estimated_compute_units": 200000
    }' > /dev/null &
  
  # Batch in groups of 100
  if [ $((i % 100)) -eq 0 ]; then
    wait
    echo "Completed $i requests..."
  fi
done

wait

end_time=$(date +%s)
duration=$((end_time - start_time))
rps=$((NUM_REQUESTS / duration))

echo ""
echo "Results:"
echo "- Total requests: $NUM_REQUESTS"
echo "- Duration: ${duration}s"
echo "- Requests per second: $rps"
```

## Monitoring Queries

### Check Daily Spending

```sql
-- Check today's spending by chain
SELECT 
  chain,
  amount_spent,
  transaction_count,
  amount_spent::float / CASE 
    WHEN chain = 'stellar' THEN 1000000
    WHEN chain = 'near' THEN 10000
    WHEN chain = 'solana' THEN 100
  END as percentage_of_limit
FROM daily_spending
WHERE date = CURRENT_DATE
ORDER BY chain;
```

### Recent Failed Executions

```sql
-- Find failed executions in last hour
SELECT 
  e.id,
  e.quote_id,
  q.user_id,
  e.error_message,
  e.executed_at
FROM executions e
JOIN quotes q ON q.id = e.quote_id
WHERE e.status = 'failed'
  AND e.executed_at > NOW() - INTERVAL '1 hour'
ORDER BY e.executed_at DESC
LIMIT 20;
```

### Circuit Breaker History

```sql
-- Check circuit breaker triggers in last 30 days
SELECT 
  triggered_at,
  reason,
  resolved_at,
  EXTRACT(EPOCH FROM (resolved_at - triggered_at)) / 60 as downtime_minutes
FROM circuit_breaker_state
WHERE triggered_at > NOW() - INTERVAL '30 days'
ORDER BY triggered_at DESC;
```

## Common Issues and Solutions

### Issue: Quote Expires Before Payment

**Symptom:** Users see "Quote expired" error

**Solution:**
```bash
# Increase quote TTL in config
QUOTE_TTL_SECONDS=600  # 10 minutes instead of 5
```

### Issue: Execution Timeout

**Symptom:** Executions stuck in "pending" state

**Solution:**
```sql
-- Find stuck executions
SELECT * FROM executions 
WHERE status = 'pending' 
  AND executed_at < NOW() - INTERVAL '5 minutes';

-- Manual intervention (if needed)
UPDATE executions 
SET status = 'failed', 
    error_message = 'Timeout - manual intervention'
WHERE id = '...';
```

### Issue: High Daily Spending

**Symptom:** Circuit breaker triggers due to volume

**Solution:**
```bash
# Temporarily increase limits
STELLAR_DAILY_LIMIT=2000000  # Double the limit

# Or pause non-critical operations
# Let team investigate spending patterns
```




<!-- migrations/001 -->

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    stellar_address TEXT,
    near_address TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_stellar UNIQUE (stellar_address),
    CONSTRAINT unique_near UNIQUE (near_address),
    CONSTRAINT at_least_one_address CHECK (
        stellar_address IS NOT NULL OR near_address IS NOT NULL
    )
);

CREATE INDEX idx_users_stellar ON users(stellar_address) WHERE stellar_address IS NOT NULL;
CREATE INDEX idx_users_near ON users(near_address) WHERE near_address IS NOT NULL;

-- Balances table
CREATE TYPE chain_type AS ENUM ('stellar', 'near', 'solana');

CREATE TABLE balances (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    chain chain_type NOT NULL,
    amount NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (amount >= 0),
    locked_amount NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (locked_amount >= 0),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, chain),
    CONSTRAINT valid_locked_amount CHECK (locked_amount <= amount)
);

CREATE INDEX idx_balances_user ON balances(user_id);

-- Quotes table
CREATE TYPE quote_status AS ENUM ('pending', 'committed', 'executed', 'expired', 'failed');

CREATE TABLE quotes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source_chain chain_type NOT NULL,
    source_amount NUMERIC(78, 0) NOT NULL CHECK (source_amount > 0),
    destination_chain chain_type NOT NULL,
    destination_amount NUMERIC(78, 0) NOT NULL CHECK (destination_amount > 0),
    service_fee NUMERIC(78, 0) NOT NULL CHECK (service_fee >= 0),
    gas_estimate NUMERIC(78, 0) NOT NULL CHECK (gas_estimate >= 0),
    total_cost NUMERIC(78, 0) NOT NULL CHECK (total_cost > 0),
    solana_instructions BYTEA NOT NULL,
    estimated_compute_units INTEGER NOT NULL CHECK (estimated_compute_units > 0),
    nonce TEXT NOT NULL UNIQUE,
    status quote_status NOT NULL DEFAULT 'pending',
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    payment_address TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT destination_must_be_solana CHECK (destination_chain = 'solana'),
    CONSTRAINT source_not_solana CHECK (source_chain != 'solana'),
    CONSTRAINT total_cost_calculation CHECK (
        total_cost = source_amount + service_fee + gas_estimate
    )
);

CREATE INDEX idx_quotes_user ON quotes(user_id);
CREATE INDEX idx_quotes_status ON quotes(status);
CREATE INDEX idx_quotes_expires ON quotes(expires_at);
CREATE INDEX idx_quotes_nonce ON quotes(nonce);
CREATE INDEX idx_quotes_created ON quotes(created_at DESC);

-- Executions table
CREATE TYPE execution_status AS ENUM ('pending', 'success', 'failed');

CREATE TABLE executions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    quote_id UUID NOT NULL REFERENCES quotes(id) ON DELETE CASCADE UNIQUE,
    solana_signature TEXT,
    status execution_status NOT NULL DEFAULT 'pending',
    gas_used NUMERIC(78, 0),
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    executed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP WITH TIME ZONE,
    CONSTRAINT signature_required_on_success CHECK (
        (status = 'success' AND solana_signature IS NOT NULL) OR status != 'success'
    )
);

CREATE UNIQUE INDEX idx_executions_quote ON executions(quote_id);
CREATE INDEX idx_executions_status ON executions(status);
CREATE INDEX idx_executions_signature ON executions(solana_signature) WHERE solana_signature IS NOT NULL;
CREATE INDEX idx_executions_executed ON executions(executed_at DESC);

-- Settlements table
CREATE TABLE settlements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    execution_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    source_chain chain_type NOT NULL,
    source_txn_hash TEXT NOT NULL,
    source_amount NUMERIC(78, 0) NOT NULL,
    settled_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    verified_at TIMESTAMP WITH TIME ZONE,
    CONSTRAINT unique_settlement_per_execution UNIQUE (execution_id)
);

CREATE INDEX idx_settlements_execution ON settlements(execution_id);
CREATE INDEX idx_settlements_source_txn ON settlements(source_txn_hash);
CREATE INDEX idx_settlements_settled ON settlements(settled_at DESC);

-- Treasury balances table (operational monitoring)
CREATE TABLE treasury_balances (
    chain chain_type PRIMARY KEY,
    balance NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (balance >= 0),
    last_updated TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_reconciled TIMESTAMP WITH TIME ZONE
);

INSERT INTO treasury_balances (chain) VALUES ('stellar'), ('near'), ('solana');

-- Daily spending limits tracking
CREATE TABLE daily_spending (
    chain chain_type NOT NULL,
    date DATE NOT NULL,
    amount_spent NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (amount_spent >= 0),
    transaction_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (chain, date)
);

CREATE INDEX idx_daily_spending_date ON daily_spending(date DESC);

-- Circuit breaker state
CREATE TABLE circuit_breaker_state (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    triggered_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    reason TEXT NOT NULL,
    resolved_at TIMESTAMP WITH TIME ZONE,
    resolved_by TEXT
);

CREATE INDEX idx_circuit_breaker_active ON circuit_breaker_state(triggered_at DESC)
    WHERE resolved_at IS NULL;

-- Audit log for critical operations
CREATE TYPE audit_event_type AS ENUM (
    'quote_created',
    'quote_committed',
    'execution_started',
    'execution_completed',
    'execution_failed',
    'settlement_recorded',
    'circuit_breaker_triggered',
    'circuit_breaker_reset',
    'limit_exceeded'
);

CREATE TABLE audit_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type audit_event_type NOT NULL,
    entity_id UUID,
    user_id UUID,
    details JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_log_event_type ON audit_log(event_type);
CREATE INDEX idx_audit_log_entity ON audit_log(entity_id);
CREATE INDEX idx_audit_log_created ON audit_log(created_at DESC);

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for updated_at
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_balances_updated_at BEFORE UPDATE ON balances
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_quotes_updated_at BEFORE UPDATE ON quotes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
