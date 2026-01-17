use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Type, prelude::FromRow};
use uuid::Uuid;
use std::primitive::str;
use std::fmt;

use crate::error::{AppError, AppResult};

/// Universal Chain enum - used everywhere in the system
/// Any chain can be funding OR execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(type_name = "chain_type", rename_all = "lowercase")]
pub enum Chain{
    Solana,
    Stellar,
    Near
}

impl fmt::Display for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Chain {
    pub fn as_str(&self) -> &'static str {
        match self {
            Chain::Stellar => "stellar",
            Chain::Solana => "solana",
            Chain::Near => "near",
        }
    }

    /// Return all supported chains
    pub fn all() -> Vec<Chain> {
        vec![Chain::Solana, Chain::Stellar, Chain::Near]
    }

    /// Check if this chain pair is supported for cross-chain execution
    pub fn is_pair_supported(funding: Chain, execution: Chain) -> bool {
       // SECURITY: Explicit whitelist of supported pairs
     // this would be configurable
        if funding == execution {
            //Same-chain not allowed
            return false; 
        }

        match (funding, execution) {
            //All combinations that are currently supported
            (Chain::Stellar, Chain::Solana) => true,
            (Chain::Stellar, Chain::Near) => true,
            (Chain::Near, Chain::Stellar) => true,
            (Chain::Near, Chain::Solana) => true,
            (Chain::Solana, Chain::Stellar) => true,
            (Chain::Solana, Chain::Near) => true,
            _ => false,
        }

    }

}

/// Asset representation (chain-specific)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub chain: Chain,
    pub symbol: String,
    /// Asset address/identifier (e.g, token mint for solana)
    pub address: Option<String>,
}


impl Asset {
    /// Native asset for a chain
    pub fn native(chain: Chain) -> Self {
        let symbol = match chain {
            Chain::Solana => "SOL",
            Chain::Stellar => "XLM",
            Chain::Near => "NEAR",
        };

        Self { 
            chain, 
            symbol: symbol.to_string(), 
            address: None
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
    Settled
}

///Execution status enum

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "execution_status", rename_all = "lowercase")]
pub enum ExecutionStatus {
    Pending,
    Success,
    Failed,
}


///User entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub solana_address: Option<String>,
    pub stellar_address: Option<String>,
    pub near_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

///Balance entity (per chain, per asset)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub user_id: Uuid,
    pub chain: Chain,
    pub asset: String,

    #[serde(with = "rust_decimal::serde::float")]
    pub amount: rust_decimal::Decimal,

    #[serde(with = "rust_decimal::serde::float")]
    pub locked_amount: rust_decimal::Decimal,
    pub updated_at: DateTime<Utc>,
}

impl Balance {
    pub fn available(&self) -> rust_decimal::Decimal {
        self.amount - self.locked_amount
    }

    pub fn has_avaliable(&self, required: rust_decimal::Decimal) -> bool {
        self.available() >= required
    }
}


/// Quote entity - representing a symmentric cross-chain execution quote
/// 
/// Critical INVARIANT: funding_chain != execution_chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub id: Uuid,
    pub user_id: Uuid,

    //Symmnetric chain pair
    pub funding_chain: Chain,
    pub execution_chain: Chain,

    //Assets
    pub funding_asset: String,
    pub execution_asset: String,

    //Amounts
    #[serde(with = "rust_decimal::serde::float")]
    pub max_funding_amount: rust_decimal::Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub execution_cost: rust_decimal::Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub service_fee: rust_decimal::Decimal,

    //Execution payload (chain-agnostic)
    pub execution_instructions: Vec<u8>,
    pub estimated_compute_units: Option<i32>,

    //Metadata
    pub nonce: String,
    pub status: QuoteStatus,
    pub expires_at: DateTime<Utc>,
    pub payment_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>, 

}


impl Quote {
    ///Check if quote is still valid
    pub fn is_valid(&self) -> bool {
        self.status == QuoteStatus::Pending && self.expires_at > Utc::now()
    }

    /// check if quote can be commited
    pub fn can_commit(&self) -> bool {
        self.is_valid()
    }

    /// Check if quote has expired
    pub fn is_expired(&self) -> bool {
        self.expires_at <= Utc::now()
    }

    /// Check if quote can be executed
    pub fn can_execute(&self) -> bool {
        self.status == QuoteStatus::Committed && !self.is_expired()
    }

    /// Verify funding and execution chains are different
    pub fn has_valid_chain_pair(&self) -> bool {
        self.funding_chain != self.execution_chain 
            && Chain::is_pair_supported(self.funding_chain, self.execution_chain)
    }

    /// Total amount user must pay on funding chain
    pub fn total_funding_required(&self) -> rust_decimal::Decimal {
        self.max_funding_amount + self.service_fee
    }
}


