# CrossChain Payments Platform - Complete API Flow Guide

This guide walks through the entire user journey from wallet registration to successful cross-chain token swap and settlement.

---

## 📋 Table of Contents

1. [User Onboarding & Wallet Management](#user-onboarding--wallet-management)
2. [Chain Selection & Token Discovery](#chain-selection--token-discovery)
3. [Quote Generation & Trade Setup](#quote-generation--trade-setup)
4. [Payment Processing](#payment-processing)
5. [Cross-Chain Execution](#cross-chain-execution)
6. [Settlement & Treasury Management](#settlement--treasury-management)
7. [Error Handling & Status Checks](#error-handling--status-checks)

---

## User Onboarding & Wallet Management

### Phase 1: User Registration & First Wallet Connection

**Goal**: User creates account and connects their first wallet (funding chain)

#### Step 1: Register Wallet (Funding Chain - Chain A)

```
POST /wallet/register
Content-Type: application/json

{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "chain": "Solana",  // Chain A (user's funding chain)
  "address": "DPfbqv8h9VHhEBFhtZNUdBkYLRfAVsxJ8C1BZg1tcNfp"
}
```

**Response**:
```json
{
  "wallet_id": "660e8400-e29b-41d4-a716-446655440001",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "chain": "Solana",
  "address": "DPfbqv8h9VHhEBFhtZNUdBkYLRfAVsxJ8C1BZg1tcNfp",
  "status": "unverified"
}
```

**Frontend Action**:
- Store `wallet_id` and `user_id` for later use
- Show verification prompt to user

---

#### Step 2: Verify Wallet (Prove Ownership)

User signs a message with their wallet to prove they own it.

```
POST /wallet/verify
Content-Type: application/json

{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "chain": "Solana",
  "address": "DPfbqv8h9VHhEBFhtZNUdBkYLRfAVsxJ8C1BZg1tcNfp",
  "nonce": "generated_random_string_123",
  "signature": "user_signed_message_base58_encoded",
  "signed_message": "Message user actually signed"
}
```

**Response**:
```json
{
  "wallet_id": "660e8400-e29b-41d4-a716-446655440001",
  "chain": "Solana",
  "address": "DPfbqv8h9VHhEBFhtZNUdBkYLRfAVsxJ8C1BZg1tcNfp",
  "verified": true,
  "verified_at": "2026-01-07T10:30:00Z"
}
```

**Frontend Action**:
- Update wallet status to `verified`
- Enable trading features
- Ready for user to connect second wallet

---

#### Step 3: Connect Second Wallet (Execution Chain - Chain B)

User connects wallet on destination chain (e.g., Stellar) where they want to receive tokens.

```
POST /wallet/register
Content-Type: application/json

{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "chain": "Stellar",  // Chain B (execution/destination chain)
  "address": "GBQQ7JJGKYF7ZXLNWCVHTSWVWPKTX4HP5K5AO5C2AQMS2G4BCXJSCBXZ"
}
```

**Response**:
```json
{
  "wallet_id": "660e8400-e29b-41d4-a716-446655440002",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "chain": "Stellar",
  "address": "GBQQ7JJGKYF7ZXLNWCVHTSWVWPKTX4HP5K5AO5C2AQMS2G4BCXJSCBXZ",
  "status": "unverified"
}
```

**Frontend Action**:
- Verify this wallet as well (repeat Step 2)
- Now user has two wallets set up: Solana (funding) and Stellar (execution)
- Can optionally add more wallets for different chains

---

#### Step 4: Get User's Wallets

At any time, retrieve all wallets connected by user:

```
GET /wallet/user/550e8400-e29b-41d4-a716-446655440000
```

**Response**:
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "wallets": [
    {
      "wallet_id": "660e8400-e29b-41d4-a716-446655440001",
      "chain": "Solana",
      "address": "DPfbqv8h9VHhEBFhtZNUdBkYLRfAVsxJ8C1BZg1tcNfp",
      "status": "verified",
      "verified_at": "2026-01-07T10:25:00Z"
    },
    {
      "wallet_id": "660e8400-e29b-41d4-a716-446655440002",
      "chain": "Stellar",
      "address": "GBQQ7JJGKYF7ZXLNWCVHTSWVWPKTX4HP5K5AO5C2AQMS2G4BCXJSCBXZ",
      "status": "verified",
      "verified_at": "2026-01-07T10:28:00Z"
    }
  ]
}
```

---

## Chain Selection & Token Discovery

### Phase 2: User Views Available Tokens and DEXes

**Goal**: Show user what tokens they can trade and which DEXes support them

#### Step 1: Get All Supported Chains

```
GET /discovery/chains
```

**Response**:
```json
[
  "Solana",
  "Stellar",
  "Near"
]
```

---

#### Step 2: Get DEXes Available on Selected Chain

User clicks on Solana → show all DEXes that support it

```
GET /discovery/dexes/Solana
```

**Response**:
```json
[
  {
    "name": "Raydium",
    "chain": "Solana",
    "fee_tier": "0.25%",
    "available": true
  },
  {
    "name": "Marinade",
    "chain": "Solana",
    "fee_tier": "0.25%",
    "available": true
  }
]
```

---

#### Step 3: Get Chain Discovery (Tokens + DEXes in One Call)

More efficient - get all tokens supported on a chain and their available DEXes:

```
GET /discovery/chain/Solana
```

**Response**:
```json
{
  "chain": "Solana",
  "dexes": [
    {
      "name": "Raydium",
      "chain": "Solana",
      "fee_tier": "0.25%",
      "available": true
    },
    {
      "name": "Marinade",
      "chain": "Solana",
      "fee_tier": "0.25%",
      "available": true
    }
  ],
  "supported_tokens": [
    {
      "address": "EPjFWdd5Au...",
      "symbol": "USDC",
      "name": "USD Coin",
      "decimals": 6,
      "logo_uri": "https://..."
    },
    {
      "address": "Es9vMFrzacc...",
      "symbol": "USDT",
      "name": "Tether",
      "decimals": 6,
      "logo_uri": "https://..."
    },
    {
      "address": "EPb9SEg86...",
      "symbol": "SOL",
      "name": "Solana",
      "decimals": 9,
      "logo_uri": "https://..."
    }
  ]
}
```

**Frontend Layout**:
```
┌─────────────────────────────────┐
│ SOLANA Chain Tokens             │
├─────────────────────────────────┤
│ ○ SOL      - Available on: Raydium, Marinade
│ ○ USDC     - Available on: Raydium
│ ○ USDT     - Available on: Raydium, Marinade
│                                 │
│ [Click on token for details]    │
└─────────────────────────────────┘
```

---

#### Step 4: Get Tokens for Specific DEX

Show tokens available on a specific DEX:

```
GET /discovery/dex/Raydium/Solana
```

**Response**:
```json
{
  "chain": "Solana",
  "assets": [
    {
      "address": "EPjFWdd5Au...",
      "symbol": "USDC",
      "name": "USD Coin",
      "decimals": 6,
      "logo_uri": "https://..."
    },
    {
      "address": "Es9vMFrzacc...",
      "symbol": "USDT",
      "name": "Tether",
      "decimals": 6,
      "logo_uri": "https://..."
    }
  ],
  "total_count": 2
}
```

---

#### Step 5: Get Coin Details, Charts & Historical Data

When user clicks a token to see details:

```
GET /quote-engine/ohlc?asset=SOL&chain=Solana&timeframe=1h&limit=24
```

**Response**:
```json
[
  {
    "timestamp": 1672531200,
    "open": 16.50,
    "high": 16.75,
    "low": 16.45,
    "close": 16.60,
    "volume": 125000.00
  },
  {
    "timestamp": 1672534800,
    "open": 16.60,
    "high": 16.85,
    "low": 16.55,
    "close": 16.80,
    "volume": 145000.00
  }
]
```

**Frontend Display**:
```
┌──────────────────────────────────────┐
│ SOL - Solana Native Token            │
├──────────────────────────────────────┤
│ Current Price: $16.80                │
│ 24h High: $16.85  |  24h Low: $16.45 │
│ 24h Volume: 145K  |  Market Cap: XXX │
│                                      │
│ [1H] [4H] [1D] [1W] [1M]             │
│                                      │
│  Chart renders here (candlestick)    │
│  with OHLC data                      │
│                                      │
│ [← Back] [Trade SOL]                 │
└──────────────────────────────────────┘
```

---

## Quote Generation & Trade Setup

### Phase 3: User Initiates Trade

**Goal**: Generate a quote for swapping tokens across chains

#### Step 1: Create Quote (Request Price & Terms)

User wants to: **Send 100 USDC from Solana → Receive equivalent XLM on Stellar**

```
POST /quote
Content-Type: application/json

{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "funding_chain": "Solana",           // Chain A (user pays from)
  "execution_chain": "Stellar",         // Chain B (user receives on)
  "funding_asset": "USDC",              // Asset user sends
  "execution_asset": "XLM",             // Asset user receives
  "execution_instructions_base64": "...",  // Chain-specific swap instructions
  "estimated_compute_units": 200000    // (Optional, for Solana)
}
```

**What happens in backend**:
1. ✅ Validates that user exists
2. ✅ Validates funding_chain and execution_chain are different
3. ✅ Validates both assets are whitelisted
4. ✅ Checks daily spending limits (risk control)
5. ✅ Queries current prices from oracle
6. ✅ Calculates swap route (best DEX path)
7. ✅ Estimates gas/execution costs
8. ✅ Generates unique quote with expiration time

**Response**:
```json
{
  "quote_id": "770e8400-e29b-41d4-a716-446655440003",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "funding_chain": "Solana",
  "execution_chain": "Stellar",
  "funding_asset": "USDC",
  "execution_asset": "XLM",
  "max_funding_amount": "100.00",        // Max user can send
  "execution_cost": "97.50",             // Amount user will receive
  "service_fee": "2.50",                 // Platform fee
  "payment_address": "STELLAR_TREASURY_ADDRESS",  // Where to send payment
  "expires_at": "2026-01-07T10:45:00Z",  // Quote valid for 15 mins
  "nonce": "abc123xyz789"                // Unique identifier for this quote
}
```

**Frontend Action**:
- Display quote to user: "Send 100 USDC → Receive 97.50 XLM"
- Show fee breakdown
- Start countdown timer to expiration
- Show two buttons: [Approve Trade] or [Cancel]

---

#### Step 2: User Approves Trade (Spending Approval)

User clicks "Approve Trade" - this creates a spending approval record:

```
POST /approval/create
Content-Type: application/json

{
  "quote_id": "770e8400-e29b-41d4-a716-446655440003",
  "user_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Response**:
```json
{
  "approval_id": "880e8400-e29b-41d4-a716-446655440004",
  "quote_id": "770e8400-e29b-41d4-a716-446655440003",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "pending_signature",
  "challenge": "xyz123challenge",       // User must sign this
  "signature_required": true,
  "created_at": "2026-01-07T10:31:00Z"
}
```

**Frontend Action**:
- Show wallet signature request to user
- User signs the challenge with their wallet
- Once signed, proceed to payment

---

#### Step 3: User Approves & Executes Payment (Improved Flow)

**NEW APPROACH: Token Approval + Signature Pattern** (Better UX)

Instead of manual transfer, we use a signature-based approval system where:
- User signs an approval message with their wallet
- Backend verifies the signature
- Backend executes the transfer automatically
- No manual copy/paste required

#### Step 3a: Create Approval Request

User clicks "Approve & Pay" button:

```
POST /approval/create
Content-Type: application/json

{
  "quote_id": "770e8400-e29b-41d4-a716-446655440003",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "funding_chain": "Solana",
  "token": "USDC",
  "amount": "100.00",
  "recipient": "SOLAR_TREASURY_ADDRESS"
}
```

**Backend Response**:
```json
{
  "approval_id": "990e8400-e29b-41d4-a716-446655440005",
  "message_to_sign": "APPROVE_USDC_TRANSFER\nAmount: 100.00 USDC\nRecipient: SOLAR_TREASURY_ADDRESS\nQuote ID: 770e8400-e29b-41d4-a716-446655440003\nNonce: xyz123abc789\nExpires: 2026-01-07T10:45:00Z",
  "nonce": "xyz123abc789",
  "expires_at": "2026-01-07T10:45:00Z"
}
```

**Frontend Display**:
```
┌──────────────────────────────────────────┐
│ Approve Token Transfer                   │
├──────────────────────────────────────────┤
│                                          │
│ Platform requests permission to spend:   │
│                                          │
│ Token:     USDC                          │
│ Amount:    100.00                        │
│ Recipient: SOLAR_TREASURY                │
│ Quote ID:  770e8400-e29b-41d4-a716...   │
│                                          │
│ [Approve with Wallet] [Cancel]           │
└──────────────────────────────────────────┘
```

#### Step 3b: User Signs in Wallet

**Frontend Action** (Solana example):
```javascript
// 1. Get message from approval response
const message = new TextEncoder().encode(approval.message_to_sign);

// 2. Prompt user's wallet to sign
const signature = await wallet.signMessage(message);
// ^ Shows wallet UI: "Sign this message?"

// 3. User clicks "Approve" in wallet
// ^ Wallet returns signature
```

**Wallet Signature Prompt**:
```
┌──────────────────────────────────────┐
│ Solana Wallet                        │
├──────────────────────────────────────┤
│ Sign Message?                        │
│                                      │
│ APPROVE_USDC_TRANSFER                │
│ Amount: 100.00 USDC                  │
│ Recipient: SOLAR_TREASURY            │
│ Expires: 2026-01-07T10:45:00Z        │
│                                      │
│ [Approve] [Reject]                   │
└──────────────────────────────────────┘
```

#### Step 3c: Submit Signed Approval + Execute Transfer

Once user signs, frontend submits to backend:

```
POST /approval/submit
Content-Type: application/json

{
  "approval_id": "990e8400-e29b-41d4-a716-446655440005",
  "user_wallet": "DPfbqv8h9VHhEBFhtZNUdBkYLRfAVsxJ8C1BZg1tcNfp",
  "signature": "base64_encoded_signature_from_wallet",
  "message": "APPROVE_USDC_TRANSFER\nAmount: 100.00 USDC\n...",
  "nonce": "xyz123abc789"
}
```

**Backend Processing**:
1. ✅ Verify signature matches user's wallet public key (cryptographically proven)
2. ✅ Verify message hasn't been tampered with
3. ✅ Verify nonce matches (prevent replay attacks)
4. ✅ Verify approval hasn't expired (max 15 min)
5. ✅ **Immediately execute transfer** using treasury wallet
6. ✅ Broadcast transaction to Solana blockchain
7. ✅ Return transaction hash

**Backend Response**:
```json
{
  "approval_id": "990e8400-e29b-41d4-a716-446655440005",
  "status": "executed",
  "transaction_hash": "4vJ9ukSvHwNYUVfABCDEFGHIJKLMNOPQRSTUVWXYZ",
  "amount": "100.00",
  "token": "USDC",
  "from": "DPfbqv8h9VHhEBFhtZNUdBkYLRfAVsxJ8C1BZg1tcNfp",
  "to": "SOLAR_TREASURY_ADDRESS",
  "confirmation_status": "Processed",
  "estimated_confirmation_time": "5-10 seconds"
}
```

**Frontend Display**:
```
┌──────────────────────────────────────────┐
│ ✓ Approval Signed!                       │
├──────────────────────────────────────────┤
│                                          │
│ Status: Transfer in progress             │
│                                          │
│ ⏳ Sending 100 USDC to treasury...       │
│                                          │
│ Transaction: 4vJ9ukSv...                 │
│                                          │
│ Estimated time: 5-10 seconds             │
│ ████████░░░░░░░░░░ 50%                  │
│                                          │
│ DO NOT CLOSE THIS PAGE                   │
└──────────────────────────────────────────┘
```

#### Step 3d: Poll for Confirmation

Frontend continuously polls status (every 2 seconds):

```
GET /approval/status/990e8400-e29b-41d4-a716-446655440005
```

**Response** (updates as blockchain processes):

**Stage 1 - Submitted**:
```json
{
  "approval_id": "990e8400-e29b-41d4-a716-446655440005",
  "status": "submitted",
  "transaction_hash": "4vJ9ukSvHwNYUVfABCDEFGHIJKLMNOPQRSTUVWXYZ",
  "confirmation_status": "Processed"
}
```

**Stage 2 - Confirmed on Blockchain**:
```json
{
  "approval_id": "990e8400-e29b-41d4-a716-446655440005",
  "status": "confirmed",
  "transaction_hash": "4vJ9ukSvHwNYUVfABCDEFGHIJKLMNOPQRSTUVWXYZ",
  "confirmation_status": "Finalized",
  "block_height": 123456789,
  "confirmed_at": "2026-01-07T10:35:00Z"
}
```

**Stage 3 - Execution Triggered**:
```json
{
  "approval_id": "990e8400-e29b-41d4-a716-446655440005",
  "status": "confirmed",
  "transaction_hash": "4vJ9ukSvHwNYUVfABCDEFGHIJKLMNOPQRSTUVWXYZ",
  "confirmation_status": "Finalized",
  "execution_status": "initiated",
  "execution_message": "Transfer confirmed. Initiating cross-chain execution..."
}
```

**Frontend Progress Display**:
```
Transfer Execution Progress:
━━━━━━━━━━━━━━━━━━━━━ 100%

Steps:
✓ User signed approval
✓ Signature verified on backend
✓ Transfer submitted to Solana
✓ Transaction processed
✓ Block finalized (height: 123456789)
✓ 100 USDC received in treasury
✓ Cross-chain execution initiated
```

---

**Why This Approach is Better** (vs Manual Transfer):

| Aspect | Manual Transfer | Approval Flow |
|--------|-----------------|---------------|
| User Steps | 7-10 manual | 2 clicks |
| UX | Leave platform | Stay in platform |
| Error Rate | High (copy/paste) | Low (wallet handles it) |
| Speed | Depends on user | Immediate |
| Retry Logic | Manual | Automatic |
| Security | Medium | High (cryptographic) |
| Time to Execute | 5+ minutes | <1 minute |

**Fallback**: For advanced users, we still support manual transfer as an alternative option.

---

## Payment Processing

### Phase 4: Backend Detects Payment & Commits Quote

**How it works**:
1. User sends 100 USDC to treasury wallet with memo containing quote_id
2. Solana blockchain confirms transaction
3. Webhook listener detects the transaction and calls payment webhook
4. Quote status changes from "Pending" to "Committed"

#### Webhook Listener Flow

The backend continuously monitors blockchain for incoming transactions to treasury wallets.

**When transaction detected on Solana**:
```
Internal webhook processor triggers:

POST /webhook/payment (Internal)
{
  "chain": "Solana",
  "transaction_hash": "abc123def456...",
  "from_address": "DPfbqv8h9VHhEBFhtZNUdBkYLRfAVsxJ8C1BZg1tcNfp",
  "to_address": "SOLAR_TREASURY_ADDRESS",
  "amount": "100.00",
  "asset": "USDC",
  "memo": "770e8400-e29b-41d4-a716-446655440003",  // quote_id
  "timestamp": "2026-01-07T10:35:00Z"
}
```

**Backend Processing**:
1. ✅ Parses memo to find quote_id
2. ✅ Verifies amount matches quote
3. ✅ Locks quote to prevent double-spending
4. ✅ Marks quote as "Committed" (payment received)
5. ✅ **Triggers cross-chain execution immediately**

**Response**:
```json
{
  "accepted": true,
  "quote_id": "770e8400-e29b-41d4-a716-446655440003",
  "funding_chain": "Solana",
  "execution_chain": "Stellar",
  "message": "Payment received and committed. Executing cross-chain swap..."
}
```

**Frontend Action**:
- Show status: "Payment received ✓"
- Show status: "Initiating cross-chain swap..."
- Poll `/status/{quote_id}` endpoint for updates

---

## Cross-Chain Execution

### Phase 5: Backend Executes Trade on Destination Chain

Once payment is received and locked, backend automatically:

1. **Queries DEX for best swap route on execution chain** (Stellar)
2. **Builds transaction** to swap 100 USDC for ~97.50 XLM
3. **Signs transaction** with treasury account
4. **Submits transaction** to execution chain blockchain
5. **Sends tokens to user's destination wallet**

#### The Execution Flow (Internal)

```
Treasury Wallet (Backend Controls)
    ↓
    ├─ [1] Queries Stellar DEX for swap route
    │         "Swap 100 USDC for XLM at rate 0.975"
    │
    ├─ [2] Builds transaction (XDR)
    │         Swap instructions + Send XLM to user
    │
    ├─ [3] Signs with treasury private key
    │         Signature added to transaction
    │
    ├─ [4] Submits to Stellar network
    │         Transaction processing...
    │
    ├─ [5] Confirmed! Hash: xyz123hash...
    │
    └─ [6] Sends 97.50 XLM to user's Stellar wallet
            GBQQ7JJGKYF7ZXLNWCVHTSWVWPKTX4HP5K5AO5C2AQMS2G4BCXJSCBXZ
            
User receives XLM in their wallet ✓
```

#### Checking Execution Status

Frontend continuously checks progress:

```
GET /status/770e8400-e29b-41d4-a716-446655440003
```

**Response** (updates as process progresses):

**Stage 1 - Payment Received**:
```json
{
  "quote_id": "770e8400-e29b-41d4-a716-446655440003",
  "funding_chain": "Solana",
  "execution_chain": "Stellar",
  "status": "committed",
  "transaction_hash": "4vJ9ukSvHwNYUVfABCDE...",
  "executed_at": null,
  "error_message": null
}
```

**Stage 2 - Executing on Destination Chain**:
```json
{
  "quote_id": "770e8400-e29b-41d4-a716-446655440003",
  "funding_chain": "Solana",
  "execution_chain": "Stellar",
  "status": "executing",
  "transaction_hash": "4vJ9ukSvHwNYUVfABCDE...",
  "executed_at": null,
  "error_message": null
}
```

**Stage 3 - Completed Successfully**:
```json
{
  "quote_id": "770e8400-e29b-41d4-a716-446655440003",
  "funding_chain": "Solana",
  "execution_chain": "Stellar",
  "status": "completed",
  "transaction_hash": "4vJ9ukSvHwNYUVfABCDE...",
  "executed_at": "2026-01-07T10:37:00Z",
  "error_message": null
}
```

**Frontend Display** (Real-time progress):
```
Trade Execution Progress:
━━━━━━━━━━━━━━━━━━━━━ 100%

Steps:
✓ Payment received on Solana (100 USDC locked)
✓ Quote committed
✓ DEX route calculated (Marinade route)
✓ Transaction built on Stellar
✓ Signature verified
✓ Submitted to Stellar network
✓ Confirmed on blockchain

Result:
You sent: 100 USDC (from Solana)
You received: 97.50 XLM (on Stellar)
Platform fee: 2.50 USDC
Execution time: 2 minutes 15 seconds

[View on Explorer] [Done]
```

---

## Settlement & Treasury Management

### Phase 6: Settlement & Monitoring

**What happens**:
- Treasury wallet now has 100 USDC from user payment
- Treasury wallet has 97.50 XLM less (sent to user)
- Backend tracks treasury balance for rebalancing

#### Monitor Treasury Balances

Get all treasury balances across chains:

```
GET /admin/treasury
```

**Response**:
```json
[
  {
    "chain": "Solana",
    "asset": "USDC",
    "balance": "5,250.00",      // After receiving user's 100 USDC
    "last_updated": "2026-01-07T10:37:00Z"
  },
  {
    "chain": "Solana",
    "asset": "SOL",
    "balance": "10.50",
    "last_updated": "2026-01-07T10:37:00Z"
  },
  {
    "chain": "Stellar",
    "asset": "XLM",
    "balance": "1,902.50",       // After sending user 97.50 XLM
    "last_updated": "2026-01-07T10:37:00Z"
  },
  {
    "chain": "Near",
    "asset": "NEAR",
    "balance": "500.00",
    "last_updated": "2026-01-07T10:37:00Z"
  }
]
```

#### Get Specific Chain Treasury Balance

```
GET /admin/treasury/Solana
```

**Response**:
```json
{
  "chain": "Solana",
  "asset": "USDC",
  "balance": "5,250.00",
  "last_updated": "2026-01-07T10:37:00Z"
}
```

---

## Error Handling & Status Checks

### Common Error Scenarios

#### Scenario 1: Quote Expires

If user takes >15 minutes to pay:

```
GET /status/770e8400-e29b-41d4-a716-446655440003

Response:
{
  "status": "expired",
  "error_message": "Quote expired at 2026-01-07T10:45:00Z"
}
```

**Frontend Action**: 
- Show "Quote expired" message
- Offer to create new quote

---

#### Scenario 2: Insufficient Balance

If user tries to send less than required:

```
POST /quote
{...}

Response Error:
{
  "error": "RiskControl",
  "message": "Daily spending limit exceeded. 
             Current: $250, Limit: $500, Requested: $300"
}
```

**Frontend Action**:
- Show remaining daily limit
- Suggest smaller amount or wait until next day

---

#### Scenario 3: Execution Failed

If blockchain transaction fails on destination chain:

```
GET /status/770e8400-e29b-41d4-a716-446655440003

Response:
{
  "quote_id": "770e8400-e29b-41d4-a716-446655440003",
  "status": "failed",
  "error_message": "Stellar execution failed: Insufficient trust line.
                    User must add XLM trust line before receiving.",
  "executed_at": null
}
```

**Frontend Action**:
- Show error to user with clear explanation
- Provide recovery steps (e.g., "Add XLM trust line in your wallet")
- Offer retry or refund options

---

#### Scenario 4: Health Check & Circuit Breaker

Monitor system health and chains:

```
GET /health
```

**Response**:
```json
{
  "status": "healthy",
  "timestamp": "2026-01-07T10:40:00Z",
  "circuit_breakers": [
    {
      "chain": "Solana",
      "active": false,
      "reason": null
    },
    {
      "chain": "Stellar",
      "active": false,
      "reason": null
    },
    {
      "chain": "Near",
      "active": false,
      "reason": null
    }
  ]
}
```

If a circuit breaker is active (chain is down):

```json
{
  "status": "degraded",
  "timestamp": "2026-01-07T10:40:00Z",
  "circuit_breakers": [
    {
      "chain": "Solana",
      "active": true,
      "reason": "RPC connection failed. Retrying..."
    },
    {
      "chain": "Stellar",
      "active": false,
      "reason": null
    },
    {
      "chain": "Near",
      "active": false,
      "reason": null
    }
  ]
}
```

**Frontend Action**:
- Show warning banner if any chain is down
- Disable trading for that chain
- Show status updates

---

## Frontend Navigation Structure

Based on the API flow, here's recommended page structure:

```
┌─────────────────────────────────────────────┐
│           CrossChain Payments Platform      │
├─────────────────────────────────────────────┤
│                                             │
│  [Connect Wallets] [Trade] [Portfolio]      │
│                                             │
├─────────────────────────────────────────────┤
│                                             │
│ ┌──────────────────────────────────────┐   │
│ │ 1. ONBOARDING PAGE                   │   │
│ │    ├─ Connect Wallet (Chain A)       │   │
│ │    │  ├─ Address input               │   │
│ │    │  └─ Verify ownership            │   │
│ │    └─ Connect Wallet (Chain B)       │   │
│ │       ├─ Address input               │   │
│ │       └─ Verify ownership            │   │
│ └──────────────────────────────────────┘   │
│                                             │
│ ┌──────────────────────────────────────┐   │
│ │ 2. TOKEN DISCOVERY PAGE              │   │
│ │    ├─ Select Funding Chain (A)       │   │
│ │    │  └─ List all tokens on A        │   │
│ │    ├─ Click token for details        │   │
│ │    │  ├─ Show charts (OHLC)          │   │
│ │    │  ├─ Show DEXes supporting token │   │
│ │    │  └─ Show token stats            │   │
│ │    └─ Show price feeds              │   │
│ └──────────────────────────────────────┘   │
│                                             │
│ ┌──────────────────────────────────────┐   │
│ │ 3. TRADE SETUP PAGE                  │   │
│ │    ├─ Select execution chain (B)     │   │
│ │    ├─ Enter amount to send           │   │
│ │    ├─ Select destination asset       │   │
│ │    ├─ [Get Quote] button             │   │
│ │    └─ Show quote results             │   │
│ │       ├─ Amount to receive           │   │
│ │       ├─ Fee breakdown               │   │
│ │       ├─ Exchange rate               │   │
│ │       ├─ [Approve Trade] button      │   │
│ │       └─ Timer countdown             │   │
│ └──────────────────────────────────────┘   │
│                                             │
│ ┌──────────────────────────────────────┐   │
│ │ 4. PAYMENT PAGE                      │   │
│ │    ├─ Payment instructions           │   │
│ │    ├─ QR code                        │   │
│ │    ├─ Copy address button            │   │
│ │    ├─ Amount to send                 │   │
│ │    └─ Status: "Waiting for payment..." │   │
│ └──────────────────────────────────────┘   │
│                                             │
│ ┌──────────────────────────────────────┐   │
│ │ 5. EXECUTION STATUS PAGE             │   │
│ │    ├─ Real-time progress bar         │   │
│ │    ├─ Step-by-step execution log     │   │
│ │    │  ├─ ✓ Payment received          │   │
│ │    │  ├─ ✓ Quote committed           │   │
│ │    │  ├─ ⏳ DEX route calculated     │   │
│ │    │  ├─ ⏳ Transaction built         │   │
│ │    │  └─ ⏳ Submitting...             │   │
│ │    └─ [View on Explorer]             │   │
│ └──────────────────────────────────────┘   │
│                                             │
│ ┌──────────────────────────────────────┐   │
│ │ 6. COMPLETED PAGE                    │   │
│ │    ├─ Transaction summary            │   │
│ │    ├─ Amount sent / received         │   │
│ │    ├─ Fees paid                      │   │
│ │    ├─ Execution time                 │   │
│ │    ├─ Links to explorers             │   │
│ │    └─ [Done] button                  │   │
│ └──────────────────────────────────────┘   │
│                                             │
│ ┌──────────────────────────────────────┐   │
│ │ 7. PORTFOLIO PAGE                    │   │
│ │    ├─ User's wallets across chains   │   │
│ │    ├─ Balance per chain              │   │
│ │    ├─ Total portfolio value (USD)    │   │
│ │    └─ Historical transactions        │   │
│ └──────────────────────────────────────┘   │
│                                             │
└─────────────────────────────────────────────┘
```

---

## API Endpoint Reference

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/wallet/register` | POST | Register new wallet for user |
| `/wallet/verify` | POST | Verify wallet ownership |
| `/wallet/user/{user_id}` | GET | Get all wallets for user |
| `/discovery/chains` | GET | List all supported chains |
| `/discovery/dexes/{chain}` | GET | List DEXes on a chain |
| `/discovery/chain/{chain}` | GET | Get chain discovery (tokens + DEXes) |
| `/discovery/dex/{dex_name}/{chain}` | GET | Get tokens on specific DEX |
| `/quote-engine/ohlc` | GET | Get OHLC data for charts |
| `/quote` | POST | Create a cross-chain quote |
| **`/approval/create`** | **POST** | **Create spending approval (user signs)** |
| **`/approval/submit`** | **POST** | **Submit signed approval (backend executes)** |
| **`/approval/status/{approval_id}`** | **GET** | **Get approval + transfer status** |
| **`/approval/cancel/{approval_id}`** | **POST** | **Cancel pending approval** |
| `/webhook/payment` | POST | Payment detection webhook (legacy manual flow) |
| `/status/{quote_id}` | GET | Get execution status |
| `/health` | GET | System health & circuit breaker status |
| `/admin/treasury` | GET | Get all treasury balances |
| `/admin/treasury/{chain}` | GET | Get specific chain treasury |

**New Endpoints (Bold)** are part of the improved Approval Flow for better UX and security.

---

## Summary: Complete User Journey

```
User Flow Diagram:
═════════════════════════════════════════════════════════════════

1. ONBOARDING
   └─ Register Wallet A (funding) → Verify
   └─ Register Wallet B (execution) → Verify

2. DISCOVERY
   └─ Browse supported chains
   └─ View tokens on selected chain
   └─ Click token → View charts & details
   └─ See which DEXes support it

3. TRADE SETUP
   └─ Select funding chain & asset
   └─ Select execution chain & asset
   └─ Enter amount
   └─ Get Quote (price, fees, terms)

4. PAYMENT
   └─ Review quote terms
   └─ Approve trade (sign message)
   └─ Send payment to treasury (user's wallet action)
   └─ Frontend polls /status until payment detected

5. EXECUTION (Automatic - Backend)
   └─ Payment confirmed on blockchain
   └─ Quote committed (no double-spend)
   └─ Treasury executes swap on destination chain
   └─ Tokens sent to user's destination wallet
   └─ Frontend shows real-time progress

6. COMPLETION
   └─ User receives tokens on destination chain
   └─ View transaction history
   └─ See final balances across wallets
   └─ Option to make another trade

═════════════════════════════════════════════════════════════════
```

---

## Key Points for Frontend Engineer

1. **Wallet Management**: Always store and reference `wallet_id` for user's wallets

2. **Quote Generation**: Quotes expire (typically 15 mins) - show countdown timer

3. **Payment Responsibility**: User must send exact amount to treasury address with quote_id in memo

4. **Async Execution**: Use polling on `/status/{quote_id}` every 2-5 seconds to update UI

5. **Error Handling**: Always show user-friendly error messages with recovery instructions

6. **Circuit Breakers**: Check `/health` before allowing trades; show warnings if chains are down

7. **Real-time Updates**: Show step-by-step progress (payment received → executing → completed)

8. **Transaction Links**: Provide explorer links for both funding and execution chains

9. **Risk Controls**: Respect daily spending limits returned in API responses

10. **Cache OHLC Data**: Charts data is cached; refresh every 5 minutes for real-time updates

---

This API is production-ready and handles all the complexities of multi-chain swaps transparently!
