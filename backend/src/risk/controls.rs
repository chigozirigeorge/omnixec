use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sqlx::types::BigDecimal;
use std::{
    collections::HashMap, str::FromStr, sync::Arc
};
use chrono::Utc;
use tracing::{error, warn};
use crate::{
    error::{AppResult, RiskError},
    ledger::{models::*, repository::LedgerRepository}
};

/// Risk control configuration
#[derive(Debug, Clone)]
pub struct RiskConfig {
    /// per chain daily spending limits
    pub daily_limits: HashMap<Chain, Decimal>,
    ///Hourly outflow threashold (percentage of treasury)
    pub hourly_outflow_threashold: Decimal,
    /// Max consecutive failures before circuit breaker
    pub max_consecutive_failures: i32,
    ///Enable circuit breaker
    pub circuit_breaker_enabled: bool,
}

impl Default for RiskConfig {
    fn default() -> Self {
        let mut daily_limits = HashMap::new();
        daily_limits.insert(Chain::Near, dec!(10_000));
        daily_limits.insert(Chain::Solana, dec!(100));
        daily_limits.insert(Chain::Stellar, dec!(1_000_000));

        Self { 
            daily_limits, 
            hourly_outflow_threashold: dec!(0.2), 
            max_consecutive_failures: 5, 
            circuit_breaker_enabled: true 
        }
    }
}


pub struct RiskController {
    config: RiskConfig,
    ledger: Arc<LedgerRepository>,
}

impl RiskController {
    pub fn new(config: RiskConfig, ledger: Arc<LedgerRepository>) -> Self {
        Self { config, ledger }
    }

    
    fn get_chain_daily_limit(&self, chain: Chain) -> Decimal {
        self.config
            .daily_limits
            .get(&chain)
            .copied()
            .unwrap_or(dec!(1_000_000))
    }

    /// Public method to get daily limit for a chain
    pub async fn get_daily_limit(&self, chain: Chain) -> AppResult<Decimal> {
        Ok(self.get_chain_daily_limit(chain))
    }

    ///to get chain daily spending limit
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
                transaction_count: 0
            });

            let limit = self.get_chain_daily_limit(chain);
            let new_total = spending.amount_spent + amount;

            if new_total > limit {
                warn!(
                    "Daily limit exceeded for {:?}: {} + {} > {}",
                    chain, spending.amount_spent, amount, limit
                );

                self.ledger
                    .log_audit_event(
                        AuditEventType::LimitExceeded, 
                        Some(chain), 
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

                return  Err(
                    RiskError::DailyLimitExceeded {
                         chain, 
                         current: spending.amount_spent.to_string(), 
                         limit: limit.to_string()
                        }
                        .into());
            }

            Ok(())
    }

    /// Check if execution is allowed under current risk controls
    pub async fn check_execution_allowed(&self, chain: Chain, amount: Decimal) -> AppResult<()> {
        // check circuit breaker
        if self.config.circuit_breaker_enabled {
            if let Some(breaker) = self.ledger.get_active_circuit_breaker(chain).await? {
                error!(
                    "Circuit breaker is active for {:?}: {} (trigerred at: {})",
                    chain, breaker.reason, breaker.triggered_at
                );
                return  Err(RiskError::CircuitBreakerTriggered { 
                    chain, 
                    reason: breaker.reason 
                }.into());
            }
        }

        // Check daily spending limit
        self.check_daily_limit(chain, amount).await?;

        Ok(())
    }

    pub async fn record_spending(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        chain: Chain,
        amount: Decimal
    ) -> AppResult<()> {
        let today = Utc::now().date_naive();

        self.ledger
            .increment_daily_spending(
                tx, 
                chain, 
                today, 
                BigDecimal::from_str(&amount.to_string()).unwrap()
            )
            .await?;

        Ok(())
    }

    pub async fn trigger_circuit_breaker(&self, chain: Chain, reason: String) -> AppResult<()> {
        error!("Triggering circuit breaker for {:?}: {}", chain, reason.clone());

        let state = self.ledger.trigger_circuit_breaker(chain, reason.clone()).await?;

        self.ledger
            .log_audit_event(
                AuditEventType::CircuitBreakerTriggered, 
                Some(chain), 
                Some(state.id), 
                None, 
                serde_json::json!({
                    "chain": chain,
                    "reason": reason,
                    "triggered_at": state.triggered_at,
                }),
            )
            .await?;

        Ok(())
    }


  }