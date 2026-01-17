# Crosschain Payments - API Routes Documentation

## Spending Approval Endpoints

### 1. CREATE SPENDING APPROVAL
**Endpoint:** `POST /api/v1/spending-approval/create`

**Purpose:** Create an unsigned spending approval that user must sign on their device.

**Request Body:**
```json
{
  "quote_id": "UUID",
  "approved_amount": "Decimal string (in base units)",
  "wallet_address": "User's wallet address on funding chain"
}
```

**Response (200 OK):**
```json
{
  "id": "UUID - approval ID",
  "user_id": "UUID",
  "funding_chain": "solana|stellar|near",
  "approved_amount": "Decimal string",
  "fee_amount": "Decimal string",
  "gas_amount": "Decimal string",
  "execution_amount": "Decimal string",
  "asset": "Token symbol (SOL/XLM/NEAR)",
  "quote_id": "UUID",
  "wallet_address": "User's wallet",
  "treasury_address": "Where funds go",
  "is_used": false,
  "created_at": "2026-01-06T...",
  "expires_at": "2026-01-06T... (5 min from now)",
  "nonce": "Unique replay protection nonce"
}
```

**Security Checks:**
- Validates quote exists and belongs to user
- Verifies token is on DexWhitelist
- Validates approved amount is reasonable

**Errors:**
- `404 Not Found`: Quote doesn't exist
- `400 Bad Request`: Invalid token or amount

---

### 2. SUBMIT SPENDING APPROVAL
**Endpoint:** `POST /api/v1/spending-approval/:approval_id/submit`

**Purpose:** Submit user's signed approval. This is the critical authorization step.

**Request Body:**
```json
{
  "approval_id": "UUID",
  "signature": "Base58/XDR/Base64 encoded signature from user's wallet"
}
```

**Response (200 OK):**
```json
{
  "approval_id": "UUID",
  "quote_id": "UUID",
  "status": "authorized",
  "message": "Spending approval verified and authorized. Tokens are now accessible for this transaction.",
  "authorized_amount": "Decimal string",
  "authorized_at": "2026-01-06T...",
  "asset": "Token symbol",
  "chain": "solana|stellar|near"
}
```

**Security Checks (7-step verification):**
1. ✅ Approval exists and valid
2. ✅ Not already used (atomic check)
3. ✅ Not expired
4. ✅ User has submitted signature
5. ✅ User has sufficient token balance
6. ✅ Marks as used atomically
7. ✅ Logs audit event

**Errors:**
- `404 Not Found`: Approval doesn't exist
- `400 Bad Request`: Already used or expired
- `400 Invalid Input`: Insufficient token balance

---

### 3. GET SPENDING APPROVAL STATUS
**Endpoint:** `GET /api/v1/spending-approval/:approval_id`

**Purpose:** Query the status of an existing spending approval.

**Request Parameters:**
- `approval_id` (path): UUID of the approval

**Response (200 OK):**
```json
{
  "id": "UUID",
  "user_id": "UUID",
  "funding_chain": "solana|stellar|near",
  "approved_amount": "Decimal string",
  "fee_amount": "Decimal string",
  "gas_amount": "Decimal string",
  "execution_amount": "Decimal string",
  "asset": "Token symbol",
  "quote_id": "UUID",
  "wallet_address": "User's wallet",
  "treasury_address": "Treasury address",
  "is_used": true|false,
  "created_at": "2026-01-06T...",
  "expires_at": "2026-01-06T...",
  "nonce": "Unique nonce"
}
```

**Errors:**
- `404 Not Found`: Approval doesn't exist

---

### 4. LIST USER APPROVALS
**Endpoint:** `GET /api/v1/spending-approval/user/:user_id`

**Purpose:** List all spending approvals for a user (active and inactive).

**Request Parameters:**
- `user_id` (path): UUID of the user

**Response (200 OK):**
```json
{
  "user_id": "UUID",
  "count": 5,
  "approvals": [
    {
      "id": "UUID",
      "funding_chain": "solana",
      "approved_amount": "1000000000",
      "asset": "SOL",
      "quote_id": "UUID",
      "is_used": false,
      "created_at": "2026-01-06T...",
      "expires_at": "2026-01-06T...",
      "nonce": "..."
    }
    // ... more approvals
  ],
  "fetched_at": "2026-01-06T..."
}
```

**Note:** Returns both active (not used, not expired) and inactive approvals.

---

## Settlement Endpoints

### 5. GET SETTLEMENT STATUS
**Endpoint:** `GET /api/v1/settlement/:quote_id`

**Purpose:** Get complete settlement information for a quote, including execution status and all recorded settlements.

**Request Parameters:**
- `quote_id` (path): UUID of the quote

**Response (200 OK):**
```json
{
  "quote_id": "UUID",
  "status": "pending|committed|executed|failed|expired|settled",
  "execution_chain": "solana|stellar|near",
  "funding_chain": "solana|stellar|near",
  "execution_cost": "Decimal string",
  "max_funding_amount": "Decimal string",
  "service_fee": "Decimal string",
  "settlement_records": [
    {
      "settlement_id": "UUID",
      "chain": "solana",
      "transaction_hash": "Hash of settlement transaction",
      "amount": "Decimal string",
      "settled_at": "2026-01-06T...",
      "verified_at": "2026-01-06T... (null if not yet verified)"
    }
    // ... more settlement records
  ],
  "created_at": "2026-01-06T...",
  "expires_at": "2026-01-06T..."
}
```

