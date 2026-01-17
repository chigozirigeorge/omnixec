# API Usage Analysis & Gap Report

## Current Implementation Status

### ✅ IMPLEMENTED ENDPOINTS

#### Quote Management (3/3 endpoints implemented)
1. **POST /quote** ✅
   - Used in: `Trade.tsx` → `handleGetQuote()`
   - Data flow: User selects chains/assets/amount → API creates quote → stores in Zustand
   - Status: Fully implemented

2. **POST /commit** ✅
   - Used in: `QuoteReview.tsx` → `handleCommit()`
   - Data flow: User reviews quote → submits commitment → navigates to execution
   - Status: Fully implemented

3. **GET /status/:quote_id** ✅
   - Used in: `Execution.tsx` → polls every 3 seconds
   - Data flow: Continuous polling for transaction status
   - Status: Fully implemented

#### Chart API (2/2 endpoints implemented)
1. **GET /chart/:asset/:chain/:timeframe** ✅
   - Used in: `Charts.tsx` → `chartApi.getOHLC()`
   - Status: Integrated with lightweight-charts library
   - Note: Currently using mock data generation

2. **GET /chart/:asset/:chain/:timeframe/latest** ✅
   - Used in: `Charts.tsx` (available but not actively used)
   - Status: Endpoint defined but not utilized

#### Admin API (1/2 endpoints implemented)
1. **GET /health** ✅
   - Status: Defined in API client but never called
   - Use case: Could be used for system status checks

2. **GET /admin/treasury** ✅
   - Status: Defined in API client but never called
   - Use case: Could be used for admin dashboard

---

## ❌ UNUSED/NOT IMPLEMENTED ENDPOINTS

### Spending Approval Endpoints (0/4 implemented)
These are **CRITICAL** missing endpoints for the spending approval flow:

1. **POST /api/v1/spending-approval/create**
   - Purpose: Create unsigned spending approval for user to sign
   - Missing integration: No API call in codebase
   - Required data: quote_id, approved_amount, wallet_address
   - Impact: Cannot implement secure spending authorization flow

2. **POST /api/v1/spending-approval/:approval_id/submit**
   - Purpose: Submit user's signed approval (7-step verification)
   - Missing integration: No API call in codebase
   - Required data: approval_id, signature (Base58/XDR/Base64)
   - Impact: Cannot execute secure token authorization

3. **GET /api/v1/spending-approval/:approval_id**
   - Purpose: Query spending approval status
   - Missing integration: No API call in codebase
   - Use case: Verify approval before proceeding

4. **GET /api/v1/spending-approval/user/:user_id**
   - Purpose: List all user's approvals (active and inactive)
   - Missing integration: No API call in codebase
   - Use case: User history/audit trail

### Settlement Endpoints (0/1 implemented)
1. **GET /api/v1/settlement/:quote_id**
   - Purpose: Get complete settlement info with execution status and settlement records
   - Missing integration: No API call in codebase
   - Currently using: Basic `GET /status/:quote_id` instead
   - Gap: Not capturing full settlement records array or on-chain verification status

### Treasury Management Endpoints (0/2 implemented)
1. **GET /api/v1/admin/treasury** (different from old endpoint)
   - Purpose: Get treasury balances with circuit breaker status
   - Missing integration: Never called
   - Data includes: chain, asset, balance, circuit_breaker_active

2. **GET /api/v1/admin/treasury/:chain**
   - Purpose: Detailed chain treasury info with daily limits
   - Missing integration: Never called
   - Critical data missing: daily_limit, daily_spending, daily_remaining, circuit_breaker details

---

## 🔴 CRITICAL GAPS

### 1. **Spending Approval Flow Not Implemented**
**Current Flow:**
```
Trade → Create Quote → Commit Quote → Execution Status
```

**Should Be (per API docs):**
```
Trade → Create Quote → Create Spending Approval → User Signs → Submit Approval → Commit Quote → Execution Status
```

**Missing Components:**
- No spending approval creation in Trade/QuoteReview
- No wallet signing integration for approvals
- No approval submission flow
- No atomic verification of approvals before commit

**Implementation Required:**
- Add spending approval step before commit
- Integrate wallet signing (use wallet adapters for Solana/Stellar/NEAR)
- Submit signed approval to `/api/v1/spending-approval/{id}/submit`
- Verify approval before proceeding to execution

