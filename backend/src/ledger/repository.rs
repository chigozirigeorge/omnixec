use super::models::*;
use crate::error::{AppError, AppResult, ExecutionError, QuoteError};
use sqlx::types::BigDecimal;
use rust_decimal::Decimal;
use std::str::FromStr;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use tracing::info;

/// Ledger repository - THE source of truth for all state
pub struct LedgerRepository {
    pub pool: PgPool,
}

impl LedgerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ========== USER OPERATIONS ==========

    pub async fn create_user(
        &self,
        solana_address: Option<String>,
        stellar_address: Option<String>,
        near_address: Option<String>,
    ) -> AppResult<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (solana_address, stellar_address, near_address)
            VALUES ($1, $2, $3)
            RETURNING id, solana_address, stellar_address, near_address, created_at, updated_at
            "#
        )
        .bind(solana_address)
        .bind(stellar_address)
        .bind(near_address)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn get_user_by_id(&self, user_id: Uuid) -> AppResult<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, solana_address, stellar_address, near_address, created_at, updated_at
            FROM users
            WHERE id = $1
            "#
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Alias for get_user_by_id - retrieves user by ID
    pub async fn get_user(&self, user_id: Uuid) -> AppResult<Option<User>> {
        self.get_user_by_id(user_id).await
    }

    // ========== BALANCE OPERATIONS ==========

    pub async fn get_balance(
        &self,
        user_id: Uuid,
        chain: Chain,
        asset: &str,
    ) -> AppResult<Option<Balance>> {
        let balance = sqlx::query!(
            r#"
            SELECT user_id, chain as "chain: Chain", asset, amount, locked_amount, updated_at
            FROM balances
            WHERE user_id = $1 AND chain = $2 AND asset = $3
            "#,
            user_id,
            chain as Chain,
            asset
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|row| {
            let amount = Decimal::from_str(&row.amount.to_string()).ok()?;
            let locked_amount = Decimal::from_str(&row.locked_amount.to_string()).ok()?;
            Some(Balance {
                user_id: row.user_id,
                chain: row.chain,
                asset: row.asset,
                amount,
                locked_amount,
                updated_at: row.updated_at,
            })
        }).flatten();

        Ok(balance)
    }

    pub async fn lock_funds(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        chain: Chain,
        asset: &str,
        amount: BigDecimal,
    ) -> AppResult<()> {
        let result = sqlx::query!(
            r#"
            UPDATE balances
            SET locked_amount = locked_amount + $4
            WHERE user_id = $1 AND chain = $2 AND asset = $3 AND (amount - locked_amount) >= $4
            "#,
            user_id,
            chain as Chain,
            asset,
            BigDecimal::from_str(&amount.to_string()).unwrap()
        )
        .execute(&mut **tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(QuoteError::InsufficientFunds {
                required: amount.to_string(),
                available: "unknown".to_string(),
            }
            .into());
        }

        Ok(())
    }

    // ========== QUOTE OPERATIONS ==========

    /// Create a symmetric cross-chain quote
    pub async fn create_quote(
        &self,
        user_id: Uuid,
        funding_chain: Chain,
        execution_chain: Chain,
        funding_asset: String,
        execution_asset: String,
        max_funding_amount: BigDecimal,
        execution_cost: BigDecimal,
        service_fee: BigDecimal,
        execution_instructions: Vec<u8>,
        estimated_compute_units: Option<i32>,
        nonce: String,
        expires_at: chrono::DateTime<chrono::Utc>,
        payment_address: Option<String>,
    ) -> AppResult<Quote> {
        let quote = sqlx::query!(
            r#"
            INSERT INTO quotes (
                user_id, funding_chain, execution_chain, funding_asset, execution_asset,
                max_funding_amount, execution_cost, service_fee, execution_instructions,
                estimated_compute_units, nonce, expires_at, payment_address
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING 
                id, user_id,
                funding_chain as "funding_chain: Chain",
                execution_chain as "execution_chain: Chain",
                funding_asset, execution_asset,
                max_funding_amount, execution_cost, service_fee,
                execution_instructions, estimated_compute_units, nonce,
                status as "status: QuoteStatus", expires_at, payment_address,
                created_at, updated_at
            "#,
            user_id,
            funding_chain as Chain,
            execution_chain as Chain,
            funding_asset,
            execution_asset,
            BigDecimal::from_str(&max_funding_amount.to_string()).unwrap(),
            BigDecimal::from_str(&execution_cost.to_string()).unwrap(),
            BigDecimal::from_str(&service_fee.to_string()).unwrap(),
            execution_instructions,
            estimated_compute_units,
            nonce,
            expires_at,
            payment_address
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Quote {
            id: quote.id,
            user_id: quote.user_id,
            funding_chain: quote.funding_chain,
            execution_chain: quote.execution_chain,
            funding_asset: quote.funding_asset,
            execution_asset: quote.execution_asset,
            max_funding_amount: Decimal::from_str(&quote.max_funding_amount.to_string()).unwrap(),
            execution_cost: Decimal::from_str(&quote.execution_cost.to_string()).unwrap(),
            service_fee: Decimal::from_str(&quote.service_fee.to_string()).unwrap(),
            execution_instructions: quote.execution_instructions,
            estimated_compute_units: quote.estimated_compute_units,
            nonce: quote.nonce,
            status: quote.status,
            expires_at: quote.expires_at,
            payment_address: quote.payment_address,
            created_at: quote.created_at,
            updated_at: quote.updated_at,
        })
    }

    pub async fn get_quote(&self, quote_id: Uuid) -> AppResult<Option<Quote>> {
        let quote = sqlx::query!(
            r#"
            SELECT 
                id, user_id,
                funding_chain as "funding_chain: Chain",
                execution_chain as "execution_chain: Chain",
                funding_asset, execution_asset,
                max_funding_amount, execution_cost, service_fee,
                execution_instructions, estimated_compute_units, nonce,
                status as "status: QuoteStatus", expires_at, payment_address,
                created_at, updated_at
            FROM quotes
            WHERE id = $1
            "#,
            quote_id
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|row| {
            let max_funding_amount = Decimal::from_str(&row.max_funding_amount.to_string()).ok()?;
            let execution_cost = Decimal::from_str(&row.execution_cost.to_string()).ok()?;
            let service_fee = Decimal::from_str(&row.service_fee.to_string()).ok()?;
            Some(Quote {
                id: row.id,
                user_id: row.user_id,
                funding_chain: row.funding_chain,
                execution_chain: row.execution_chain,
                funding_asset: row.funding_asset,
                execution_asset: row.execution_asset,
                max_funding_amount,
                execution_cost,
                service_fee,
                execution_instructions: row.execution_instructions,
                estimated_compute_units: row.estimated_compute_units,
                nonce: row.nonce,
                status: row.status,
                expires_at: row.expires_at,
                payment_address: row.payment_address,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        }).flatten();

        Ok(quote)
    }

    pub async fn update_quote_status(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        quote_id: Uuid,
        from_status: QuoteStatus,
        to_status: QuoteStatus,
    ) -> AppResult<()> {
        // SECURITY: Validate state machine transitions
        Self::validate_state_transition(from_status, to_status)?;

        let result = sqlx::query!(
            r#"
            UPDATE quotes
            SET status = $3, updated_at = NOW()
            WHERE id = $1 AND status = $2
            "#,
            quote_id,
            from_status as QuoteStatus,
            to_status as QuoteStatus
        )
        .execute(&mut **tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(QuoteError::InvalidState {
                current: "unknown".to_string(),
                expected: format!("{:?}", from_status),
            }
            .into());
        }

        Ok(())
    }

    /// Validate quote status state machine transitions
    /// Valid transitions:
    /// - Pending → Committed, Expired
    /// - Committed → Executed, Failed, Expired
    /// - Executed → Settled, Failed
    /// - Terminal states (Settled, Failed, Expired) → NO TRANSITIONS ALLOWED
    fn validate_state_transition(from: QuoteStatus, to: QuoteStatus) -> AppResult<()> {
        let allowed_transitions = match from {
            QuoteStatus::Pending => vec![
                QuoteStatus::Committed,
                QuoteStatus::Expired,
            ],
            QuoteStatus::Committed => vec![
                QuoteStatus::Executed,
                QuoteStatus::Failed,
                QuoteStatus::Expired,
            ],
            QuoteStatus::Executed => vec![
                QuoteStatus::Settled,
                QuoteStatus::Failed,
            ],
            // Terminal states - no transitions allowed
            QuoteStatus::Settled | QuoteStatus::Expired | QuoteStatus::Failed => {
                return Err(QuoteError::InvalidState {
                    current: format!("{:?}", from),
                    expected: "No transitions from terminal states".to_string(),
                }
                .into());
            }
        };

        if !allowed_transitions.contains(&to) {
            return Err(QuoteError::InvalidState {
                current: format!("{:?}", from),
                expected: format!("{:?}", allowed_transitions),
            }
            .into());
        }

        Ok(())
    }

    /// Expire old pending quotes (TTL exceeded)
    pub async fn expire_old_pending_quotes(&self) -> AppResult<u64> {
        let result = sqlx::query!(
            r#"
            UPDATE quotes
            SET status = $2
            WHERE status = $1 AND expires_at < NOW()
            "#,
            QuoteStatus::Pending as QuoteStatus,
            QuoteStatus::Expired as QuoteStatus
        )
        .execute(&self.pool)
        .await?;
        
        Ok(result.rows_affected())
    }

    /// Expire old committed quotes (TTL exceeded while waiting for execution)
    pub async fn expire_old_committed_quotes(&self) -> AppResult<u64> {
        let result = sqlx::query!(
            r#"
            UPDATE quotes
            SET status = $2
            WHERE status = $1 AND expires_at < NOW()
            "#,
            QuoteStatus::Committed as QuoteStatus,
            QuoteStatus::Expired as QuoteStatus
        )
        .execute(&self.pool)
        .await?;
        
        Ok(result.rows_affected())
    }

    // ========== EXECUTION OPERATIONS ==========

    /// Create execution record with chain information
    pub async fn create_execution(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        quote_id: Uuid,
        execution_chain: Chain,
    ) -> AppResult<Execution> {
        let execution = sqlx::query!(
            r#"
            INSERT INTO executions (quote_id, execution_chain)
            VALUES ($1, $2)
            RETURNING 
                id, quote_id,
                execution_chain as "execution_chain: Chain",
                transaction_hash,
                status as "status: ExecutionStatus",
                gas_used, error_message, retry_count,
                executed_at, completed_at
            "#,
            quote_id,
            execution_chain as Chain
        )
        .fetch_one(&mut **tx)
        .await?;

        let gas_used = execution.gas_used.and_then(|g| Decimal::from_str(&g.to_string()).ok());
        Ok(Execution {
            id: execution.id,
            quote_id: execution.quote_id,
            execution_chain: execution.execution_chain,
            transaction_hash: execution.transaction_hash,
            status: execution.status,
            gas_used,
            error_message: execution.error_message,
            retry_count: execution.retry_count,
            executed_at: execution.executed_at,
            completed_at: execution.completed_at,
        })
    }

    pub async fn check_execution_exist(
        &self,
        execution_id: Uuid,
    ) -> AppResult<()> {
        let result = sqlx::query(
            r#"
            SELECT id FROM executions WHERE id = $1
            "#
        )
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(_) => Err(AppError::Execution(ExecutionError::DuplicateExecution)),
            None => Ok(()),
        }
    }

    pub async fn mark_pending(
        &self,
        execution_id: Uuid,
    ) -> AppResult<()> {
        let status = ExecutionStatus::Pending;

        sqlx::query(
            r#"
            UPDATE executions
            SET status = $2
            WHERE id = $1
            "#
        )
        .bind(execution_id)
        .bind(status as ExecutionStatus)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn complete_execution(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        execution_id: Uuid,
        status: ExecutionStatus,
        transaction_hash: Option<String>,
        gas_used: Option<BigDecimal>,
        error_message: Option<String>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE executions
            SET status = $2, transaction_hash = $3, gas_used = $4, 
                error_message = $5, completed_at = NOW()
            WHERE id = $1
            "#  
        )
        .bind(execution_id)
        .bind(status as ExecutionStatus)
        .bind(transaction_hash)
        .bind(gas_used.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()))
        .bind(error_message)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    // ========== SETTLEMENT OPERATIONS ==========

    pub async fn create_settlement(
        &self,
        execution_id: Uuid,
        funding_chain: Chain,
        funding_txn_hash: String,
        funding_amount: BigDecimal,
    ) -> AppResult<Settlement> {
        let settlement = sqlx::query!(
            r#"
            INSERT INTO settlements (execution_id, funding_chain, funding_txn_hash, funding_amount)
            VALUES ($1, $2, $3, $4)
            RETURNING 
                id, execution_id,
                funding_chain as "funding_chain: Chain",
                funding_txn_hash, funding_amount, settled_at, verified_at
            "#,
            execution_id,
            funding_chain as Chain,
            funding_txn_hash,
            funding_amount
        )
        .fetch_one(&self.pool)
        .await?;

        let funding_amount = Decimal::from_str(&settlement.funding_amount.to_string()).unwrap();
        Ok(Settlement {
            id: settlement.id,
            execution_id: settlement.execution_id,
            funding_chain: settlement.funding_chain,
            funding_txn_hash: settlement.funding_txn_hash,
            funding_amount,
            settled_at: settlement.settled_at,
            verified_at: settlement.verified_at,
        })
    }

    // ========== AUDIT LOG ==========

    pub async fn log_audit_event(
        &self,
        event_type: AuditEventType,
        chain: Option<Chain>,
        entity_id: Option<Uuid>,
        user_id: Option<Uuid>,
        details: serde_json::Value,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO audit_log (event_type, chain, entity_id, user_id, details)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            event_type as AuditEventType,
            chain as Option<Chain>,
            entity_id,
            user_id,
            details
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ========== DAILY SPENDING TRACKING ==========

    pub async fn get_daily_spending(
        &self,
        chain: Chain,
        date: chrono::NaiveDate,
    ) -> AppResult<Option<DailySpending>> {
        let spending = sqlx::query!(
            r#"
            SELECT chain as "chain: Chain", date, amount_spent, transaction_count
            FROM daily_spending
            WHERE chain = $1 AND date = $2
            "#,
            chain as Chain,
            date
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|row| {
            let amount_spent = Decimal::from_str(&row.amount_spent.to_string()).ok()?;
            Some(DailySpending {
                chain: row.chain,
                date: row.date,
                amount_spent,
                transaction_count: row.transaction_count,
            })
        }).flatten();

        Ok(spending)
    }

    pub async fn increment_daily_spending(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        chain: Chain,
        date: chrono::NaiveDate,
        amount: BigDecimal,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO daily_spending (chain, date, amount_spent, transaction_count)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT (chain, date)
            DO UPDATE SET
                amount_spent = daily_spending.amount_spent + EXCLUDED.amount_spent,
                transaction_count = daily_spending.transaction_count + 1
            "#,
            chain as Chain,
            date,
            BigDecimal::from_str(&amount.to_string()).unwrap()
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    // ========== CIRCUIT BREAKER ==========

    pub async fn get_active_circuit_breaker(
        &self,
        chain: Chain,
    ) -> AppResult<Option<CircuitBreakerState>> {
        let state = sqlx::query_as!(
            CircuitBreakerState,
            r#"
            SELECT id, chain as "chain: Chain", triggered_at, reason, resolved_at, resolved_by
            FROM circuit_breaker_state
            WHERE chain = $1 AND resolved_at IS NULL
            ORDER BY triggered_at DESC
            LIMIT 1
            "#,
            chain as Chain
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(state)
    }

    pub async fn trigger_circuit_breaker(
        &self,
        chain: Chain,
        reason: String,
    ) -> AppResult<CircuitBreakerState> {
        let state = sqlx::query_as!(
            CircuitBreakerState,
            r#"
            INSERT INTO circuit_breaker_state (chain, reason)
            VALUES ($1, $2)
            RETURNING id, chain as "chain: Chain", triggered_at, reason, resolved_at, resolved_by
            "#,
            chain as Chain,
            reason
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(state)
    }

    pub async fn begin_tx(&self) -> AppResult<Transaction<'_, Postgres>> {
        Ok(self.pool.begin().await?)
    }

    // ========== ASYNC WEBHOOK SUPPORT ==========

    /// Get execution by quote_id
    pub async fn get_execution_by_quote_id(&self, quote_id: &Uuid) -> AppResult<Execution> {
        let execution = sqlx::query!(
            r#"
            SELECT 
                id, quote_id,
                execution_chain as "execution_chain: Chain",
                transaction_hash, status as "status: ExecutionStatus",
                gas_used, error_message, retry_count,
                executed_at, completed_at
            FROM executions
            WHERE quote_id = $1
            LIMIT 1
            "#,
            quote_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Execution not found".into()))?;

        let gas_used = execution.gas_used.and_then(|g| Decimal::from_str(&g.to_string()).ok());
        Ok(Execution {
            id: execution.id,
            quote_id: execution.quote_id,
            execution_chain: execution.execution_chain,
            transaction_hash: execution.transaction_hash,
            status: execution.status,
            gas_used,
            error_message: execution.error_message,
            retry_count: execution.retry_count,
            executed_at: execution.executed_at,
            completed_at: execution.completed_at,
        })
    }

    /// Update execution with transaction hash and status
    pub async fn update_execution_hash(
        &self,
        execution_id: &Uuid,
        transaction_hash: &str,
        status: &str,
    ) -> AppResult<()> {
        let exec_status = match status {
            "success" | "completed" => ExecutionStatus::Success,
            "failed" | "error" => ExecutionStatus::Failed,
            _ => ExecutionStatus::Pending,
        };

        sqlx::query(
            r#"
            UPDATE executions
            SET transaction_hash = $2, status = $3, executed_at = NOW()
            WHERE id = $1
            "#
        )
        .bind(execution_id)
        .bind(transaction_hash)
        .bind(exec_status as ExecutionStatus)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update quote status by ID
    pub async fn update_quote_status_direct(
        &self,
        quote_id: &Uuid,
        to_status: QuoteStatus,
        transaction_hash: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE quotes
            SET status = $2
            WHERE id = $1
            "#
        )
        .bind(quote_id)
        .bind(to_status as QuoteStatus)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record settlement - insert into settlements table
    pub async fn record_settlement(
        &self,
        chain: &str,
        token: &str,
        amount: &str,
        transaction_hash: &str,
    ) -> AppResult<()> {
        let chain_enum = match chain.to_lowercase().as_str() {
            "solana" => Chain::Solana,
            "stellar" => Chain::Stellar,
            "near" => Chain::Near,
            _ => return Err(AppError::InvalidInput("Unknown chain".into())),
        };

        let amount_decimal = Decimal::from_str(amount)
            .map_err(|_| AppError::InvalidInput("Invalid amount".into()))?;

        sqlx::query(
            r#"
            INSERT INTO settlements (chain, token, total_amount, transaction_hash, status)
            VALUES ($1, $2, $3, $4, 'pending')
            ON CONFLICT (transaction_hash) DO NOTHING
            "#
        )
        .bind(chain_enum as Chain)
        .bind(token)
        .bind(BigDecimal::from_str(&amount_decimal.to_string()).unwrap())
        .bind(transaction_hash)
        .execute(&self.pool)
        .await?;

     // Treasury settlement is recorded implicitly through execution completion
        // The settlement refiller transfers funds to treasury as a maintenance operation
        // This does not directly map to the settlements table which tracks quote executions
        
        info!(
            "Treasury settlement recorded: {} {} on {} (tx: {})",
            amount, token, chain, transaction_hash
        );

        // TODO: Considering creating a separate treasury_transactions or refill_history table
        // to track treasury replenishment operations separately from quote execution settlements
        

        Ok(())
    }

    // ========== SETTLEMENT REFILLER SUPPORT ==========

    /// Get pending Solana settlements (not yet settled)
    /// Returns vector of (token, amount) tuples
    pub async fn get_pending_solana_settlements(&self) -> AppResult<Vec<(String, Decimal)>> {
        // Query settlements table for Solana chain settlements
        let rows = sqlx::query!(
            r#"
            SELECT 
                COALESCE('SOL', 'SOL') as token,
                SUM(CAST(funding_amount AS DECIMAL(20, 8))) as total_amount
            FROM settlements
            WHERE funding_chain = 'solana' AND verified_at IS NULL
            GROUP BY 1
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let result = rows.into_iter()
            .filter_map(|row| {
                let amount = Decimal::from_str(&row.total_amount.unwrap_or_default().to_string()).ok()?;
                Some((row.token.unwrap_or_else(|| "SOL".to_string()), amount))
            })
            .collect();

        Ok(result)
    }

    /// Get pending Stellar settlements
    pub async fn get_pending_stellar_settlements(&self) -> AppResult<Vec<(String, Decimal)>> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                COALESCE('XLM', 'XLM') as token,
                SUM(CAST(funding_amount AS DECIMAL(20, 8))) as total_amount
            FROM settlements
            WHERE funding_chain = 'stellar' AND verified_at IS NULL
            GROUP BY 1
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let result = rows.into_iter()
            .filter_map(|row| {
                let amount = Decimal::from_str(&row.total_amount.unwrap_or_default().to_string()).ok()?;
                Some((row.token.unwrap_or_else(|| "XLM".to_string()), amount))
            })
            .collect();

        Ok(result)
    }

    /// Get pending NEAR settlements
    pub async fn get_pending_near_settlements(&self) -> AppResult<Vec<(String, Decimal)>> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                COALESCE('NEAR', 'NEAR') as token,
                SUM(CAST(funding_amount AS DECIMAL(20, 8))) as total_amount
            FROM settlements
            WHERE funding_chain = 'near' AND verified_at IS NULL
            GROUP BY 1
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let result = rows.into_iter()
            .filter_map(|row| {
                let amount = Decimal::from_str(&row.total_amount.unwrap_or_default().to_string()).ok()?;
                Some((row.token.unwrap_or_else(|| "NEAR".to_string()), amount))
            })
            .collect();

        Ok(result)
    }

    // ========== SPENDING APPROVAL OPERATIONS ==========

    /// Create a new spending approval (unsigned, waiting for user signature)
    pub async fn create_spending_approval(
        &self,
        user_id: Uuid,
        quote_id: Uuid,
        funding_chain: Chain,
        asset: &str,
        approved_amount: BigDecimal,
        fee_amount: BigDecimal,
        gas_amount: BigDecimal,
        execution_amount: BigDecimal,
        wallet_address: &str,
        treasury_address: &str,
        nonce: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<Uuid> {
        let approval_id = Uuid::new_v4();
        
        sqlx::query!(
            r#"
            INSERT INTO spending_approvals (
                id, user_id, quote_id, funding_chain, asset,
                approved_amount, fee_amount, gas_amount, execution_amount,
                wallet_address, treasury_address, nonce, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
            approval_id,
            user_id,
            quote_id,
            funding_chain as Chain,
            asset,
            approved_amount,
            fee_amount,
            gas_amount,
            execution_amount,
            wallet_address,
            treasury_address,
            nonce,
            expires_at
        )
        .execute(&self.pool)
        .await?;

        Ok(approval_id)
    }

    /// Get spending approval by ID with full details
    pub async fn get_spending_approval(&self, approval_id: &Uuid) -> AppResult<Option<crate::api::spending_approval::SpendingApproval>> {
        let row = sqlx::query!(
            r#"
            SELECT 
                id, user_id, quote_id, funding_chain as "funding_chain: Chain",
                asset, approved_amount, fee_amount, gas_amount, execution_amount,
                wallet_address, treasury_address, user_signature,
                nonce, is_used, used_at, created_at, expires_at
            FROM spending_approvals
            WHERE id = $1
            "#,
            approval_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| crate::api::spending_approval::SpendingApproval {
            id: r.id,
            user_id: r.user_id,
            funding_chain: r.funding_chain,
            approved_amount: Decimal::from_str(&r.approved_amount.to_string()).unwrap_or_default(),
            fee_amount: Decimal::from_str(&r.fee_amount.to_string()).unwrap_or_default(),
            gas_amount: Decimal::from_str(&r.gas_amount.to_string()).unwrap_or_default(),
            execution_amount: Decimal::from_str(&r.execution_amount.to_string()).unwrap_or_default(),
            asset: r.asset,
            quote_id: r.quote_id,
            wallet_address: r.wallet_address,
            user_signature: r.user_signature.unwrap_or_default(),
            treasury_address: r.treasury_address,
            created_at: r.created_at,
            expires_at: r.expires_at,
            is_used: r.is_used,
            nonce: r.nonce,
        }))
    }

    /// Verify that approval is valid and mark it as used
    /// SECURITY: This is the critical authorization check
    /// 
    /// Verifies:
    /// 1. Approval exists
    /// 2. Not already used
    /// 3. Not expired
    /// 4. Signature is present (submitted)
    /// 5. Amount doesn't exceed daily limits
    pub async fn authorize_spending_approval(
        &self,
        approval_id: &Uuid,
    ) -> AppResult<(Uuid, Decimal)> {
        let mut tx = self.begin_tx().await?;

        // Get approval with FOR UPDATE lock to prevent race conditions
        let row = sqlx::query!(
            r#"
            SELECT 
                id, user_id, quote_id, approved_amount, is_used, 
                expires_at, user_signature, created_at
            FROM spending_approvals
            WHERE id = $1
            FOR UPDATE
            "#,
            approval_id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Approval not found: {}", approval_id)))?;

        // Check if already used
        if row.is_used {
            return Err(AppError::BadRequest("Approval already used".to_string()));
        }

        // Check if expired
        if chrono::Utc::now() > row.expires_at {
            return Err(AppError::BadRequest("Approval has expired".to_string()));
        }

        // Check if signature has been submitted
        if row.user_signature.is_none() || row.user_signature.as_ref().unwrap().is_empty() {
            return Err(AppError::InvalidSignature("Approval not signed yet".to_string()));
        }

        // Mark as used atomically
        sqlx::query!(
            r#"
            UPDATE spending_approvals
            SET is_used = true, used_at = NOW()
            WHERE id = $1
            "#,
            approval_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let amount = Decimal::from_str(&row.approved_amount.to_string()).unwrap_or_default();
        Ok((row.quote_id, amount))
    }

    /// Submit user signature for a spending approval
    /// SECURITY: Signature is stored but NOT yet used for authorization
    /// User must explicitly call authorize_spending_approval to use it
    pub async fn submit_spending_approval_signature(
        &self,
        approval_id: &Uuid,
        signature: &str,
    ) -> AppResult<()> {
        let row_count = sqlx::query!(
            r#"
            UPDATE spending_approvals
            SET user_signature = $2
            WHERE id = $1 AND is_used = false AND expires_at > NOW()
            "#,
            approval_id,
            signature
        )
        .execute(&self.pool)
        .await?
        .rows_affected();

        if row_count == 0 {
            return Err(AppError::BadRequest(
                "Approval not found, already used, or expired".to_string()
            ));
        }

        Ok(())
    }

    /// Check if user has valid token balance to cover spending approval
    /// Used by API to verify user can actually spend the approved amount
    pub async fn verify_approval_token_balance(
        &self,
        user_id: Uuid,
        funding_chain: Chain,
        asset: &str,
        amount_required: Decimal,
    ) -> AppResult<bool> {
        let balance = self.get_balance(user_id, funding_chain, asset).await?;

        Ok(balance
            .map(|b| b.available() >= amount_required)
            .unwrap_or(false))
    }

    /// Get all active approvals for a user (not used, not expired)
    pub async fn get_active_approvals_for_user(
        &self,
        user_id: Uuid,
    ) -> AppResult<Vec<(Uuid, Uuid)>> {
        // Returns (approval_id, quote_id) tuples
        let rows = sqlx::query!(
            r#"
            SELECT id, quote_id
            FROM spending_approvals
            WHERE user_id = $1
              AND is_used = false
              AND expires_at > NOW()
            ORDER BY created_at DESC
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| (r.id, r.quote_id)).collect())
    }

    /// List all spending approvals for a user (both active and inactive)
    pub async fn list_user_spending_approvals(
        &self,
        user_id: Uuid,
    ) -> AppResult<Vec<crate::api::spending_approval::SpendingApproval>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, user_id, funding_chain::TEXT, approved_amount, fee_amount, gas_amount, execution_amount,
                   asset, quote_id, wallet_address, treasury_address, nonce,
                   is_used, user_signature, created_at, expires_at
            FROM spending_approvals
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        let approvals = rows
            .into_iter()
            .map(|r| {
                crate::api::spending_approval::SpendingApproval {
                    id: r.id,
                    user_id: r.user_id,
                    funding_chain: match r.funding_chain.as_deref().unwrap_or("stellar") {
                        "solana" => crate::ledger::models::Chain::Solana,
                        "near" => crate::ledger::models::Chain::Near,
                        _ => crate::ledger::models::Chain::Stellar,
                    },
                    approved_amount: Decimal::from_str(&r.approved_amount.to_string()).unwrap_or(Decimal::ZERO),
                    fee_amount: Decimal::from_str(&r.fee_amount.to_string()).unwrap_or(Decimal::ZERO),
                    gas_amount: Decimal::from_str(&r.gas_amount.to_string()).unwrap_or(Decimal::ZERO),
                    execution_amount: Decimal::from_str(&r.execution_amount.to_string()).unwrap_or(Decimal::ZERO),
                    asset: r.asset,
                    quote_id: r.quote_id,
                    wallet_address: r.wallet_address,
                    treasury_address: r.treasury_address,
                    is_used: r.is_used,
                    user_signature: r.user_signature.unwrap_or_default(),
                    created_at: r.created_at,
                    expires_at: r.expires_at,
                    nonce: r.nonce,
                }
            })
            .collect();

        Ok(approvals)
    }

    /// Get all settlement records for a quote
    pub async fn get_quote_settlements(
        &self,
        quote_id: Uuid,
    ) -> AppResult<Vec<serde_json::Value>> {
        // Get the execution associated with this quote, then get its settlement
        let rows = sqlx::query!(
            r#"
            SELECT s.id, s.funding_chain::TEXT, s.funding_txn_hash, s.funding_amount, s.settled_at, s.verified_at
            FROM settlements s
            INNER JOIN executions e ON s.execution_id = e.id
            WHERE e.quote_id = $1
            ORDER BY s.settled_at DESC
            "#,
            quote_id
        )
        .fetch_all(&self.pool)
        .await?;

        let settlements = rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "settlement_id": r.id.to_string(),
                    "chain": r.funding_chain,
                    "transaction_hash": r.funding_txn_hash,
                    "amount": r.funding_amount.to_string(),
                    "settled_at": r.settled_at.to_rfc3339(),
                    "verified_at": r.verified_at.map(|d| d.to_rfc3339()),
                })
            })
            .collect();

        Ok(settlements)
    }

    // ========== TOKEN APPROVAL OPERATIONS ==========

    /// Check if nonce has been used before (prevents replay attacks)
    pub async fn is_nonce_used(&self, nonce: &str) -> AppResult<bool> {
        let result = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM token_approvals WHERE nonce = $1 AND status != 'expired')"
        )
        .bind(nonce)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get token approval by ID
    pub async fn get_token_approval(&self, approval_id: &Uuid) -> AppResult<TokenApproval> {
        let row = sqlx::query(
            "SELECT * FROM token_approvals WHERE id = $1"
        )
        .bind(approval_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| AppError::NotFound(format!("Approval not found: {}", approval_id)))?;

        TokenApproval::from_row(&row)
    }

    /// Create new token approval (unsigned)
    pub async fn create_token_approval(&self, approval: &TokenApproval) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO token_approvals (
                id, quote_id, user_id, funding_chain, token, amount, recipient,
                message, nonce, user_wallet, status, created_at, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#
        )
        .bind(approval.id)
        .bind(approval.quote_id)
        .bind(approval.user_id)
        .bind(approval.funding_chain.as_str())
        .bind(&approval.token)
        .bind(approval.amount.to_string())
        .bind(&approval.recipient)
        .bind(&approval.message)
        .bind(&approval.nonce)
        .bind(&approval.user_wallet)
        .bind(&approval.status)
        .bind(approval.created_at)
        .bind(approval.expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update approval with signature and submission details
    pub async fn update_token_approval_submitted(
        &self,
        approval_id: &Uuid,
        signature: &str,
        tx_hash: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE token_approvals 
            SET signature = $1, status = 'submitted', transaction_hash = $2, submitted_at = NOW()
            WHERE id = $3
            "#
        )
        .bind(signature)
        .bind(tx_hash)
        .bind(approval_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark token approval as confirmed
    pub async fn update_token_approval_confirmed(
        &self,
        approval_id: &Uuid,
        block_height: i64,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE token_approvals 
            SET status = 'confirmed', confirmation_status = 'Finalized', 
                block_height = $1, confirmed_at = NOW()
            WHERE id = $2
            "#
        )
        .bind(block_height)
        .bind(approval_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark token approval as executed
    pub async fn update_token_approval_executed(
        &self,
        approval_id: &Uuid,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE token_approvals SET status = 'executed', executed_at = NOW() WHERE id = $1"
        )
        .bind(approval_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark token approval as failed
    pub async fn update_token_approval_failed(
        &self,
        approval_id: &Uuid,
        error_message: &str,
        error_code: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE token_approvals 
            SET status = 'failed', error_message = $1, error_code = $2, failed_at = NOW()
            WHERE id = $3
            "#
        )
        .bind(error_message)
        .bind(error_code)
        .bind(approval_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark token approval as expired
    pub async fn mark_token_approvals_expired(&self) -> AppResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE token_approvals 
            SET status = 'expired'
            WHERE status = 'pending' AND expires_at < NOW()
            "#
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get all pending token approvals for a user
    pub async fn get_user_pending_approvals(&self, user_id: &Uuid) -> AppResult<Vec<TokenApproval>> {
        let rows = sqlx::query(
            "SELECT * FROM token_approvals WHERE user_id = $1 AND status IN ('pending', 'signed', 'submitted') ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(TokenApproval::from_row).collect()
    }

    /// Get all token approvals for a quote
    pub async fn get_quote_approvals(&self, quote_id: &Uuid) -> AppResult<Vec<TokenApproval>> {
        let rows = sqlx::query(
            "SELECT * FROM token_approvals WHERE quote_id = $1 ORDER BY created_at DESC"
        )
        .bind(quote_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(TokenApproval::from_row).collect()
    }
}


