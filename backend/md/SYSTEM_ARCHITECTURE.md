# CrossChain Payments - System Architecture & Data Flow Diagrams

## 1. High-Level System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     FRONTEND APPLICATION                       │
│  (React/Vue - Pages for onboarding, discovery, trade, status)  │
└────────────────────────────┬────────────────────────────────────┘
                             │ HTTP REST APIs
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│                    BACKEND API SERVER (Rust/Axum)              │
│ ┌──────────────────────────────────────────────────────────┐   │
│ │ CORE MODULES                                             │   │
│ │ ├─ Wallet Management (register, verify, list wallets)   │   │
│ │ ├─ Quote Engine (calculate prices, routes)              │   │
│ │ ├─ Execution Router (route trades to chains)            │   │
│ │ ├─ Risk Controller (daily limits, circuit breakers)     │   │
│ │ ├─ Ledger Repository (track quotes, executions)         │   │
│ │ └─ Trading Repository (track user trades)               │   │
│ └──────────────────────────────────────────────────────────┘   │
│                             │                                    │
│  ┌──────────────────────────┼──────────────────────────────┐   │
│  ↓                          ↓                              ↓   │
│ ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ │
│ │ PostgreSQL       │ │ Execution Layer  │ │ Blockchain       │ │
│ │ Database         │ │                  │ │ Monitoring       │ │
│ │                  │ │ ├─ Solana        │ │                  │ │
│ │ ├─ Users         │ │ ├─ Stellar       │ │ ├─ Tx Listeners  │ │
│ │ ├─ Wallets       │ │ └─ NEAR          │ │ └─ Webhooks      │ │
│ │ ├─ Quotes        │ │                  │ │                  │ │
│ │ ├─ Executions    │ │ (Signs & submits │ │ (Detects        │ │
│ │ └─ Trades        │ │  treasury txs)   │ │  payments)       │ │
│ └──────────────────┘ └──────────────────┘ └──────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                             │ Blockchain Connections
        ┌────────────────────┼────────────────────┐
        ↓                    ↓                    ↓
   ┌─────────┐         ┌──────────┐         ┌──────────┐
   │ Solana  │         │ Stellar  │         │ NEAR     │
   │ Network │         │ Network  │         │ Network  │
   └─────────┘         └──────────┘         └──────────┘
   (RPC, SPL)      (Horizon, XDR)      (JsonRpc, WASM)
```

---

## 2. User Wallet Registration Flow

```
FRONTEND                      BACKEND                      DATABASE
  │                             │                             │
  │ 1. User enters wallet addr  │                             │
  │────────────────────────────→│                             │
  │ POST /wallet/register       │                             │
  │ {user_id, chain, address}   │                             │
  │                             │ 2. Validate format          │
  │                             │    Check if exists          │
  │                             │────────────────────────────→│
  │                             │                   Check DB  │
  │                             │←────────────────────────────│
  │                             │                  Not exists  │
  │                             │ 3. Create wallet record     │
  │                             │────────────────────────────→│
  │                             │                  Store in DB │
  │                             │←────────────────────────────│
  │ 4. Response with wallet_id  │                             │
  │←────────────────────────────│                             │
  │ {wallet_id, status:         │                             │
  │  "unverified"}              │                             │
  │                             │                             │
  │ 5. User signs message       │                             │
  │────────────────────────────→│                             │
  │ POST /wallet/verify         │                             │
  │ {signature, signed_msg}     │                             │
  │                             │ 6. Verify signature         │
  │                             │    (Crypto validation)      │
  │                             │                             │
  │                             │ 7. Update wallet status     │
  │                             │────────────────────────────→│
  │                             │           Mark verified     │
  │                             │←────────────────────────────│
  │ 8. Response: verified=true  │                             │
  │←────────────────────────────│                             │
  │ Wallet is now active!       │                             │
  │                             │                             │
