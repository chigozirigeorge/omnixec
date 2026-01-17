# Quote Status Lifecycle & Transition Flow

## Current Status Enum

```rust
pub enum QuoteStatus {
    Pending,      // Initial state when quote is created
    Committed,    // User has approved spending, execution is being prepared
    Executed,     // Execution succeeded on destination chain
    Expired,      // Quote TTL exceeded
    Failed,       // Execution failed
}
```

---

## Complete Quote Lifecycle

### 1. CREATION → PENDING
**Location:** `src/quote_engine/engine.rs::generate_quote()`

```
User calls: POST /quote
  ↓
Quote created with status = PENDING
  ↓
Payment address generated for funding chain
  ↓
Quote expires_at = now + TTL_SECONDS
```

**State Machine Check:** ✅ Present in `create_quote()`
- Creates quote with default status `PENDING`
- No explicit validation of initial state (not needed)

---

### 2. PENDING → COMMITTED
**Location:** `src/quote_engine/engine.rs::validate_for_execution()`

**Triggered When:**
- User approves spending on funding chain
- Webhook confirms user's transaction (user sent funds)

**Validation Checks:**
```rust
quote.can_execute()                    // Status is Committed/Pending?
quote.has_valid_chain_pair()          // Chain pair still supported?
!quote.is_expired()                   // TTL not exceeded?
user_balance.available() >= amount    // User has funds?
```

**State Transition:**
```rust
UPDATE quotes
SET status = COMMITTED
WHERE id = quote_id AND status = PENDING
```

**Problem Found:** ⚠️ Missing expiration check!
- No method to mark quote as `EXPIRED` when TTL exceeded
- System should reject old quotes with warning

---

### 3. COMMITTED → EXECUTED
**Locations:**
- `src/quote_engine/engine.rs::mark_executed()`
- `src/execution/router.rs` (called by chain executors)

**Triggered When:**
- Chain executor successfully broadcasts transaction
- Transaction confirmed on destination chain

**Success Flow:**
```
Solana Executor broadcast successful
  ↓
Stellar confirms transaction mined
  ↓
mark_executed() called
  ↓
UPDATE quotes SET status = EXECUTED WHERE status = COMMITTED
  ↓
Settlement recorded
  ↓
AuditLog: QuoteExecuted
```

**Example Code Paths:**
```
src/execution/solana.rs:348
src/execution/stellar.rs:566
src/execution/near.rs:421
```

---

### 4. COMMITTED → FAILED
**Locations:**
- `src/quote_engine/engine.rs::mark_failed()`
- `src/execution/router.rs` (error handler)

**Triggered When:**
- Chain broadcast fails
- RPC error/timeout
- Insufficient gas on execution chain
- Transaction reverted on destination chain

**Failure Flow:**
```
Solana executor RPC error
  ↓
Retry logic exhausted
  ↓
mark_failed() called
  ↓
UPDATE quotes SET status = FAILED WHERE status = COMMITTED
  ↓
User can retry with new quote
  ↓
AuditLog: QuoteExecutionFailed
```

---

### 5. PENDING/COMMITTED → EXPIRED
**Status:** ⚠️ **NOT IMPLEMENTED**

**Should Happen:**
```
Scheduled Job (hourly/cron)
  ↓
SELECT quotes WHERE status IN (PENDING, COMMITTED) AND expires_at < NOW()
  ↓
UPDATE quotes SET status = EXPIRED WHERE status = PENDING/COMMITTED AND expires_at < NOW()
  ↓
Log expiration event
```

**Impact:**
- Users can't execute stale quotes
- Prevents confusion with old quotes
- Clears pending state after TTL

---

## Status Transition Diagram

```
                    ┌─────────────────────────────────────────┐
                    │                                         │
                    v                                         │
            ┌─────────────┐                                   │
            │  PENDING    │                                   │
            └─────────────┘                                   │
                    │                                         │
         (user approves spending)                            │
                    │                                         │
                    v                                         │
            ┌─────────────┐                                   │
            │ COMMITTED   │ ◄──── (Hourly TTL check)         │
            └─────────────┘                                   │
                 │     │                                      │
                 │     └──────────────► FAILED ◄─────────────┘
                 │                       (exec error)
                 │
                 └──────────────────► EXECUTED
                                       (success)
```

---

## Current Implementation Status

### ✅ IMPLEMENTED
| Transition | Method | File | Status |
|-----------|--------|------|--------|
| Create → PENDING | `create_quote()` | repository.rs:130 | ✅ |
| PENDING → COMMITTED | `validate_for_execution()` | engine.rs:322 | ✅ |
| COMMITTED → EXECUTED | `mark_executed()` | engine.rs:413 | ✅ |
| COMMITTED → FAILED | `mark_failed()` | engine.rs:425 | ✅ |
| State validation | `update_quote_status()` | repository.rs:249 | ✅ (atomic) |

### ⚠️ NOT IMPLEMENTED
| Transition | Method | File | Status |
|-----------|--------|------|--------|
| ANY → EXPIRED | (missing) | repository.rs | ❌ MISSING |
| Expiration check | (missing) | engine.rs | ❌ MISSING |
| Scheduled expiration job | (missing) | main.rs | ❌ MISSING |

