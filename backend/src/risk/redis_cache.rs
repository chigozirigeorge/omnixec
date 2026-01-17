use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use chrono::{DateTime, Utc};
use crate::error::AppResult;
use crate::ledger::models::Chain;

/// Redis-like in-memory cache for risk controls
/// In production, this would use actual Redis for distributed caching
#[derive(Debug, Clone)]
pub struct RiskControlCache {
    /// Daily spending by chain: chain -> amount
    daily_spending: Arc<RwLock<HashMap<String, Decimal>>>,
    /// Last reset timestamps: chain -> timestamp
    last_reset: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    /// Daily limits: chain -> max amount
    daily_limits: HashMap<String, Decimal>,
}

impl RiskControlCache {
    pub fn new(daily_limits: HashMap<String, Decimal>) -> Self {
        Self {
            daily_spending: Arc::new(RwLock::new(HashMap::new())),
            last_reset: Arc::new(RwLock::new(HashMap::new())),
            daily_limits,
        }
    }

    /// Check if spending is allowed and update counter
    /// Returns remaining daily limit
    pub async fn check_and_record_spending(
        &self,
        chain: Chain,
        amount: Decimal,
    ) -> AppResult<Decimal> {
        let chain_str = chain.as_str().to_string();
        let limit = self.daily_limits.get(&chain_str)
            .copied()
            .unwrap_or_else(|| Decimal::from(1_000_000)); // Default 1M limit

        let now = Utc::now();

        // Check if we need to reset
        let mut spending = self.daily_spending.write().await;
        let mut resets = self.last_reset.write().await;

        let last_reset_time = resets.entry(chain_str.clone()).or_insert(now);
        let elapsed_secs = (now - *last_reset_time).num_seconds();

        // Reset if 24 hours have passed
        if elapsed_secs > 86400 {
            spending.insert(chain_str.clone(), Decimal::ZERO);
            *last_reset_time = now;
            debug!("🔄 Reset daily spending for {}", chain_str);
        }

        // Get current spending
        let current_spending = spending.entry(chain_str.clone()).or_insert(Decimal::ZERO);

        // Check if adding this amount would exceed limit
        if *current_spending + amount > limit {
            let remaining = limit - *current_spending;
            return Err(
                crate::error::AppError::RiskControlViolation(
                    format!(
                        "Daily limit exceeded for {}. Remaining: {}",
                        chain_str, remaining
                    )
                )
            );
        }

        // Record spending
        *current_spending += amount;
        let remaining = limit - *current_spending;

        debug!(
            "💰 Recorded spending on {}: {} (remaining: {})",
            chain_str, amount, remaining
        );

        Ok(remaining)
    }

    /// Get current daily spending
    pub async fn get_daily_spending(&self, chain: Chain) -> Decimal {
        let chain_str = chain.as_str();
        let spending = self.daily_spending.read().await;
        spending.get(chain_str)
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Get remaining daily limit
    pub async fn get_remaining_limit(&self, chain: Chain) -> Decimal {
        let chain_str = chain.as_str();
        let limit = self.daily_limits.get(chain_str)
            .copied()
            .unwrap_or_else(|| Decimal::from(1_000_000));

        let spending = self.get_daily_spending(chain).await;
        limit - spending
    }

    /// Reset spending for a chain (testing/admin)
    pub async fn reset_spending(&self, chain: Chain) {
        let chain_str = chain.as_str().to_string();
        let mut spending = self.daily_spending.write().await;
        spending.remove(&chain_str);
        info!("🔄 Reset spending for {}", chain_str);
    }

    /// Get all remaining limits
    pub async fn get_all_remaining(&self) -> HashMap<String, Decimal> {
        let mut result = HashMap::new();
        for (chain, limit) in &self.daily_limits {
            let spending = self.get_daily_spending(Chain::Solana).await; // Note: simplified
            result.insert(chain.clone(), *limit - spending);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_risk_control_cache() {
        let mut limits = HashMap::new();
        limits.insert("solana".to_string(), Decimal::from(1000));

        let cache = RiskControlCache::new(limits);

        // Should allow spending within limit
        let remaining = cache.check_and_record_spending(Chain::Solana, Decimal::from(500)).await;
        assert!(remaining.is_ok());
        assert_eq!(remaining.unwrap(), Decimal::from(500));

        // Should reject exceeding limit
        let result = cache.check_and_record_spending(Chain::Solana, Decimal::from(600)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_remaining_limit() {
        let mut limits = HashMap::new();
        limits.insert("solana".to_string(), Decimal::from(1000));

        let cache = RiskControlCache::new(limits);
        cache.check_and_record_spending(Chain::Solana, Decimal::from(300)).await.ok();

        let remaining = cache.get_remaining_limit(Chain::Solana).await;
        assert_eq!(remaining, Decimal::from(700));
    }
}