```

---

## 3. Quote Generation & Price Calculation

```
FRONTEND                BACKEND                  ORACLES/DEX
  │                       │                         │
  │ 1. User clicks Trade  │                         │
  │────────────────────→  │                         │
  │ POST /quote           │                         │
  │ {user_id,             │                         │
  │  funding_chain: SOL,  │                         │
  │  execution_chain: STL,│                         │
  │  funding_asset: USDC, │                         │
  │  execution_asset: XLM}│                         │
  │                       │                         │
  │                       │ 2. Validate chains     │
  │                       │ Validate tokens        │
  │                       │ Check daily limit      │
  │                       │ (Risk checks)          │
  │                       │                         │
  │                       │ 3. Query prices        │
  │                       │────────────────────────→│
  │                       │ "Get current price of: │
  │                       │  USDC (SOL chain) and  │
  │                       │  XLM (STL chain)"      │
  │                       │                         │
  │                       │←────────────────────────│
  │                       │ USDC: $1.00             │
  │                       │ XLM: $0.10              │
  │                       │                         │
  │                       │ 4. Calculate route     │
  │                       │────────────────────────→│
  │                       │ "Best path for         │
  │                       │  USDC→XLM conversion?" │
  │                       │←────────────────────────│
  │                       │ Use Marinade pool      │
  │                       │ Liquidity available    │
  │                       │ Slippage: 0.5%         │
  │                       │                         │
  │                       │ 5. Calculate costs     │
  │                       │ Input: 100 USDC        │
  │                       │ Price: 1 USDC = 10 XLM │
  │                       │ Gross output: 1000 XLM │
  │                       │ Slippage (0.5%): 5 XLM │
  │                       │ Gas fees: 0.5 XLM      │
  │                       │ Platform fee: 2%       │
  │                       │ ────────────────────    │
  │                       │ Net to user: 972.5 XLM │
  │                       │                         │
  │                       │ 6. Store quote record  │
  │                       │────────────────────────→ DB
  │                       │ ID: quote_123          │
  │                       │ Status: PENDING        │
  │                       │ Expires: +15 mins      │
  │                       │ Payment addr: TREASURY │
  │                       │←────────────────────────│
  │                       │                         │
  │ 7. Return quote       │                         │
  │←────────────────────  │                         │
  │ {quote_id: 123,       │                         │
  │  max_funding: 100,    │                         │
  │  execution_cost:      │                         │
  │   972.5,              │                         │
  │  service_fee: 27.5,   │                         │
  │  payment_address:     │                         │
  │   TREASURY_WALLET,    │                         │
  │  expires_at: 10:45}   │                         │
  │                       │                         │
```

---

## 4. Payment Processing & Webhook Flow

```
USER'S WALLET (On-Chain)        BLOCKCHAIN           BACKEND SERVER
    │                               │                      │
    │ 1. Sends 100 USDC            │                      │
    │    to Treasury wallet        │                      │
    │────────────────────────────→ │                      │
    │    with memo: quote_123       │                      │
    │                               │ 2. Transaction      │
    │                               │    confirmed on     │
    │                               │    blockchain       │
    │                               │    Status: CONFIRMED│
    │                               │                      │
    │                               │ 3. Webhook listener │
    │                               │    detects txn      │
    │                               │←─────────────────────
    │                               │ Internal trigger    │
    │                               │                      │
    │                               │ 4. Parse payment    │
    │                               │    Extract memo →   │
    │                               │    quote_id: 123    │
    │                               │ Verify amount match │
    │                               │ ✓ Amount correct    │
    │                               │                      │
    │                               │ 5. Update DB        │
    │                               │ Quote status:       │
    │                               │ PENDING → COMMITTED │
    │                               │ Lock quote (prevent │
    │                               │ double spending)    │
    │                               │                      │
    │                               │ 6. Queue execution  │
    │                               │ on destination chain│
    │                               │ (async background   │
    │                               │  job triggered)     │
    │                               │                      │
    │ 7. IMMEDIATE BACKEND ACTION: Execute on destination chain (see next diagram)
    │                               │                      │
