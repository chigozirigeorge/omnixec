# Settlement Layer Architecture & Implementation

Complete guide to wallet refilling, daily settlement, and ledger reconciliation.

---

## 🏛️ Settlement Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    SETTLEMENT LAYER                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  User Quote → Execution → Webhook → Settlement              │
│                                         ↓                    │
│                              SettlementScheduler             │
│                              (Daily at 02:00 UTC)            │
│                                         ↓                    │
│                              WalletRefiller                  │
│                              (Aggregate & Send)              │
│                                         ↓                    │
│                    Solana │ Stellar │ NEAR                   │
│                    (Chains)                                  │
│                                         ↓                    │
│                           Treasury Wallets                   │
│                           (Refilled Daily)                   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 📊 Daily Settlement vs Hourly Settlement

### DAILY SETTLEMENT (RECOMMENDED) ✅

**When**: 02:00 UTC (off-peak hours)
**Why**: 
- Lower transaction fees during off-peak
- Aggregates multiple transfers into one
- Easier to reconcile and audit
- Reduces API rate limit hits
- Better for cash management

**Process**:
1. 23:00 UTC - 02:00 UTC: Collect all pending settlements
2. 02:00 UTC: Aggregate by blockchain and token
3. 02:05 UTC: Execute single large transfer per blockchain
4. 02:30 UTC: Verify on-chain settlement
5. 03:00 UTC: Update ledger and send notifications

**Cost Example** (for 1000 payments/day):
```
Daily consolidation:
- 1 Solana tx: $0.00025
- 1 Stellar tx: $0.00
- 1 NEAR tx: $0.0005
- Total: ~$0.001/day = $0.30/month

Hourly consolidation:
- 24 Solana tx: $0.006
- 24 Stellar tx: $0.00
- 24 NEAR tx: $0.012
- Total: ~$0.018/day = $5.40/month

Savings: 18x cheaper!
```

### HOURLY SETTLEMENT (High Volume Only)

**When**: Every hour (top of hour)
**Why**:
- Necessary if volume > 1000 USD/hour
- Faster wallet refilling
- Smaller transaction amounts

**Cost**: 18x more expensive
**Complexity**: Higher state management

**Recommendation**: Switch to hourly only if:
- Volume exceeds 1000 USD/hour consistently
- Wallets frequently run low
- Time-critical settlement needed

---

## 🔄 Daily Settlement Implementation

### Step 1: Database Schema for Settlements

```sql
-- Track all settlement records
CREATE TABLE settlements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    execution_time TIMESTAMP WITH TIME ZONE NOT NULL,
    chain VARCHAR(50) NOT NULL, -- 'solana', 'stellar', 'near'
    token VARCHAR(100) NOT NULL, -- 'USDC', 'USDT', etc
    total_amount DECIMAL(20, 8) NOT NULL,
    transaction_hash VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- 'pending', 'confirmed', 'failed'
    confirmed_at TIMESTAMP WITH TIME ZONE,
    error_message TEXT
);

-- Track what executions were settled
CREATE TABLE settlement_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    settlement_id UUID NOT NULL,
    execution_id UUID NOT NULL,
    amount DECIMAL(20, 8) NOT NULL,
    settled_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    FOREIGN KEY (settlement_id) REFERENCES settlements(id),
    FOREIGN KEY (execution_id) REFERENCES executions(id)
);

-- Daily settlement summary
CREATE TABLE settlement_summaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    settlement_date DATE NOT NULL UNIQUE,
    total_settled DECIMAL(20, 8),
    transaction_count INTEGER,
    total_fees DECIMAL(20, 8),
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_settlements_chain ON settlements(chain);
CREATE INDEX idx_settlements_status ON settlements(status);
CREATE INDEX idx_settlements_created_at ON settlements(created_at);
```

### Step 2: Extend Ledger Repository

