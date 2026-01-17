/// User transaction approval and spending allowance system
/// This manages user signatures, spending approvals, and transaction execution permissions

use crate::ledger::models::*;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rust_decimal::Decimal;

/// Represents a user's signed approval for a specific transaction
/// User signs this on their device to authorize spending
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingApproval {
    /// Unique approval ID
    pub id: Uuid,
    
    /// User initiating the approval
    pub user_id: Uuid,
    
    /// Chain where funds will be spent from
    pub funding_chain: Chain,
    
    /// Amount user approves to spend (in base units of funding_asset)
    pub approved_amount: Decimal,
    
    /// Amount for platform fees (included in approved_amount)
    pub fee_amount: Decimal,
    
    /// Amount for gas fees (included in approved_amount)
    pub gas_amount: Decimal,
    
    /// Actual amount going to execution (approved_amount - fee_amount - gas_amount)
    pub execution_amount: Decimal,
    
    /// Asset being spent (e.g., "SOL", "XLM", "NEAR")
    pub asset: String,
    
    /// Quote ID this approval is for
    pub quote_id: Uuid,
    
    /// User's wallet address on funding chain
    pub wallet_address: String,
    
    /// User's signature of the transaction (chain-specific format)
    /// For Solana: Base58-encoded signature
    /// For Stellar: XDR-encoded signature
    /// For NEAR: Base64-encoded signature
    pub user_signature: String,
    
    /// Treasury address where funds should be sent
    pub treasury_address: String,
    
    /// When this approval was created
    pub created_at: DateTime<Utc>,
    
    /// When this approval expires (default: 5 minutes)
    pub expires_at: DateTime<Utc>,
    
    /// Whether this approval has been used
    pub is_used: bool,
    
    /// Nonce for replay protection
    pub nonce: String,
}

impl SpendingApproval {
    /// Check if approval is still valid (not expired and not used)
    pub fn is_valid(&self) -> bool {
        !self.is_used && Utc::now() < self.expires_at
    }

    /// Create a new spending approval (unsigned)
    pub fn new(
        user_id: Uuid,
        funding_chain: Chain,
        approved_amount: Decimal,
        fee_amount: Decimal,
        gas_amount: Decimal,
        asset: String,
        quote_id: Uuid,
        wallet_address: String,
        treasury_address: String,
    ) -> Self {
        let now = Utc::now();
        
        SpendingApproval {
            id: Uuid::new_v4(),
            user_id,
            funding_chain,
            approved_amount,
            fee_amount,
            gas_amount,
            execution_amount: approved_amount - fee_amount - gas_amount,
            asset,
            quote_id,
            wallet_address,
            user_signature: String::new(), // Will be filled by user signature
            treasury_address,
            created_at: now,
            expires_at: now + Duration::minutes(5),
            is_used: false,
            nonce: format!("{}-{}", Uuid::new_v4(), now.timestamp_millis()),
        }
    }

    /// Set the user's signature
    pub fn with_signature(mut self, signature: String) -> Self {
        self.user_signature = signature;
        self
    }

    /// Mark this approval as used
    pub fn mark_as_used(&mut self) {
        self.is_used = true;
    }
}

/// Request to create a spending approval
#[derive(Debug, Deserialize)]
pub struct CreateSpendingApprovalRequest {
    /// Quote ID to create approval for
    pub quote_id: Uuid,
    
    /// Total amount user is approving (in base units)
    pub approved_amount: String, // String to handle large decimals
    
    /// User's wallet address
    pub wallet_address: String,
}

/// Request to submit a signed approval
#[derive(Debug, Deserialize)]
pub struct SubmitSignedApprovalRequest {
    /// Approval ID to sign
    pub approval_id: Uuid,
    
    /// User's signature (chain-specific encoding)
    pub signature: String,
}

/// Response with approval details and signing instructions
#[derive(Debug, Serialize)]
pub struct SpendingApprovalResponse {
    pub id: Uuid,
    pub quote_id: Uuid,
    pub approved_amount: String,
    pub fee_amount: String,
    pub gas_amount: String,
    pub execution_amount: String,
    pub asset: String,
    pub funding_chain: String,
    pub wallet_address: String,
    pub treasury_address: String,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
    
    /// What the user needs to sign (chain-specific format)
    pub message_to_sign: String,
}

impl SpendingApprovalResponse {
    pub fn from(approval: &SpendingApproval) -> Self {
        let message_to_sign = format!(
            "Approve spending {} {} from {} to {}. Amount: {}. Fee: {}. Gas: {}. Nonce: {}",
            approval.approved_amount,
            approval.asset,
            approval.wallet_address,
            approval.treasury_address,
            approval.approved_amount,
            approval.fee_amount,
            approval.gas_amount,
            approval.nonce
        );

        SpendingApprovalResponse {
            id: approval.id,
            quote_id: approval.quote_id,
            approved_amount: approval.approved_amount.to_string(),
            fee_amount: approval.fee_amount.to_string(),
            gas_amount: approval.gas_amount.to_string(),
            execution_amount: approval.execution_amount.to_string(),
            asset: approval.asset.clone(),
            funding_chain: format!("{:?}", approval.funding_chain),
            wallet_address: approval.wallet_address.clone(),
            treasury_address: approval.treasury_address.clone(),
            nonce: approval.nonce.clone(),
            expires_at: approval.expires_at,
            message_to_sign,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spending_approval_creation() {
        let approval = SpendingApproval::new(
            Uuid::new_v4(),
            Chain::Solana,
            Decimal::from(1_000_000),
            Decimal::from(1_000),
            Decimal::from(5_000),
            "SOL".to_string(),
            Uuid::new_v4(),
            "wallet123".to_string(),
            "treasury123".to_string(),
        );

        assert!(approval.is_valid());
        assert!(!approval.is_used);
        assert_eq!(approval.execution_amount, Decimal::from(994_000));
    }

    #[test]
    fn test_spending_approval_expiration() {
        let mut approval = SpendingApproval::new(
            Uuid::new_v4(),
            Chain::Solana,
            Decimal::from(1_000_000),
            Decimal::from(1_000),
            Decimal::from(5_000),
            "SOL".to_string(),
            Uuid::new_v4(),
            "wallet123".to_string(),
            "treasury123".to_string(),
        );

        approval.expires_at = Utc::now() - Duration::minutes(1);
        assert!(!approval.is_valid());
    }

    #[test]
    fn test_spending_approval_marked_used() {
        let mut approval = SpendingApproval::new(
            Uuid::new_v4(),
            Chain::Solana,
            Decimal::from(1_000_000),
            Decimal::from(1_000),
            Decimal::from(5_000),
            "SOL".to_string(),
            Uuid::new_v4(),
            "wallet123".to_string(),
            "treasury123".to_string(),
        );

        approval.mark_as_used();
        assert!(!approval.is_valid());
    }
}