///Execution entity - represents execution on any chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub id: Uuid,
    pub quote_id: Uuid,
    
    // Which chain it was executed on
    pub execution_chain: Chain,

    //Chain specific transaction identifier
    pub transaction_hash: Option<String>,

    pub status: ExecutionStatus,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub gas_used: Option<rust_decimal::Decimal>,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub executed_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}


///Settlement entity - records the funding chain payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settlement {
    pub id: Uuid,
    pub execution_id: Uuid,

    //Which chain was funding from
    pub funding_chain: Chain,
    pub funding_txn_hash: String,
    #[serde(with = "rust_decimal::serde::float")]
    pub funding_amount: rust_decimal::Decimal,
    pub settled_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}


///Treasury balance entity (per chain)
#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct TreasuryBalance {
    pub chain: Chain,
    pub asset: String,
    #[serde(with = "rust_decimal::serde::float")]
    pub balance: rust_decimal::Decimal,
    pub last_updated: DateTime<Utc>,
    pub last_reconciled: Option<DateTime<Utc>>,
}


///Daily spending tracking (per chain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySpending {
    pub chain: Chain,
    pub date: NaiveDate,
    #[serde(with = "rust_decimal::serde::float")]
    pub amount_spent: rust_decimal::Decimal,
    pub transaction_count: i32,
}


///Audit event type
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
     pub chain: Option<Chain>,
     pub entity_id: Option<Uuid>,
     pub user_id: Option<Uuid>,
     pub details: serde_json::Value,
     pub created_at: DateTime<Utc>,
}

/// Circuit breaker state (per chain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    pub id: Uuid,
    pub chain: Chain,
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

/// Token Approval entity - represents a user-signed approval for token transfer
/// This uses the signature verification pattern instead of manual transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenApproval {
    pub id: Uuid,
    pub quote_id: Uuid,
    pub user_id: Uuid,
    pub funding_chain: Chain,
    pub token: String,
    
    #[serde(with = "rust_decimal::serde::float")]
    pub amount: rust_decimal::Decimal,
    pub recipient: String,
    
    pub message: String,
    pub nonce: String,
    pub signature: Option<String>,
    pub user_wallet: String,
    
    pub status: String, // "pending", "signed", "submitted", "confirmed", "executed", "failed", "expired", "cancelled"
    pub transaction_hash: Option<String>,
    pub block_height: Option<i64>,
    pub confirmation_status: Option<String>,
    
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub retry_count: i32,
    pub last_retry_at: Option<DateTime<Utc>>,
}

impl TokenApproval {
    /// Create from database row
    pub fn from_row(row: &sqlx::postgres::PgRow) -> AppResult<Self> {
        use sqlx::Row;
        use std::str::FromStr;
        
        let amount_str: String = row.try_get("amount")?;
        let amount = rust_decimal::Decimal::from_str(&amount_str)
            .map_err(|_| AppError::InvalidInput("Invalid amount format".to_string()))?;
        
        let chain_str: String = row.try_get("funding_chain")?;
        let funding_chain = match chain_str.as_str() {
            "solana" => Chain::Solana,
            "stellar" => Chain::Stellar,
            "near" => Chain::Near,
            _ => return Err(AppError::InvalidInput("Invalid chain".to_string())),
        };
        
        Ok(TokenApproval {
            id: row.try_get("id")?,
            quote_id: row.try_get("quote_id")?,
            user_id: row.try_get("user_id")?,
            funding_chain,
            token: row.try_get("token")?,
            amount,
            recipient: row.try_get("recipient")?,
            message: row.try_get("message")?,
            nonce: row.try_get("nonce")?,
            signature: row.try_get("signature")?,
            user_wallet: row.try_get("user_wallet")?,
            status: row.try_get("status")?,
            transaction_hash: row.try_get("transaction_hash")?,
            block_height: row.try_get("block_height")?,
            confirmation_status: row.try_get("confirmation_status")?,
            created_at: row.try_get("created_at")?,
            expires_at: row.try_get("expires_at")?,
            submitted_at: row.try_get("submitted_at")?,
            confirmed_at: row.try_get("confirmed_at")?,
            executed_at: row.try_get("executed_at")?,
            failed_at: row.try_get("failed_at")?,
            error_message: row.try_get("error_message")?,
            error_code: row.try_get("error_code")?,
            retry_count: row.try_get("retry_count")?,
            last_retry_at: row.try_get("last_retry_at")?,
        })
    }
}

impl TokenApproval {
    /// Check if approval is still valid (not expired and pending)
    pub fn is_valid(&self) -> bool {
        self.status == "pending" && Utc::now() < self.expires_at
    }

    /// Check if approval has been signed
    pub fn is_signed(&self) -> bool {
        self.status == "signed" && self.signature.is_some()
    }