```rust
// In src/ledger/repository.rs

impl LedgerRepository {
    /// Get pending settlements for Solana (not yet settled)
    pub async fn get_pending_solana_settlements(&self) -> AppResult<Vec<(String, Decimal)>> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                e.token,
                SUM(CAST(e.amount AS DECIMAL(20, 8))) as amount
            FROM executions e
            WHERE e.chain = 'solana'
                AND e.status = 'Executed'
                AND NOT EXISTS (
                    SELECT 1 FROM settlement_executions se
                    WHERE se.execution_id = e.id
                )
            GROUP BY e.token
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter()
            .map(|row| (row.token, row.amount))
            .collect())
    }

    /// Same for Stellar and NEAR
    pub async fn get_pending_stellar_settlements(&self) -> AppResult<Vec<(String, Decimal)>> {
        // Similar query for Stellar
        Ok(vec![])
    }

    pub async fn get_pending_near_settlements(&self) -> AppResult<Vec<(String, Decimal)>> {
        // Similar query for NEAR
        Ok(vec![])
    }

    /// Record settlement in ledger
    pub async fn record_settlement(
        &self,
        chain: &str,
        token: &str,
        amount: &str,
        tx_hash: &str,
    ) -> AppResult<Uuid> {
        let settlement_id = Uuid::new_v4();
        let amount_decimal: Decimal = amount.parse()?;

        sqlx::query(
            r#"
            INSERT INTO settlements (id, chain, token, total_amount, transaction_hash, status)
            VALUES ($1, $2, $3, $4, $5, 'pending')
            "#
        )
        .bind(&settlement_id)
        .bind(chain)
        .bind(token)
        .bind(&amount_decimal)
        .bind(tx_hash)
        .execute(&self.pool)
        .await?;

        info!("✓ Settlement recorded: {} {} on {}", amount, token, chain);
        Ok(settlement_id)
    }

    /// Mark executions as settled
    pub async fn mark_executions_settled(
        &self,
        settlement_id: &Uuid,
        execution_ids: &[Uuid],
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;

        for exec_id in execution_ids {
            sqlx::query(
                "INSERT INTO settlement_executions (settlement_id, execution_id, amount) 
                 SELECT $1, id, amount FROM executions WHERE id = $2"
            )
            .bind(settlement_id)
            .bind(exec_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
```

### Step 3: Configure in main.rs

```rust
// In src/main.rs

use crate::settlement::{SettlementScheduler, SettlementScheduleConfig, WalletRefiller};

#[tokio::main]
async fn main() -> Result<()> {
    // ... existing setup ...

    // Initialize settlement layer
    let refiller = Arc::new(
        WalletRefiller::new(ledger.clone())
            .with_solana(solana_executor.clone())
            .with_stellar(stellar_executor.clone())
            .with_near(near_executor.clone())
    );

    // Configure daily settlement at 02:00 UTC
    let settlement_config = SettlementScheduleConfig {
        frequency: SettlementFrequency::Daily,
        execution_hour: 2, // 02:00 UTC
        min_settlement_amount: "10".to_string(), // $10 minimum
        solana_enabled: true,
        stellar_enabled: true,
        near_enabled: true,
    };

    let scheduler = SettlementScheduler::new(settlement_config, refiller);
    
    // Start scheduler in background
    scheduler.start();

    // ... rest of application ...
    
    Ok(())
}
```

---

## 💰 Settlement Flow Example

### Scenario: 100 USDC payments received on Solana

**Timeline**:

```
09:00 - User 1 submits quote: 50 USDC Solana → Ethereum
        ✓ Execution created
        ✓ Webhook received (settlement pending)

10:30 - User 2 submits quote: 50 USDC Solana → Stellar
        ✓ Execution created
        ✓ Webhook received (settlement pending)

02:00 UTC (SETTLEMENT TIME)
        ✓ Scheduler wakes up
        ✓ Query: "Get all pending Solana USDC settlements"
        → Result: 100 USDC (50 + 50)
        ✓ Aggregate: 100 USDC total
        ✓ Execute single transaction: 100 USDC to treasury
        ✓ TX Hash: 0xABC123...
        ✓ Update ledger with settlement record
        ✓ Mark both executions as settled
        ✓ Send notification: "Settlement completed: 100 USDC"

02:05 - Verification
        ✓ Poll blockchain: confirm 100 USDC received
        ✓ Update settlement status: "confirmed"
        ✓ Log in settlement_summaries

03:00 - Next day accounting
        ✓ Generate settlement report
        ✓ Reconcile with blockchain data
        ✓ Prepare for financial reconciliation
```