### 2. **User ID Management**
**Current Issue:**
```tsx
// Trade.tsx, line 60
user_id: crypto.randomUUID()  // ❌ Generates random UUID every time!
```

**Problems:**
- New random UUID generated for each quote creation
- No persistent user identification
- Cannot track user history via `/spending-approval/user/:user_id`
- Backend cannot correlate quotes/approvals to same user

**Solution Required:**
- Implement persistent user identification (Auth/Session)
- Store user_id in Zustand store
- Pass same user_id throughout transaction lifecycle

### 3. **Settlement Status vs Quote Status**
**Current Implementation:**
- Uses basic `GET /status/:quote_id` endpoint
- Missing settlement-specific data:
  - Settlement records array
  - Per-chain settlement transactions
  - On-chain verification status
  - Settlement timestamps

**Better Approach:**
- Switch to `GET /api/v1/settlement/:quote_id`
- Display settlement records in UI
- Show on-chain verification progress
- Handle settlement-specific error states

### 4. **Circuit Breaker Not Monitored**
**Missing Feature:**
- No monitoring of circuit breaker status
- Cannot handle execution halts gracefully
- No UI feedback when circuit breaker is active

**Solution Required:**
- Poll `/api/v1/admin/treasury/:chain` before commit
- Check `circuit_breaker.active` status
- Block execution if circuit breaker triggered
- Display reason to user

### 5. **No Daily Spending Limits Display**
**Missing Information:**
- User not shown daily spending limits
- No warning when approaching limits
- No visibility into daily transaction count

**Solution Required:**
- Display treasury limits in UI
- Show daily remaining amount
- Warn if quote would exceed daily limit

---

## 📋 IMPLEMENTATION ROADMAP

### Phase 1: User Identification & Setup
**Priority: CRITICAL**
1. Remove random UUID generation in Trade.tsx
2. Add user authentication/session management to Zustand
3. Pass persistent user_id to all API calls

**Files to modify:**
- `src/stores/useStore.ts` - Add user management
- `src/pages/Trade.tsx` - Use store user_id instead of random UUID

### Phase 2: Spending Approval Flow
**Priority: CRITICAL**
1. Create new approval service in `lib/api.ts`:
   ```typescript
   export const approvalApi = {
     create: (data: CreateSpendingApprovalRequest) =>
       api.post<SpendingApprovalResponse>('/api/v1/spending-approval/create', data),
     
     submit: (approvalId: string, signature: string) =>
       api.post(`/api/v1/spending-approval/${approvalId}/submit`, { signature }),
     
     getStatus: (approvalId: string) =>
       api.get(`/api/v1/spending-approval/${approvalId}`),
     
     listUserApprovals: (userId: string) =>
       api.get(`/api/v1/spending-approval/user/${userId}`),
   };
   ```

2. Add approval state to Zustand
3. Create approval creation flow in Trade.tsx
4. Integrate wallet signing (use @solana/web3.js, stellar-sdk, etc.)
5. Add approval submission in QuoteReview.tsx before commit

**Files to create:**
- `src/types/approval.ts` - Type definitions
- `src/hooks/useWalletSign.ts` - Wallet signing hook

**Files to modify:**
- `src/lib/api.ts` - Add approvalApi
- `src/stores/useStore.ts` - Add approval state
- `src/pages/Trade.tsx` - Create approval before quote
- `src/pages/QuoteReview.tsx` - Submit approval before commit

### Phase 3: Settlement & Execution Improvements
**Priority: HIGH**
1. Update `Execution.tsx` to use settlement endpoint:
   ```tsx
   const response = await settlementApi.getStatus(quote.quote_id);
   // Display settlement records
   // Show on-chain verification progress
   ```

2. Add settlement record display in UI
3. Handle settlement-specific states

**Files to create:**
- `src/types/settlement.ts`

**Files to modify:**
- `src/lib/api.ts` - Add settlementApi
- `src/pages/Execution.tsx` - Use settlement endpoint

### Phase 4: Treasury Monitoring & Circuit Breaker
**Priority: MEDIUM**
1. Add treasury monitoring before quote commit:
   ```tsx
   const treasuryStatus = await treasuryApi.getChainStatus(funding_chain);
   if (treasuryStatus.circuit_breaker.active) {
     // Show error and reason to user
   }
   ```

2. Add daily limit warnings
3. Display circuit breaker status in UI

