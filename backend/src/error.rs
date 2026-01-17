use std::str::Utf8Error;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use near_primitives::account::id::ParseAccountError;
use sqlx::migrate::MigrateError;
use crate::ledger::models::Chain;
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

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Adapter not found")]
    AdapterNotFound,

    #[error("Unsupported chain pair")]
    UnsupportedChainPair,

    #[error("Unsupported chain: {0}")]
    UnsupportedChain(String),

    #[error("No liquidity available: {0}")]
    NoLiquidityAvailable(String),

    #[error("External error: {0}")]
    ExternalError(String),

    #[error("Bad request: {0}")]
    BadRequest(String),
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

    #[error("Same chain funding and execution not allowed")]
    SameChainFunding,

    #[error("Chain pair {funding:?} -> {execution:?} not supported")]
    UnsupportedChainPair { funding: Chain, execution: Chain },

    #[error("Price feed unavailable: {0}")]
    PriceUnavailable(String),
}

/// Execution-related errors
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Execution failed on {chain:?}: {message}")]
    ChainExecutionFailed { chain: Chain, message: String },

    #[error("Gas estimation failed: {0}")]
    GasEstimationFailed(String),

    #[error("Transaction simulation failed: {0}")]
    SimulationFailed(String),

    #[error("Insufficient treasury balance on {0:?}")]
    InsufficientTreasury(Chain),

    #[error("Execution already exists for quote")]
    DuplicateExecution,

    #[error("Transaction timeout")]
    Timeout,

    #[error("Invalid Instruction Data")]
    InvalidInstructionData,

    #[error("Unsupported execution chain: {0:?}")]
    UnsupportedChain(Chain),

    #[error("Invalid chain pair: funding={funding:?}, execution={execution:?}")]
    InvalidChainPair { funding: Chain, execution: Chain },

    #[error("Executor chain mismatch: expected {expected:?}, got {actual:?}")]
    ExecutorChainMismatch { expected: Chain, actual: Chain },
}

/// Risk control errors
#[derive(Error, Debug)]
pub enum RiskError {
    #[error("Daily limit exceeded for {chain:?}: {current}/{limit}")]
    DailyLimitExceeded {
        chain: Chain,
        current: String,
        limit: String,
    },

    #[error("Insufficient balance on {chain:?}: {asset} - required {required}")]
    InsufficientBalance {
        chain: Chain,
        asset: String,
        required: String,
    },

    #[error("Circuit breaker triggered for {chain:?}: {reason}")]
    CircuitBreakerTriggered { chain: Chain, reason: String },

    #[error("Abnormal outflow detected on {chain:?}: {details}")]
    AbnormalOutflow { chain: Chain, details: String },

    #[error("User spending limit exceeded")]
    UserLimitExceeded,
}

/// Chain-specific errors
#[derive(Error, Debug)]
pub enum ChainError {
    #[error("Solana error: {0}")]
    Solana(String),

    #[error("Stellar error: {0}")]
    Stellar(String),

    #[error("Near error: {0}")]
    Near(String),

    #[error("Invalid address format for {chain:?}: {address}")]
    InvalidAddress { chain: Chain, address: String },

    #[error("Transaction parsing failed on {chain:?}: {message}")]
    ParseError { chain: Chain, message: String },
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
            AppError::Quote(QuoteError::SameChainFunding) => (
                StatusCode::BAD_REQUEST,
                "SAME_CHAIN_FUNDING",
                "Funding and execution chains must be different".to_string(),
                None,
            ),
            AppError::Quote(QuoteError::UnsupportedChainPair { funding, execution }) => (
                StatusCode::BAD_REQUEST,
                "UNSUPPORTED_CHAIN_PAIR",
                format!("Chain pair {:?} -> {:?} is not supported", funding, execution),
                Some(serde_json::json!({
                    "funding_chain": funding,
                    "execution_chain": execution,
                })),
            ),
            AppError::Execution(ExecutionError::ChainExecutionFailed { chain, message }) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "EXECUTION_FAILED",
                format!("Execution failed on {:?}: {}", chain, message),
                Some(serde_json::json!({"chain": chain})),
            ),
            AppError::Execution(ExecutionError::InsufficientTreasury(chain)) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "INSUFFICIENT_TREASURY",
                format!("Insufficient treasury balance on {:?}", chain),
                Some(serde_json::json!({"chain": chain})),
            ),
            AppError::Execution(ExecutionError::UnsupportedChain(chain)) => (
                StatusCode::BAD_REQUEST,
                "UNSUPPORTED_CHAIN",
                format!("Chain {:?} is not supported for execution", chain),
                Some(serde_json::json!({"chain": chain})),
            ),
            AppError::Execution(ExecutionError::InvalidChainPair { funding, execution }) => (
                StatusCode::BAD_REQUEST,
                "INVALID_CHAIN_PAIR",
                format!(
                    "Invalid chain pair: funding={:?}, execution={:?}",
                    funding, execution
                ),
                Some(serde_json::json!({
                    "funding_chain": funding,
                    "execution_chain": execution,
                })),
            ),
            AppError::RiskControl(RiskError::DailyLimitExceeded { chain, current, limit }) => (
                StatusCode::TOO_MANY_REQUESTS,
                "DAILY_LIMIT_EXCEEDED",
                format!("Daily limit exceeded for {:?}", chain),
                Some(serde_json::json!({
                    "chain": chain,
                    "current": current,
                    "limit": limit,
                })),
            ),
            AppError::RiskControl(RiskError::CircuitBreakerTriggered { chain, reason }) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "CIRCUIT_BREAKER_TRIGGERED",
                format!("Service temporarily unavailable for {:?}: {}", chain, reason),
                Some(serde_json::json!({"chain": chain})),
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

impl From<Utf8Error> for AppError {
    fn from(error: Utf8Error) -> Self {
        AppError::Internal(format!("Error converting: {:?}", error))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        AppError::Internal(format!("Error converting: {:?}", error))
    }
}

impl From<ParseAccountError> for AppError {
    fn from(error: ParseAccountError) -> Self {
        AppError::Internal(format!("Error parsing to Account: {:?}", error))
    }
}

impl From<rust_decimal::Error> for AppError {
    fn from(error: rust_decimal::Error) -> Self {
        AppError::InvalidInput(format!("Decimal conversion error: {:?}", error))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        AppError::ExternalError(format!("HTTP request error: {:?}", error))
    }
}

impl From<MigrateError> for AppError {
    fn from(error: MigrateError) -> Self {
        AppError::Internal(format!("Migration error: {:?}", error))
    }
}

/// Approval-related errors
#[derive(Error, Debug)]
pub enum ApprovalError {
    #[error("Approval not found: {0}")]
    NotFound(String),

    #[error("Approval expired")]
    Expired,

    #[error("Nonce already used (replay attack prevented)")]
    NonceAlreadyUsed,

    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("Message tampering detected")]
    MessageTampering,

    #[error("Invalid approval status: {current}, expected: {expected}")]
    InvalidStatus { current: String, expected: String },

    #[error("Approval not yet confirmed")]
    NotConfirmed,

    #[error("Confirmation timeout")]
    ConfirmationTimeout,
}

impl From<ApprovalError> for AppError {
    fn from(error: ApprovalError) -> Self {
        AppError::Internal(error.to_string())
    }
}

/// Result type alias for the application
pub type AppResult<T> = Result<T, AppError>;