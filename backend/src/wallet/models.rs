use crate::ledger::models::Chain;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WalletStatus {
    Unverified,
    Verified,
    Suspended,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserWallet {
    pub id: Uuid,
    pub user_id: Uuid,
    pub chain: Chain,
    pub address: String,
    pub status: WalletStatus,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_synced: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletBalance {
    pub wallet_id: Uuid,
    pub token_address: String,
    pub token_symbol: String,
    pub balance: Decimal,
    pub balance_usd: Option<Decimal>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainBalance {
    pub chain: Chain,
    pub native_balance: Decimal,
    pub native_symbol: String,
    pub total_usd: Option<Decimal>,
    pub tokens: Vec<WalletBalance>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiChainWalletState {
    pub user_id: Uuid,
    pub wallets: Vec<UserWallet>,
    pub balances: Vec<ChainBalance>,
    pub total_portfolio_usd: Option<Decimal>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletVerificationRequest {
    pub chain: Chain,
    pub address: String,
    pub signature: String,
    pub message: String,
    pub public_key: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletVerificationResponse {
    pub wallet_id: Uuid,
    pub chain: Chain,
    pub address: String,
    pub verified: bool,
    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletBalanceSnapshot {
    pub wallet_id: Uuid,
    pub balances: Vec<TokenBalance>,
    pub snapshot_time: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenBalance {
    pub token_address: String,
    pub symbol: String,
    pub decimals: u32,
    pub amount: Decimal,
    pub usd_value: Option<Decimal>,
}

impl UserWallet {
    pub fn new(user_id: Uuid, chain: Chain, address: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            chain,
            address,
            status: WalletStatus::Unverified,
            verified_at: None,
            created_at: Utc::now(),
            last_synced: None,
        }
    }

    pub fn mark_verified(mut self) -> Self {
        self.status = WalletStatus::Verified;
        self.verified_at = Some(Utc::now());
        self
    }

    pub fn is_verified(&self) -> bool {
        self.status == WalletStatus::Verified
    }

    pub fn can_execute_trade(&self) -> bool {
        self.is_verified() && self.status != WalletStatus::Suspended
    }
}

impl ChainBalance {
    pub fn new(chain: Chain, native_symbol: String) -> Self {
        Self {
            chain,
            native_balance: Decimal::ZERO,
            native_symbol,
            total_usd: None,
            tokens: vec![],
            last_updated: Utc::now(),
        }
    }

    pub fn calculate_total_usd(&mut self) {
        let mut total = self.native_balance;
        for token in &self.tokens {
            if let Some(usd) = token.balance_usd {
                total += usd;
            }
        }
        self.total_usd = Some(total);
    }
}