**Files to modify:**
- `src/lib/api.ts` - Add proper treasuryApi methods
- `src/pages/Trade.tsx` - Check treasury before quote
- `src/pages/QuoteReview.tsx` - Check circuit breaker before commit

### Phase 5: Admin Dashboard Features
**Priority: LOW**
1. Create admin dashboard for treasury monitoring
2. Display treasury balances across chains
3. Show circuit breaker status
4. Display daily spending metrics

**Files to create:**
- `src/pages/Admin.tsx`
- `src/components/TreasuryDashboard.tsx`

---

## 📊 API Integration Summary Table

| Endpoint | Status | Used In | Data Gap |
|----------|--------|---------|----------|
| POST /quote | ✅ | Trade.tsx | None |
| POST /commit | ✅ | QuoteReview.tsx | Missing approval check |
| GET /status/:quote_id | ✅ | Execution.tsx | Not using settlement records |
| GET /chart/:asset/:chain/:timeframe | ✅ | Charts.tsx | Using mock data |
| GET /chart/latest | ✅ | Charts.tsx | Not used |
| POST /spending-approval/create | ❌ | NONE | CRITICAL |
| POST /spending-approval/:id/submit | ❌ | NONE | CRITICAL |
| GET /spending-approval/:id | ❌ | NONE | CRITICAL |
| GET /spending-approval/user/:id | ❌ | NONE | Audit trail lost |
| GET /settlement/:quote_id | ❌ | NONE | Full settlement data lost |
| GET /admin/treasury | ❌ | NONE | Dashboard needed |
| GET /admin/treasury/:chain | ❌ | NONE | Circuit breaker ignored |

---

## 🚨 Data Flow Issues

### Issue 1: User ID Not Persistent
```
Current: crypto.randomUUID() → new UUID per quote
Better: Persist in store → use same UUID for all operations
```

### Issue 2: Approval Bypass
```
Current: Quote → Commit (no verification)
Should: Quote → CreateApproval → UserSign → SubmitApproval → Commit
```

### Issue 3: Settlement Data Not Captured
```
Current: Only basic quote status
Should: Full settlement records with tx hashes and verification status
```

### Issue 4: No Circuit Breaker Checks
```
Current: Blindly commit quotes
Should: Check treasury circuit breaker before commit
```

---

## Implementation Examples

### Example: Spending Approval Flow
```typescript
// Step 1: After user reviews quote and clicks "Approve"
const approval = await approvalApi.create({
  quote_id: quote.quote_id,
  approved_amount: quote.max_funding_amount,
  wallet_address: wallet.address,
});

// Step 2: User signs approval on their device
const signature = await wallet.signMessage(approval.nonce);

// Step 3: Submit signed approval
await approvalApi.submit(approval.id, signature);

// Step 4: Now safe to commit
await quoteApi.commit(quote.quote_id);
```

### Example: Settlement Status
```typescript
// Better status polling
const settlement = await settlementApi.getStatus(quote.quote_id);

// Display settlement records
settlement.settlement_records.forEach(record => {
  console.log(`${record.chain}: ${record.transaction_hash}`);
  console.log(`Verified: ${record.verified_at}`);
});

// Check circuit breaker
const treasury = await treasuryApi.getChainStatus(quote.execution_chain);
if (treasury.circuit_breaker.active) {
  throw new Error(`Circuit breaker: ${treasury.circuit_breaker.reason}`);
}
```

---

## Summary

**Current State:** Frontend implements basic quote → commit → status flow but missing security-critical spending approval layer and settlement tracking.

**Key Gaps:**
1. ❌ Spending approval flow (CRITICAL)
2. ❌ User identification persistence (CRITICAL)
3. ❌ Settlement record tracking (HIGH)
4. ❌ Circuit breaker monitoring (HIGH)
5. ❌ Daily spending limits (MEDIUM)

**Effort Estimate:**
- Phase 1 (User ID): 2-3 hours
- Phase 2 (Spending Approval): 8-12 hours (includes wallet integration)
- Phase 3 (Settlement): 4-6 hours
- Phase 4 (Treasury): 4-6 hours
- Phase 5 (Admin): 6-8 hours

**Total: 24-35 hours** to fully implement all endpoints

**Must-Do First:** Phase 1 & 2 (User ID + Spending Approval) - These are blocking requirements for production-ready security.