    /// Check if approval has been confirmed on-chain
    pub fn is_confirmed(&self) -> bool {
        self.status == "confirmed" && self.confirmed_at.is_some()
    }

    /// Check if approval has been executed
    pub fn is_executed(&self) -> bool {
        self.status == "executed" && self.executed_at.is_some()
    }
}

/// Request to create a new token approval
#[derive(Debug, Deserialize)]
pub struct CreateTokenApprovalRequest {
    pub quote_id: Uuid,
    pub user_id: Uuid,
    pub funding_chain: String,
    pub token: String,
    pub amount: String,
    pub recipient: String,
}

/// Response when approval is created (contains message to sign)
#[derive(Debug, Serialize)]
pub struct CreateTokenApprovalResponse {
    pub approval_id: Uuid,
    pub message_to_sign: String,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
}

/// Request to submit a signed approval
#[derive(Debug, Deserialize)]
pub struct SubmitTokenApprovalRequest {
    pub approval_id: Uuid,
    pub user_wallet: String,
    pub signature: String,
    pub message: String,
    pub nonce: String,
}

/// Response when approval is submitted and executed
#[derive(Debug, Serialize)]
pub struct SubmitTokenApprovalResponse {
    pub approval_id: Uuid,
    pub status: String,
    pub transaction_hash: String,
    pub confirmation_status: String,
    pub estimated_confirmation_time: u32,
}

/// Response for approval status checks
#[derive(Debug, Serialize)]
pub struct TokenApprovalStatusResponse {
    pub approval_id: Uuid,
    pub status: String,
    pub transaction_hash: Option<String>,
    pub confirmation_status: Option<String>,
    pub block_height: Option<i64>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum ApprovalStatus {
    Pending,
    Signed,
    Submitted,
    Confirmed,
    Executed,
    Failed,
    Expired,
    Cancelled,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApprovalStatus::Pending => "pending",
            ApprovalStatus::Signed => "signed",
            ApprovalStatus::Submitted => "submitted",
            ApprovalStatus::Confirmed => "confirmed",
            ApprovalStatus::Executed => "executed",
            ApprovalStatus::Failed => "failed",
            ApprovalStatus::Expired => "expired",
            ApprovalStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => ApprovalStatus::Pending,
            "signed" => ApprovalStatus::Signed,
            "submitted" => ApprovalStatus::Submitted,
            "confirmed" => ApprovalStatus::Confirmed,
            "executed" => ApprovalStatus::Executed,
            "failed" => ApprovalStatus::Failed,
            "expired" => ApprovalStatus::Expired,
            "cancelled" => ApprovalStatus::Cancelled,
            _ => ApprovalStatus::Pending,
        }
    }
}





// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_chain_pair_validation() {
//         // Same chain should not be supported
//         assert!(!Chain::is_pair_supported(Chain::Solana, Chain::Solana));
//         assert!(!Chain::is_pair_supported(Chain::Stellar, Chain::Stellar));
//         assert!(!Chain::is_pair_supported(Chain::Near, Chain::Near));

//         // Different chains should be supported
//         assert!(Chain::is_pair_supported(Chain::Stellar, Chain::Solana));
//         assert!(Chain::is_pair_supported(Chain::Solana, Chain::Stellar));
//         assert!(Chain::is_pair_supported(Chain::Near, Chain::Solana));
//         assert!(Chain::is_pair_supported(Chain::Solana, Chain::Near));
//     }

//     #[test]
//     fn test_quote_chain_validation() {
//         let mut quote = Quote {
//             id: Uuid::new_v4(),
//             user_id: Uuid::new_v4(),
//             funding_chain: Chain::Stellar,
//             execution_chain: Chain::Solana,
//             funding_asset: "XLM".to_string(),
//             execution_asset: "SOL".to_string(),
//             max_funding_amount: rust_decimal::Decimal::new(1000000, 0),
//             execution_cost: rust_decimal::Decimal::new(1000000, 0),
//             service_fee: rust_decimal::Decimal::new(1000, 0),
//             execution_instructions: vec![],
//             estimated_compute_units: None,
//             nonce: "test".to_string(),
//             status: QuoteStatus::Pending,
//             expires_at: Utc::now() + chrono::Duration::minutes(5),
//             payment_address: None,
//             created_at: Utc::now(),
//             updated_at: Utc::now(),
//         };

//         // Valid different chains
//         assert!(quote.has_valid_chain_pair());

//         // Same chain should be invalid
//         quote.execution_chain = Chain::Stellar;
//         assert!(!quote.has_valid_chain_pair());
//     }

//     #[test]
//     fn test_native_assets() {
//         assert_eq!(Asset::native(Chain::Solana).symbol, "SOL");
//         assert_eq!(Asset::native(Chain::Stellar).symbol, "XLM");
//         assert_eq!(Asset::native(Chain::Near).symbol, "NEAR");
//     }
// }