```

---

## 5. Cross-Chain Execution (Backend Treasury Action)

```
BACKEND EXECUTION ENGINE              DESTINATION CHAIN          USER WALLET
         │                                    │                       │
         │ 1. Poll quote status               │                       │
         │    Status: COMMITTED               │                       │
         │    (Payment received & locked)     │                       │
         │                                    │                       │
         │ 2. Query DEX liquidity             │                       │
         │─────────────────────────────────→  │                       │
         │ "What's the swap rate for         │                       │
         │  100 USDC → XLM right now?"        │                       │
         │                                    │                       │
         │                        ← Rate info │                       │
         │                        1 USDC =    │                       │
         │                        9.75 XLM   │                       │
         │                                    │                       │
         │ 3. Build transaction (XDR format)  │                       │
         │    Treasury account signs:         │                       │
         │    - "Swap 100 USDC for 975 XLM"   │                       │
         │    - "Send 975 XLM to [user_addr]" │                       │
         │    - Nonce: prev_nonce + 1         │                       │
         │    - Fee: network_fee              │                       │
         │                                    │                       │
         │ 4. Sign with treasury key          │                       │
         │    Add cryptographic signature     │                       │
         │    to transaction                  │                       │
         │                                    │                       │
         │ 5. Submit to blockchain            │                       │
         │─────────────────────────────────→  │                       │
         │ POST /tx (e.g., Stellar horizon)   │                       │
         │ {transaction_envelope: {...}}      │                       │
         │                                    │                       │
         │                        ← Accepted  │                       │
         │                        TX hash:    │                       │
         │                        abc123...   │                       │
         │                                    │                       │
         │ 6. Poll blockchain for status      │                       │
         │─────────────────────────────────→  │                       │
         │ "Is transaction abc123 confirmed?" │                       │
         │                                    │                       │
         │                        ← Status:   │                       │
         │                        PENDING...  │                       │
         │                        (retry)     │                       │
         │                                    │                       │
         │                        ← Status:   │                       │
         │                        CONFIRMED!  │                       │
         │                                    │                       │
         │ 7. Extract results                 │                       │
         │    - Swap executed ✓               │                       │
         │    - 975 XLM transferred           │                       │
         │    - To user address ✓             │                       │
         │                        ────────────────────────────────→  │
         │                                    │ User receives        │
         │                                    │ 975 XLM in wallet    │
         │                                    │                       │
         │ 8. Update ledger                   │                       │
         │    Quote status:                   │                       │
         │    COMMITTED → EXECUTING           │                       │
         │    EXECUTING → COMPLETED           │                       │
         │    TX hash: abc123...              │                       │
         │    Success! ✓                      │                       │
         │                                    │                       │