---

## 🔒 Security Considerations

### 1. Multi-Signature Settlement Wallets

For production, treasury should use multi-sig:

```rust
// Solana: 2-of-3 multisig
// Stellar: 2-of-3 multisig
// NEAR: 2-of-3 multisig

// Each requires 2 of 3 signers:
Signer 1: Operations Team
Signer 2: Finance Team
Signer 3: CEO/CRO
```

### 2. Daily Limit per Settlement

```rust
pub struct SettlementLimits {
    pub max_daily_amount: Decimal,
    pub max_per_transaction: Decimal,
    pub min_confirmations: u32,
}

// Check before executing
if total_amount > limits.max_daily_amount {
    return Err("Daily settlement limit exceeded");
}
```

### 3. Audit Trail

```sql
-- Every settlement is logged and immutable
CREATE TABLE settlement_audit_log (
    id UUID PRIMARY KEY,
    settlement_id UUID NOT NULL,
    action VARCHAR(100), -- 'created', 'signed', 'submitted', 'confirmed', 'failed'
    actor VARCHAR(100), -- who authorized
    timestamp TIMESTAMP WITH TIME ZONE,
    
    FOREIGN KEY (settlement_id) REFERENCES settlements(id)
);
```

### 4. Failure Handling

```rust
pub async fn handle_settlement_failure(
    &self,
    settlement_id: &Uuid,
    error: &str,
) -> AppResult<()> {
    // Log failure
    self.ledger.mark_settlement_failed(settlement_id, error).await?;
    
    // Send alert to ops team
    self.notification_queue.queue_notification(
        ops_user_id,
        NotificationChannel::Email,
        NotificationPriority::High,
        "Settlement Failed: Manual intervention needed",
    ).await?;
    
    // Freeze settlements until resolved
    self.ledger.pause_settlements().await?;
    
    Ok(())
}
```

---

## 📈 Monitoring Settlement

### Key Metrics

```
Settlement Dashboard:
- Daily settlements: count
- Daily volume: USD
- Success rate: %
- Average time to settlement: minutes
- Failed settlements: count
- Recovery rate: % of failed recovered
- Transaction costs: USD
```

### Alert Rules

```yaml
alerts:
  - name: settlement_pending_too_long
    threshold: pending_count > 0 after 03:00 UTC
    action: page_oncall

  - name: settlement_failure
    threshold: settlement.status == 'failed'
    action: alert_cto

  - name: settlement_cost_spike
    threshold: daily_cost > avg * 2
    action: investigate

  - name: low_treasury_balance
    threshold: treasury < min_threshold
    action: alert_operations
```

---

## 🧪 Testing Settlement

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_daily_settlement_scheduling() {
        let config = SettlementScheduleConfig {
            frequency: SettlementFrequency::Daily,
            execution_hour: 2,
            min_settlement_amount: "10".to_string(),
            solana_enabled: true,
            stellar_enabled: true,
            near_enabled: true,
        };

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        let next = SettlementScheduler::calculate_next_daily_execution(now, 2);
        
        assert_eq!(next.hour(), 2);
    }

    #[tokio::test]
    async fn test_settlement_aggregation() {
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
```

### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_settlement() {
    // 1. Create quote and execution
    // 2. Simulate webhook receipt
    // 3. Trigger settlement
    // 4. Verify blockchain transaction
    // 5. Verify ledger updated
}
```

---

## 🚀 Deployment Checklist

- [ ] Settlement database schema created
- [ ] Ledger repository extended with settlement methods
- [ ] SettlementScheduler implemented
- [ ] WalletRefiller implemented
- [ ] Multi-sig wallets configured
- [ ] Settlement limits defined
- [ ] Monitoring dashboard set up
- [ ] Alert rules configured
- [ ] Audit logging enabled
- [ ] Failure handling tested
- [ ] Recovery procedures documented
- [ ] Team trained on settlement ops

---

**Next Steps**:
1. Implement daily execution time configuration
2. Add settlement retry logic with exponential backoff
3. Create settlement dashboard for operations team
4. Set up automated reconciliation with blockchain data
