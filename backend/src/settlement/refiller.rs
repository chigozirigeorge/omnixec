// Wallet Refiller - executes the actual settlement transfers
// 
// Daily Settlement Flow:
// 1. Query all pending settlements from ledger
// 2. Aggregate by blockchain and token
// 3. Calculate refill amounts needed
// 4. Execute transfers to treasury wallets
// 5. Record settlement in ledger
// 6. Send notification to ops team

use crate::{error::AppResult, execution::{near::NearExecutor, stellar::StellarExecutor, solana::SolanaExecutor, router::Executor}};
use crate::ledger::repository::LedgerRepository;
use std::sync::Arc;
use tracing::{info, warn};
use rust_decimal::Decimal;

/// Wallet refiller - manages treasury wallet replenishment
pub struct WalletRefiller {
    ledger: Arc<LedgerRepository>,
    solana_executor: Option<Arc<SolanaExecutor>>,
    stellar_executor: Option<Arc<StellarExecutor>>,
    near_executor: Option<Arc<NearExecutor>>,
}

impl WalletRefiller {
    pub fn new(ledger: Arc<LedgerRepository>) -> Self {
        Self {
            ledger,
            solana_executor: None,
            stellar_executor: None,
            near_executor: None,
        }
    }

    pub fn with_solana(mut self, executor: Arc<SolanaExecutor>) -> Self {
        self.solana_executor = Some(executor);
        self
    }

    pub fn with_stellar(mut self, executor: Arc<StellarExecutor>) -> Self {
        self.stellar_executor = Some(executor);
        self
    }

    pub fn with_near(mut self, executor: Arc<NearExecutor>) -> Self {
        self.near_executor = Some(executor);
        self
    }

    // ========== SOLANA SETTLEMENT ==========

    pub async fn execute_solana_settlement(&self) -> AppResult<()> {
        info!("🔄 Executing Solana settlement");

        // 1. Get pending settlements from ledger
        let pending = self.ledger.get_pending_solana_settlements().await?;

        if pending.is_empty() {
            info!("✓ No Solana settlements pending");
            return Ok(());
        }

        info!("📊 Found {} Solana settlements", pending.len());

        // 2. Aggregate by token
        let aggregated = Self::aggregate_by_token(&pending);

        // 3. Execute transfers
        for (token_mint, total_amount) in aggregated {
            // Check if refill is needed
            if total_amount < Decimal::new(10, 2) {
                info!("⏭️ Solana {} settlement amount too small: {}", token_mint, total_amount);
                continue;
            }

            // Record settlement in ledger
            if let Some(executor) = &self.solana_executor {
                let tx_hash = executor
                    .as_ref()
                    .transfer_to_treasury(&token_mint, &total_amount.to_string())
                    .await?;

                info!(
                    "✓ Solana settlement executed: {} {} (tx: {})",
                    total_amount, token_mint, tx_hash
                );

                // Record settlement
                self.ledger
                    .record_settlement("solana", &token_mint, &total_amount.to_string(), &tx_hash)
                    .await?;
            } else {
                warn!("⚠️ Solana executor not configured, skipping settlement");
            }
        }

        Ok(())
    }

    // ========== STELLAR SETTLEMENT ==========

    pub async fn execute_stellar_settlement(&self) -> AppResult<()> {
        info!("🔄 Executing Stellar settlement");

        // 1. Get pending settlements
        let pending = self.ledger.get_pending_stellar_settlements().await?;

        if pending.is_empty() {
            info!("✓ No Stellar settlements pending");
            return Ok(());
        }

        info!("📊 Found {} Stellar settlements", pending.len());

        // 2. Aggregate by asset
        let aggregated = Self::aggregate_by_token(&pending);

        // 3. Execute transfers
        for (asset_code, total_amount) in aggregated {
            if total_amount < Decimal::new(10, 2) {
                info!("⏭️ Stellar {} settlement amount too small: {}", asset_code, total_amount);
                continue;
            }

            // Record settlement in ledger
            if let Some(executor) = &self.stellar_executor {
                let tx_hash = executor
                    .as_ref()
                    .transfer_to_treasury(&asset_code, &total_amount.to_string())
                    .await?;

                info!(
                    "✓ Stellar settlement executed: {} {} (tx: {})",
                    total_amount, asset_code, tx_hash
                );

                self.ledger
                    .record_settlement("stellar", &asset_code, &total_amount.to_string(), &tx_hash)
                    .await?;
            } else {
                warn!("⚠️ Stellar executor not configured, skipping settlement");
            }
        }

        Ok(())
    }

    // ========== NEAR SETTLEMENT ==========

    pub async fn execute_near_settlement(&self) -> AppResult<()> {
        info!("🔄 Executing NEAR settlement");

        // 1. Get pending settlements
        let pending = self.ledger.get_pending_near_settlements().await?;

        if pending.is_empty() {
            info!("✓ No NEAR settlements pending");
            return Ok(());
        }

        info!("📊 Found {} NEAR settlements", pending.len());

        // 2. Aggregate by token
        let aggregated = Self::aggregate_by_token(&pending);

        // 3. Execute transfers
        for (token_contract, total_amount) in aggregated {
            if total_amount < Decimal::new(10, 2) {
                info!("⏭️ NEAR {} settlement amount too small: {}", token_contract, total_amount);
                continue;
            }

            // Record settlement in ledger
            if let Some(executor) = &self.near_executor {
                let tx_hash = executor
                    .as_ref()
                    .transfer_to_treasury(&token_contract, &total_amount.to_string())
                    .await?;

                info!(
                    "✓ NEAR settlement executed: {} {} (tx: {})",
                    total_amount, token_contract, tx_hash
                );

                self.ledger
                    .record_settlement("near", &token_contract, &total_amount.to_string(), &tx_hash)
                    .await?;
            } else {
                warn!("⚠️ NEAR executor not configured, skipping settlement");
            }
        }

        Ok(())
    }

    // ========== HELPER FUNCTIONS ==========

    /// Aggregate settlement amounts by token/asset
    fn aggregate_by_token(settlements: &[(String, Decimal)]) -> std::collections::HashMap<String, Decimal> {
        let mut aggregated = std::collections::HashMap::new();

        for (token, amount) in settlements {
            *aggregated.entry(token.clone()).or_insert_with(|| Decimal::ZERO) += amount;
        }

        aggregated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_by_token() {
        let settlements = vec![
            ("USDC".to_string(), Decimal::new(100, 2)),
            ("USDC".to_string(), Decimal::new(200, 2)),
            ("USDT".to_string(), Decimal::new(150, 2)),
        ];

        let result = WalletRefiller::aggregate_by_token(&settlements);

        assert_eq!(result.get("USDC"), Some(&Decimal::new(300, 2)));
        assert_eq!(result.get("USDT"), Some(&Decimal::new(150, 2)));
    }
}
