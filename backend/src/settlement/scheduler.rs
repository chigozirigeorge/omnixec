// Settlement Scheduler - handles daily wallet refilling
//
// Daily Strategy (RECOMMENDED):
// - Executes at 02:00 UTC (off-peak hours)
// - Aggregates all pending settlements
// - Single large transfer per blockchain
// - Lower fees, more efficient
// - Easier reconciliation
//
// Alternative: Use Hourly if volume > 1000 USD/hour

use chrono::{DateTime, TimeZone, Utc};
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};
use tracing::{error, info};
use std::sync::Arc;
use uuid::Uuid;
use crate::settlement::refiller::WalletRefiller;

/// Settlement schedule configuration
#[derive(Debug, Clone)]
pub struct SettlementScheduleConfig {
    /// Settlement frequency: "daily" or "hourly"
    pub frequency: SettlementFrequency,
    /// UTC hour to execute settlement (0-23)
    pub execution_hour: u32,
    /// Minimum amount to trigger settlement (in USD)
    pub min_settlement_amount: String,
    /// Enabled for each chain
    pub solana_enabled: bool,
    pub stellar_enabled: bool,
    pub near_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementFrequency {
    Daily,
    Hourly,
}

/// Settlement execution result
#[derive(Debug, Clone)]
pub struct SettlementResult {
    pub settlement_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub chain: String,
    pub amount: String,
    pub transaction_hash: String,
    pub status: SettlementStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

/// Settlement scheduler - coordinates daily/hourly wallet refilling
pub struct SettlementScheduler {
    config: SettlementScheduleConfig,
    refiller: Arc<WalletRefiller>,
}

impl SettlementScheduler {
    pub fn new(config: SettlementScheduleConfig, refiller: Arc<WalletRefiller>) -> Self {
        Self { config, refiller }
    }

    /// Start the settlement scheduler (runs in background)
    pub fn start(&self) -> JoinHandle<()> {
        let config = self.config.clone();
        let refiller = self.refiller.clone();

        tokio::spawn(async move {
            match config.frequency {
                SettlementFrequency::Daily => {
                    Self::run_daily_scheduler(&config, &refiller).await
                }
                SettlementFrequency::Hourly => {
                    Self::run_hourly_scheduler(&config, &refiller).await
                }
            }
        })
    }

    /// Daily scheduler - runs once per day at configured hour
    async fn run_daily_scheduler(
        config: &SettlementScheduleConfig,
        refiller: &Arc<WalletRefiller>,
    ) {
        loop {
            // Calculate time until next execution
            let now = Utc::now();
            let next_execution = Self::calculate_next_daily_execution(now, config.execution_hour);
            let duration_until_execution = next_execution.signed_duration_since(now);

            if duration_until_execution.num_seconds() > 0 {
                info!(
                    "⏰ Next settlement scheduled for: {} UTC",
                    next_execution.format("%H:%M:%S")
                );

                tokio::time::sleep(Duration::from_secs(
                    duration_until_execution.num_seconds() as u64,
                ))
                .await;
            }

            // Execute settlement
            info!("🔄 Starting daily settlement cycle");

            if config.solana_enabled {
                if let Err(e) = refiller.execute_solana_settlement().await {
                    error!("❌ Solana settlement failed: {:?}", e);
                }
            }

            if config.stellar_enabled {
                if let Err(e) = refiller.execute_stellar_settlement().await {
                    error!("❌ Stellar settlement failed: {:?}", e);
                }
            }

            if config.near_enabled {
                if let Err(e) = refiller.execute_near_settlement().await {
                    error!("❌ NEAR settlement failed: {:?}", e);
                }
            }

            info!("✓ Settlement cycle completed");
        }
    }

    /// Hourly scheduler - runs every hour
    async fn run_hourly_scheduler(
        config: &SettlementScheduleConfig,
        refiller: &Arc<WalletRefiller>,
    ) {
        // Check every hour
        let mut interval = interval(Duration::from_secs(3600));

        loop {
            interval.tick().await;

            info!("🔄 Starting hourly settlement cycle");

            if config.solana_enabled {
                if let Err(e) = refiller.execute_solana_settlement().await {
                    error!("❌ Solana settlement failed: {:?}", e);
                }
            }

            if config.stellar_enabled {
                if let Err(e) = refiller.execute_stellar_settlement().await {
                    error!("❌ Stellar settlement failed: {:?}", e);
                }
            }

            if config.near_enabled {
                if let Err(e) = refiller.execute_near_settlement().await {
                    error!("❌ NEAR settlement failed: {:?}", e);
                }
            }

            info!("✓ Hourly settlement cycle completed");
        }
    }

    /// Calculate next daily execution time
    fn calculate_next_daily_execution(now: DateTime<Utc>, execution_hour: u32) -> DateTime<Utc> {
        let mut next = now
            .date_naive()
            .and_hms_opt(execution_hour, 0, 0)
            .unwrap();
        let next_dt = Utc.from_utc_datetime(&next);

        // If execution time has passed today, schedule for tomorrow
        if next_dt <= now {
            next = (now.date_naive() + chrono::Duration::days(1))
                .and_hms_opt(execution_hour, 0, 0)
                .unwrap();
            Utc.from_utc_datetime(&next)
        } else {
            next_dt
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Timelike};

    use super::*;

    #[test]
    fn test_calculate_next_daily_execution() {
        // Current time: 2024-01-01 10:00:00 UTC
        let now = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();

        // Execution hour: 14:00 (today)
        let next = SettlementScheduler::calculate_next_daily_execution(now, 14);
        assert_eq!(next.hour(), 14);
        assert_eq!(next.day(), 1);

        // Execution hour: 09:00 (already passed, so tomorrow)
        let next = SettlementScheduler::calculate_next_daily_execution(now, 9);
        assert_eq!(next.hour(), 9);
        assert_eq!(next.day(), 2);
    }
}