---

## Issues & Gaps

### Issue 1: No Expiration Cleanup ❌
**Problem:** Quotes never automatically expire
**Impact:** 
- Database bloats with old quotes
- Users confused if they retry old quote
- No audit trail of expired quotes

**Solution:**
```rust
// Add to repository.rs
pub async fn expire_old_quotes(&self) -> AppResult<u64> {
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

// Also expire COMMITTED quotes
pub async fn expire_committed_quotes(&self) -> AppResult<u64> {
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

// Run in background task
pub async fn cleanup_expired_quotes(ledger: Arc<LedgerRepository>) {
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await; // Every hour
        match ledger.expire_old_quotes().await {
            Ok(count) => info!("Expired {} pending quotes", count),
            Err(e) => error!("Failed to expire quotes: {:?}", e),
        }
        match ledger.expire_committed_quotes().await {
            Ok(count) => info!("Expired {} committed quotes", count),
            Err(e) => error!("Failed to expire committed quotes: {:?}", e),
        }
    }
}
```

---

### Issue 2: No Check Before Execution ❌
**Problem:** `can_execute()` method exists but not used everywhere

**Current Usage:**
```rust
// Used in:
src/quote_engine/engine.rs:332    // validate_for_execution() ✅
```

**Should Be Used:**
```rust
// Missing from:
src/api/handler.rs                 // Before webhook settlement
src/execution/router.rs            // Before executing
```

---

### Issue 3: Race Condition in Status Transitions ⚠️
**Current Protection:** ✅ Good!

```rust
pub async fn update_quote_status(
    &self,
    tx: &mut Transaction<'_, Postgres>,
    quote_id: Uuid,
    from_status: QuoteStatus,
    to_status: QuoteStatus,
) -> AppResult<()> {
    let result = sqlx::query!(
        r#"
        UPDATE quotes
        SET status = $3
        WHERE id = $1 AND status = $2    // ← Checks current status!
        "#,
        // ...
    )
    .execute(&mut **tx)
    .await?;
    
    if result.rows_affected() == 0 {
        return Err(QuoteError::InvalidState {...})?;
    }
    Ok(())
}
```

**Why This Is Good:**
- Prevents double-execution (if Committed → Executed runs twice)
- Ensures single source of truth
- Atomic at DB level
- **No redundant state changes possible**

---

## Recommended State Lifecycle Additions

### 1. Add Expiration to Create Quote
```rust
pub async fn create_quote(...) -> AppResult<Quote> {
    // ... existing code ...
    
    // Insert should default to:
    // status = PENDING
    // created_at = NOW()
    // updated_at = NOW()
    // expires_at = NOW() + TTL
    
    // Then start background task to clean expired
}
```

### 2. Add Expiration Validator
```rust
impl Quote {
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.expires_at
    }
    
    pub fn can_execute(&self) -> bool {
        self.status == QuoteStatus::Committed && !self.is_expired()
    }
}
```

### 3. Start Background Cleanup Task in main.rs
```rust
// In src/main.rs after initializing LedgerRepository
tokio::spawn(cleanup_expired_quotes(ledger.clone()));
```

### 4. Add Transitions Diagram to Database
```sql
-- Document as comment in migrations/

-- Quote Status Transitions:
-- PENDING      → COMMITTED (when user approves spending)
-- COMMITTED    → EXECUTED  (when execution succeeds)
-- COMMITTED    → FAILED    (when execution fails)
-- PENDING      → EXPIRED   (when TTL exceeded)
-- COMMITTED    → EXPIRED   (when TTL exceeded while waiting)
-- EXPIRED      → (final, no further transitions)
-- FAILED       → (final, user must create new quote)
-- EXECUTED     → (final, completed successfully)
```

---

## Production Ready Checklist

- [x] Atomic status transitions (using WHERE status = X)
- [x] Validation before execution
- [x] Success path (PENDING → COMMITTED → EXECUTED)
- [x] Failure path (PENDING/COMMITTED → FAILED)
- [ ] Expiration cleanup job (⚠️ MISSING)
- [ ] Expiration transition (⚠️ MISSING)
- [ ] Audit logging for all transitions (⚠️ PARTIAL)
- [ ] Monitoring/alerts for stuck quotes (⚠️ MISSING)

---

## Summary

**Your status lifecycle is 80% complete:**
- ✅ Quote created as PENDING
- ✅ User approval moves to COMMITTED
- ✅ Execution success/failure handled
- ✅ Atomic transitions prevent redundancy
- ❌ Missing: Automatic expiration cleanup

**The system DOES prevent redundancy because:**
1. Each status transition checks current status with `WHERE status = X`
2. If status doesn't match, transition fails with InvalidState error
3. No double-execution possible due to database constraint
4. Transaction wrapping ensures atomicity

**Add these 2 methods to complete the lifecycle:**
1. `expire_old_quotes()` - Mark old PENDING/COMMITTED as EXPIRED
2. Background task that runs hourly to clean expired quotes