**Notes:**
- `settlement_records` array is empty if no settlements exist yet
- `verified_at` is null until settlement is verified on-chain
- Status transitions follow strict state machine rules

**Errors:**
- `404 Not Found`: Quote doesn't exist

---

## Treasury Management Endpoints

### 6. GET ALL TREASURY BALANCES
**Endpoint:** `GET /api/v1/admin/treasury`

**Purpose:** Get treasury balances across all chains with circuit breaker status.

**Request Parameters:** None

**Response (200 OK):**
```json
{
  "treasuries": [
    {
      "chain": "solana",
      "asset": "SOL",
      "balance": "50.5",
      "circuit_breaker_active": false,
      "last_updated": "2026-01-06T..."
    },
    {
      "chain": "stellar",
      "asset": "XLM",
      "balance": "1000.25",
      "circuit_breaker_active": false,
      "last_updated": "2026-01-06T..."
    },
    {
      "chain": "near",
      "asset": "NEAR",
      "balance": "100.75",
      "circuit_breaker_active": true,
      "last_updated": "2026-01-06T..."
    }
  ],
  "total_chains": 3,
  "timestamp": "2026-01-06T..."
}
```

**Use Case:** Admin dashboard showing overall treasury health.

---

### 7. GET CHAIN TREASURY BALANCE
**Endpoint:** `GET /api/v1/admin/treasury/:chain`

**Purpose:** Get detailed treasury information for a specific chain, including daily limits and circuit breaker status.

**Request Parameters:**
- `chain` (path): `solana`, `stellar`, or `near`

**Response (200 OK):**
```json
{
  "chain": "solana",
  "asset": "SOL",
  "balance": "50.5",
  "daily_limit": "100.0",
  "daily_spending": "25.5",
  "daily_remaining": "74.5",
  "daily_transaction_count": 12,
  "circuit_breaker": {
    "active": false,
    "reason": null,
    "triggered_at": null
  },
  "last_updated": "2026-01-06T..."
}
```

**Detailed Response Fields:**
- `daily_limit`: Maximum amount that can be spent per day
- `daily_spending`: Amount already spent today
- `daily_remaining`: Limit minus spending
- `daily_transaction_count`: Number of transactions executed today
- `circuit_breaker.active`: Whether circuit breaker is triggered
- `circuit_breaker.reason`: Why it was triggered (e.g., "Execution failure cascade: 5 consecutive failures")
- `circuit_breaker.triggered_at`: When it was activated

**Use Case:** Ops monitoring - track treasury health, spending limits, and system status.

**Errors:**
- `400 Bad Request`: Invalid chain name (not solana|stellar|near)

---

## Authentication & Authorization

All endpoints require:
- Valid user context (via session/JWT)
- User ID must match approval/spending records
- Admin endpoints require admin role

## Error Responses

Standard error format (applicable to all endpoints):
```json
{
  "error": "Error message",
  "code": "ERROR_CODE",
  "timestamp": "2026-01-06T..."
}
```

Common HTTP Status Codes:
- `200 OK`: Success
- `400 Bad Request`: Invalid input, validation failed
- `401 Unauthorized`: Missing/invalid authentication
- `403 Forbidden`: User lacks permissions
- `404 Not Found`: Resource not found
- `409 Conflict`: State conflict (e.g., already used)
- `500 Internal Server Error`: Server error

---

## Request/Response Content Type

All endpoints use:
- **Content-Type:** `application/json`
- **Accept:** `application/json`

---

## Rate Limiting

Subject to global rate limits:
- User endpoints: 100 requests/minute per user
- Admin endpoints: 50 requests/minute per admin
- Webhook endpoints: No rate limit (authenticated via HMAC)

---

## Idempotency

For state-changing operations (POST):
- Use `nonce` field for replay protection
- Submit twice with same nonce → second request is rejected
- Safe for network retry scenarios

---

## Example Workflow: Complete Spending Approval Flow

```
1. User initiates transaction
   POST /api/v1/spending-approval/create
   ← Returns unsigned approval

2. User signs approval on their device
   (Uses wallet-specific signing logic)

3. Client submits signed approval
   POST /api/v1/spending-approval/{id}/submit
   ← Returns authorized status

4. System begins execution
   (Funds are now locked and accessible)

5. Query settlement status
   GET /api/v1/settlement/{quote_id}
   ← Returns execution chain, status, and settlement records

6. Monitor treasury
   GET /api/v1/admin/treasury/{chain}
   ← Track daily spending, limits, circuit breaker
```

---

## Notes for API Consumers

1. **Approval Expiration:** Approvals expire 5 minutes after creation. Submit within this window.
2. **Settlement Delays:** On-chain confirmations may take 10-60 seconds depending on chain.
3. **Circuit Breaker:** When active, execution is halted. Check status before retrying.
4. **Idempotency:** All spending approval operations are idempotent via nonce.
5. **Audit Trail:** All operations logged. Use quote_id to trace full transaction lifecycle.
