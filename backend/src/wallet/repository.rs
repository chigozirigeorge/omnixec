use crate::error::{AppError, AppResult};
use crate::ledger::models::Chain;
use crate::wallet::models::{UserWallet, WalletBalance, ChainBalance, WalletStatus};
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;

pub struct WalletRepository {
    // In production, this would be PostgreSQL via sqlx
    wallets: tokio::sync::RwLock<HashMap<Uuid, UserWallet>>,
    balances: tokio::sync::RwLock<HashMap<Uuid, Vec<WalletBalance>>>,
}

impl WalletRepository {
    pub fn new() -> Self {
        Self {
            wallets: tokio::sync::RwLock::new(HashMap::new()),
            balances: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    pub async fn register_wallet(&self, wallet: UserWallet) -> AppResult<UserWallet> {
        let mut wallets = self.wallets.write().await;
        wallets.insert(wallet.id, wallet.clone());
        Ok(wallet)
    }

    pub async fn get_wallet(&self, wallet_id: Uuid) -> AppResult<UserWallet> {
        let wallets = self.wallets.read().await;
        wallets
            .get(&wallet_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Wallet {} not found", wallet_id)))
    }

    pub async fn get_user_wallets(&self, user_id: Uuid) -> AppResult<Vec<UserWallet>> {
        let wallets = self.wallets.read().await;
        let user_wallets = wallets
            .values()
            .filter(|w| w.user_id == user_id)
            .cloned()
            .collect();
        Ok(user_wallets)
    }

    pub async fn get_wallet_by_address(
        &self,
        chain: Chain,
        address: &str,
    ) -> AppResult<Option<UserWallet>> {
        let wallets = self.wallets.read().await;
        Ok(wallets
            .values()
            .find(|w| w.chain == chain && w.address == address)
            .cloned())
    }

    pub async fn verify_wallet(&self, wallet_id: Uuid) -> AppResult<UserWallet> {
        let mut wallets = self.wallets.write().await;
        let wallet = wallets
            .get_mut(&wallet_id)
            .ok_or_else(|| AppError::NotFound(format!("Wallet {} not found", wallet_id)))?;

        wallet.status = WalletStatus::Verified;
        wallet.verified_at = Some(Utc::now());

        Ok(wallet.clone())
    }

    pub async fn set_wallet_balance(
        &self,
        wallet_id: Uuid,
        token_address: String,
        token_symbol: String,
        balance: Decimal,
    ) -> AppResult<WalletBalance> {
        let mut balances = self.balances.write().await;
        let wallet_balances = balances.entry(wallet_id).or_insert_with(Vec::new);

        // Update existing or insert new
        if let Some(existing) = wallet_balances.iter_mut().find(|b| b.token_address == token_address) {
            existing.balance = balance;
            existing.last_updated = Utc::now();
        } else {
            wallet_balances.push(WalletBalance {
                wallet_id,
                token_address: token_address.clone(),
                token_symbol,
                balance,
                balance_usd: None,
                last_updated: Utc::now(),
            });
        }

        Ok(wallet_balances
            .iter()
            .find(|b| b.token_address == token_address)
            .cloned()
            .unwrap())
    }

    pub async fn get_wallet_balances(&self, wallet_id: Uuid) -> AppResult<Vec<WalletBalance>> {
        let balances = self.balances.read().await;
        Ok(balances
            .get(&wallet_id)
            .cloned()
            .unwrap_or_default())
    }

    pub async fn get_user_balance_by_chain(
        &self,
        user_id: Uuid,
        chain: Chain,
    ) -> AppResult<ChainBalance> {
        let wallets = self.wallets.read().await;
        let user_wallet = wallets
            .values()
            .find(|w| w.user_id == user_id && w.chain == chain)
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "No wallet found for user {} on {}",
                    user_id, chain
                ))
            })?;

        let balances = self.balances.read().await;
        let wallet_balances = balances.get(&user_wallet.id).cloned().unwrap_or_default();

        let native_symbol = match chain {
            Chain::Solana => "SOL",
            Chain::Stellar => "XLM",
            Chain::Near => "NEAR",
        };

        let mut chain_balance = ChainBalance::new(chain, native_symbol.to_string());
        chain_balance.tokens = wallet_balances;

        // Find native token balance
        if let Some(native) = chain_balance
            .tokens
            .iter()
            .find(|t| t.token_address == native_symbol.to_lowercase())
        {
            chain_balance.native_balance = native.balance;
        }

        chain_balance.calculate_total_usd();
        Ok(chain_balance)
    }

    pub async fn get_user_all_balances(&self, user_id: Uuid) -> AppResult<Vec<ChainBalance>> {
        let wallets = self.wallets.read().await;
        let user_wallets: Vec<_> = wallets
            .values()
            .filter(|w| w.user_id == user_id)
            .collect();

        let balances = self.balances.read().await;
        let mut all_balances = vec![];

        for wallet in user_wallets {
            let wallet_balances = balances.get(&wallet.id).cloned().unwrap_or_default();
            let native_symbol = match wallet.chain {
                Chain::Solana => "SOL",
                Chain::Stellar => "XLM",
                Chain::Near => "NEAR",
            };

            let mut chain_balance = ChainBalance::new(wallet.chain, native_symbol.to_string());
            chain_balance.tokens = wallet_balances;

            if let Some(native) = chain_balance
                .tokens
                .iter()
                .find(|t| t.token_address == native_symbol.to_lowercase())
            {
                chain_balance.native_balance = native.balance;
            }

            chain_balance.calculate_total_usd();
            all_balances.push(chain_balance);
        }

        Ok(all_balances)
    }

    pub async fn clear_all(&self) -> AppResult<()> {
        let mut wallets = self.wallets.write().await;
        let mut balances = self.balances.write().await;
        wallets.clear();
        balances.clear();
        Ok(())
    }
}