```

---

## 6. Real-Time Status Updates (Frontend Polling)

```
FRONTEND                    BACKEND API              DATABASE
  │                           │                         │
  │ 1. Quote created          │                         │
  │ Status: PENDING           │                         │
  │ starts polling             │                         │
  │                            │                         │
  │ GET /status/quote_123     │                         │
  │───────────────────────────→│                         │
  │                            │ Query quote status      │
  │                            │────────────────────────→│
  │                            │         Status: PENDING │
  │                            │←────────────────────────│
  │ ← {status: "pending",     │                         │
  │    message: "Waiting      │                         │
  │    for payment..."}        │                         │
  │                            │                         │
  │ [Wait 2 seconds]           │                         │
  │                            │                         │
  │ 2. User sends payment      │                         │
  │    (Off-chain action)      │                         │
  │                            │                         │
  │ GET /status/quote_123     │                         │
  │───────────────────────────→│                         │
  │                            │ Query quote status      │
  │                            │────────────────────────→│
  │                            │      Status: COMMITTED │
  │                            │←────────────────────────│
  │ ← {status: "committed",   │                         │
  │    tx_hash: "tx_123...",  │                         │
  │    message: "Executing    │                         │
  │    swap..."}              │                         │
  │                            │                         │
  │ [Show progress bar 33%]    │                         │
  │ [Wait 2 seconds]           │                         │
  │                            │                         │
  │ 3. Backend is executing    │                         │
  │    cross-chain swap        │                         │
  │                            │                         │
  │ GET /status/quote_123     │                         │
  │───────────────────────────→│                         │
  │                            │ Query execution table   │
  │                            │────────────────────────→│
  │                            │      Status: EXECUTING │
  │                            │ (TX submitted to STL)   │
  │                            │←────────────────────────│
  │ ← {status: "executing",   │                         │
  │    message: "Swap         │                         │
  │    executing on Stellar..."│                         │
  │                            │                         │
  │ [Show progress bar 66%]    │                         │
  │ [Wait 2 seconds]           │                         │
  │                            │                         │
  │ GET /status/quote_123     │                         │
  │───────────────────────────→│                         │
  │                            │ Query execution table   │
  │                            │────────────────────────→│
  │                            │      Status: COMPLETED │
  │                            │ TX hash: xyz789...      │
  │                            │←────────────────────────│
  │ ← {status: "completed",   │                         │
  │    message: "Trade       │                         │
  │    completed!             │                         │
  │    Received 972.5 XLM",   │                         │
  │    executed_at: "..."}     │                         │
  │                            │                         │
  │ [Show progress bar 100%]   │                         │
  │ [Show success screen]      │                         │
  │                            │                         │
```

---

## 7. Multi-Chain Balance Tracking

```
BACKEND TREASURY MANAGEMENT
════════════════════════════════════════════════════════════

Treasury Wallets (Private Keys Secured):

┌─────────────────────────────────────────────┐
│ SOLANA CHAIN TREASURY                       │
├─────────────────────────────────────────────┤
│ Address: SOLAR_TREASURY_PUB_KEY             │
│                                             │
│ Assets:                                     │
│ ├─ SOL:  10.5 (native)                      │
│ ├─ USDC: 5,250.00                           │
│ ├─ USDT: 1,200.00                           │
│ └─ RAY:  500.00                             │
│                                             │
│ Source: User payments + rebalancing         │
│ Use: Execute swaps, fee collection          │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│ STELLAR CHAIN TREASURY                      │
├─────────────────────────────────────────────┤
│ Address: STLR_TREASURY_PUB_KEY              │
│                                             │
│ Assets:                                     │
│ ├─ XLM:  2,900.50 (native)                  │
│ ├─ USDC: 3,100.00                           │
│ └─ EUR:  500.00                             │
│                                             │
│ Source: Swap outputs + rebalancing          │
│ Use: Execute swaps, send to users           │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│ NEAR CHAIN TREASURY                         │
├─────────────────────────────────────────────┤
│ Account: near_treasury.near                 │
│                                             │
│ Assets:                                     │
│ ├─ NEAR: 500.00 (native)                    │
│ ├─ USDC: 2,000.00                           │
│ └─ USDT: 1,500.00                           │
│                                             │
│ Source: User payments + rebalancing         │
│ Use: Execute swaps, fee collection          │
└─────────────────────────────────────────────┘


Frontend: GET /admin/treasury
Response: [
  {chain: "Solana", asset: "USDC", balance: "5,250.00"},
  {chain: "Stellar", asset: "XLM", balance: "2,900.50"},
  {chain: "Near", asset: "NEAR", balance: "500.00"},
  ...
]
```

---

## 8. Request/Response Sequence for Complete Trade

```
FLOW TIMELINE:
═════════════════════════════════════════════════════════════════════

T+0min   Wallet Registration
         POST /wallet/register → User Wallet A Created
         POST /wallet/register → User Wallet B Created
         (Both wallets verified)

T+5min   Token Discovery
         GET /discovery/chain/Solana → List of available tokens
         GET /quote-engine/ohlc → Chart data for selected token

T+10min  Quote Generation
         POST /quote
         {
           user_id: "...",
           funding_chain: "Solana",
           execution_chain: "Stellar",
           funding_asset: "USDC",
           execution_asset: "XLM",
           execution_instructions_base64: "..."
         }
         ← Response: quote_id, payment_address, expires_at (15 mins)

T+12min  Spending Approval
         POST /approval/create
         {quote_id: "..."}
         ← Response: approval_id, challenge_to_sign
         
         User signs challenge with wallet

         POST /approval/submit
         {approval_id: "...", signature: "..."}
         ← Response: approval_confirmed

T+13min  Payment Initiation
         USER ACTION: Send 100 USDC from Solana wallet to Treasury
         (Frontend shows: "Waiting for payment...")
         
         Poll: GET /status/quote_id
         ← {status: "pending"}

T+14min  Payment Detected & Committed
         Blockchain: Transaction confirmed on Solana
         
         Webhook: /webhook/payment (internal)
         {
           chain: "Solana",
           transaction_hash: "...",
           from_address: user_wallet_a,
           to_address: SOLAR_TREASURY,
           amount: "100.00",
           asset: "USDC",
           memo: quote_id
         }
         
         Backend:
         - Validates amount matches quote
         - Locks quote (prevents double-spending)
         - Updates quote status: PENDING → COMMITTED
         - Queues execution task
         
         Frontend Poll:
         GET /status/quote_id
         ← {status: "committed", tx_hash: "..."}
         (Frontend shows: "Executing swap...")

T+15min  Cross-Chain Execution Begins
         Backend Execution Engine:
         1. Query Stellar DEX for swap rate
         2. Build XDR transaction:
            - Swap 100 USDC for ~975 XLM
            - Send 975 XLM to user's Stellar wallet
         3. Sign with treasury private key
         4. Submit to Stellar network
         5. Poll for confirmation
         
         Frontend Poll:
         GET /status/quote_id
         ← {status: "executing"}

T+16min  Swap Confirmed on Destination Chain
         Blockchain: Transaction confirmed on Stellar
         
         Backend:
         - Detects confirmation
         - Updates quote status: EXECUTING → COMPLETED
         - Records execution hash
         - Updates treasury balances
         
         User's Stellar Wallet:
         - Receives 972.5 XLM (after fees)
         - Transaction visible in chain explorer
         
         Frontend Poll:
         GET /status/quote_id
         ← {status: "completed", executed_at: "...", tx_hash: "..."}
         
         Frontend Display:
         ✓ Trade completed successfully!
           You sent: 100 USDC on Solana
           You received: 972.5 XLM on Stellar
           Fee: 27.5 USDC

T+20min  Portfolio Update
         GET /wallet/portfolio
         ← User's updated balances across all chains
```

---

## 9. Error Handling Paths

```
QUOTE CREATION FLOW - Error Scenarios:
═════════════════════════════════════════════════════════════════

Request: POST /quote

Validation 1: Same Chain Check
├─ ✓ Pass: Different chains
└─ ✗ Fail:
   {error: "InvalidParameters", 
    message: "Funding and execution chains must be different"}

Validation 2: Token Whitelisting
├─ ✓ Pass: Both tokens whitelisted
└─ ✗ Fail:
   {error: "InvalidParameters",
    message: "Token UNKNOWN not supported"}

Validation 3: Daily Spending Limit
├─ ✓ Pass: Within limit
└─ ✗ Fail:
   {error: "RiskControl",
    message: "Daily limit exceeded. 
             Current: $1200, Limit: $1000"}

Validation 4: Risk Controls
├─ ✓ Pass: No circuit breakers
└─ ✗ Fail:
   {error: "ServiceUnavailable",
    message: "Solana chain currently unavailable"}

Success: Quote Created
{quote_id: "...", expires_at: "..."}


EXECUTION FLOW - Error Scenarios:
═════════════════════════════════════════════════════════════════

Quote: COMMITTED (payment received)

Try Execute on Destination Chain:

Error 1: Insufficient Liquidity
└─ Can't find enough liquidity on DEX
   ├─ Action: Try alternate DEX
   ├─ Action: Fallback to wrapped token route
   └─ Result: quote status → FAILED
   
Error 2: Trust Line Missing (Stellar specific)
└─ User hasn't authorized receiving this asset
   ├─ Action: Notify user to add trust line
   ├─ Wait for user to set up trust line
   ├─ Retry execution
   └─ Result: If user doesn't respond, quote expires

Error 3: Slippage Exceeded
└─ Market moved, price is now worse
   ├─ Action: Check if within tolerance
   ├─ Action: Try alternative route
   └─ Result: quote status → FAILED (with notification)

Error 4: RPC/Network Error
└─ Can't connect to blockchain
   ├─ Action: Retry with exponential backoff
   ├─ Action: Try backup RPC endpoint
   └─ After 3 failures: Circuit breaker triggers

On Failure:
GET /status/quote_id
← {status: "failed", 
   error_message: "Stellar trust line not found. 
                   User must add trust line for this asset"}


RETRY STRATEGY:
═════════════════════════════════════════════════════════════════

Retriable Errors: Network/RPC issues
├─ Retry 1: After 5 seconds
├─ Retry 2: After 15 seconds
└─ Retry 3: After 30 seconds

Non-Retriable Errors: Validation, insufficient funds, trust lines
├─ No automatic retry
├─ User must fix and retry manually
└─ Offer clear instructions
```

---

## 10. Database Schema Relationships

```
USERS
├─ id (UUID)
├─ created_at
└─ metadata

WALLETS (User's connected wallets)
├─ id (UUID)
├─ user_id (FK → USERS)
├─ chain (Solana|Stellar|Near)
├─ address (unique per chain)
├─ status (unverified|verified|frozen)
├─ verified_at
└─ created_at

QUOTES (Trading quotes generated)
├─ id (UUID)
├─ user_id (FK → USERS)
├─ funding_chain
├─ execution_chain
├─ funding_asset
├─ execution_asset
├─ status (pending|committed|expired|failed)
├─ max_funding_amount
├─ execution_cost
├─ service_fee
├─ payment_address (treasury)
├─ nonce (unique per user per day)
├─ expires_at
├─ created_at
└─ updated_at

EXECUTIONS (Quote execution results)
├─ id (UUID)
├─ quote_id (FK → QUOTES)
├─ user_id (FK → USERS)
├─ status (queued|executing|completed|failed)
├─ funding_tx_hash (payment received)
├─ execution_tx_hash (swap on destination)
├─ executed_at
├─ error_message
└─ created_at

SPENDING_APPROVALS (User authorizations)
├─ id (UUID)
├─ quote_id (FK → QUOTES)
├─ user_id (FK → USERS)
├─ challenge (nonce to sign)
├─ signature
├─ status (pending|approved|expired)
├─ approved_at
└─ created_at

DAILY_SPENDING (Risk management)
├─ id (UUID)
├─ user_id (FK → USERS)
├─ chain
├─ date
├─ amount_spent (decimal)
└─ created_at

TREASURY_BALANCE (Monitoring)
├─ chain
├─ asset
├─ balance (decimal)
├─ last_updated
└─ updated_by
```

---

## 11. Frontend Page State Management

```
State per Page:
═════════════════════════════════════════════════════════════════

ONBOARDING PAGE
├─ Step: register_wallet_1 | verify_wallet_1 | 
│         register_wallet_2 | verify_wallet_2 | complete
├─ wallet_1: {chain, address, status}
├─ wallet_2: {chain, address, status}
├─ loading: boolean
├─ error: string | null
└─ success_message: string | null

TOKEN DISCOVERY PAGE
├─ selected_chain: Chain
├─ tokens: Token[]
├─ selected_token: Token | null
├─ dexes: Dex[]
├─ ohlc_data: OhlcCandle[]
├─ price: decimal
├─ timeframe: '1h' | '4h' | '1d' | '1w'
├─ loading: boolean
└─ error: string | null

TRADE SETUP PAGE
├─ funding_chain: Chain
├─ execution_chain: Chain
├─ funding_amount: string
├─ funding_asset: string
├─ execution_asset: string
├─ quote: Quote | null
├─ quote_expires_in: number (seconds)
├─ loading: boolean
└─ error: string | null

PAYMENT PAGE
├─ quote_id: UUID
├─ payment_address: string
├─ payment_amount: string
├─ status: 'waiting' | 'detected' | 'confirmed'
├─ polling_interval: NodeJS.Timeout
└─ countdown: number (seconds)

EXECUTION STATUS PAGE
├─ quote_id: UUID
├─ current_step: 1-6
├─ steps_completed: string[]
├─ current_status: string
├─ progress_percent: number
├─ funding_tx_hash: string | null
├─ execution_tx_hash: string | null
├─ poll_interval: NodeJS.Timeout
└─ error: string | null

COMPLETION PAGE
├─ quote_id: UUID
├─ trade_summary: {
│   funding_amount,
│   execution_amount,
│   fee,
│   time_taken
│ }
├─ funding_explorer_url: string
├─ execution_explorer_url: string
└─ next_action: 'new_trade' | 'portfolio' | 'home'
```

---

This comprehensive visualization should help your frontend engineer understand:
1. How data flows through the system
2. What happens at each step
3. How to integrate with each API endpoint
4. What state to manage on the frontend
5. How to display progress to users
6. How to handle errors gracefully
