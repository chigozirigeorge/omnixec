# CrossChain Payments - Quick Reference Guide

**TL;DR for Frontend Engineers**

---

## One-Page Process Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    USER'S COMPLETE JOURNEY                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1️⃣  USER REGISTERS (Onboarding - 5-10 minutes)              │
│      └─ POST /wallet/register → Wallet A (Solana)             │
│      └─ POST /wallet/verify → Sign message                     │
│      └─ POST /wallet/register → Wallet B (Stellar)            │
│      └─ POST /wallet/verify → Sign message                     │
│      ✓ User now has 2 verified wallets                         │
│                                                                 │
│  2️⃣  USER EXPLORES (Discovery - 2-5 minutes)                  │
│      └─ GET /discovery/chains → List chains                    │
│      └─ GET /discovery/chain/{chain} → Tokens & DEXes         │
│      └─ GET /quote-engine/ohlc → Show charts                  │
│      ✓ User picks token: "I want to swap USDC"               │
│                                                                 │
│  3️⃣  USER INITIATES TRADE (Setup - 1 minute)                 │
│      └─ POST /quote → Generate price quote                     │
│      └─ Show: "Send 100 USDC → Receive 972.5 XLM"            │
│      └─ Show: "Expires in 15 minutes"                          │
│      ✓ Quote ready                                             │
│                                                                 │
│  4️⃣  USER APPROVES & PAYS (Payment - 3-5 minutes)            │
│      └─ POST /approval/create → Generate challenge            │
│      └─ User signs challenge                                   │
│      └─ POST /approval/submit → Register approval             │
│      └─ Display: "Send 100 USDC to: TREASURY_ADDRESS"        │
│      └─ Frontend: Poll GET /status/{quote_id}                │
│      └─ User sends payment from their wallet (OFF-CHAIN)      │
│      ✓ Payment detected by backend                            │
│                                                                 │
│  5️⃣  BACKEND AUTO-EXECUTES (Execution - 1-3 minutes)        │
│      └─ Webhook detects payment on Solana                     │
│      └─ Backend queries DEX for swap rate                      │
│      └─ Backend builds & signs transaction                     │
│      └─ Backend submits to Stellar                            │
│      └─ User receives 972.5 XLM automatically                 │
│      └─ Frontend: Shows real-time progress                     │
│      ✓ Trade completed ✓                                       │
│                                                                 │
│  6️⃣  COMPLETION (Final - instant)                             │
│      └─ Display: "Trade successful!"                           │
│      └─ Show: Transaction hashes                               │
│      └─ Show: Balance updates                                  │
│      ✓ User can trade again or view portfolio                 │
│                                                                 │
│  ⏱️  TOTAL TIME: ~15-25 minutes                               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Frontend Checklist

### Onboarding Flow

- [ ] Page 1: Wallet registration form
  - [ ] Chain selector (Solana, Stellar, Near)
  - [ ] Address input field
  - [ ] Submit button → POST /wallet/register

- [ ] Page 2: Wallet verification
  - [ ] Show "Please sign with your wallet"
  - [ ] Trigger wallet to sign nonce
  - [ ] Submit signature → POST /wallet/verify
  - [ ] Show "Verified ✓"

- [ ] Page 3: Register 2nd wallet (repeat pages 1-2)

### Discovery Flow

- [ ] Chain selector with icons
- [ ] Token list showing:
  - [ ] Token logo
  - [ ] Symbol (SOL, USDC, XLM)
  - [ ] Current price
  - [ ] Available DEXes badge
  - [ ] Click → Show details

- [ ] Token details modal:
  - [ ] Current price + 24h change
  - [ ] OHLC chart (candlestick)
  - [ ] Chart timeframe selector (1h, 4h, 1d, 1w)
  - [ ] DEXes supporting this token
  - [ ] [Trade This Token] button

### Trade Setup Flow

- [ ] Input fields:
  - [ ] Amount to send (e.g., "100 USDC")
  - [ ] Execution chain dropdown
  - [ ] Receive token selector

- [ ] Get Quote button
  - [ ] POST /quote
  - [ ] Show loading state
  - [ ] Display response:
    - [ ] Amount to receive
    - [ ] Fee breakdown
    - [ ] Exchange rate
    - [ ] Expiration time (countdown timer)

### Payment Flow

- [ ] Display payment instructions:
  - [ ] Treasury wallet address (copy button)
  - [ ] Amount to send (copy button)
  - [ ] Memo/Reference (quote_id)
  - [ ] QR code

- [ ] Show status:
  - [ ] "Waiting for payment..."
  - [ ] Poll GET /status/{quote_id} every 5 seconds
  - [ ] Update status in real-time

### Execution Status Flow

- [ ] Progress bar (0-100%)

- [ ] Step indicators:
  - [ ] ✓ Payment received
  - [ ] ⏳ Executing swap
  - [ ] ○ Finalizing

- [ ] Real-time updates via polling
  - [ ] Poll every 5 seconds
  - [ ] Update progress bar
  - [ ] Show transaction hashes
  - [ ] Show step completion

- [ ] Success or failure display:
  - [ ] Success: Show amount received
  - [ ] Failure: Show error with recovery instructions

### Completion Flow

- [ ] Show summary:
  - [ ] Amount sent
  - [ ] Amount received
  - [ ] Fees paid
  - [ ] Total execution time

- [ ] Links to explorers:
  - [ ] Link to Solana explorer (funding tx)
  - [ ] Link to Stellar explorer (execution tx)

- [ ] Action buttons:
  - [ ] [New Trade]
  - [ ] [View Portfolio]
  - [ ] [Home]

---

## Key APIs to Call

### 1. Wallet Management

```bash
# Register wallet
POST /wallet/register
Body: {user_id, chain, address}
Response: {wallet_id, status: "unverified"}

# Verify wallet
POST /wallet/verify
Body: {user_id, chain, address, signature}
Response: {wallet_id, verified: true}

# Get user wallets
GET /wallet/user/{user_id}
Response: [{wallet_id, chain, address, status}, ...]
```

### 2. Discovery

```bash
# Get all chains
GET /discovery/chains
Response: ["Solana", "Stellar", "Near"]

# Get all tokens on chain
GET /discovery/chain/{chain}
Response: {chain, dexes: [...], supported_tokens: [...]}

# Get chart data
GET /quote-engine/ohlc?asset=SOL&chain=Solana&timeframe=1h&limit=24
Response: [{timestamp, open, high, low, close, volume}, ...]
```

### 3. Quote & Trading

```bash
# Create quote
POST /quote
Body: {user_id, funding_chain, execution_chain, funding_asset, execution_asset, execution_instructions_base64}
Response: {quote_id, max_funding_amount, execution_cost, service_fee, payment_address, expires_at, nonce}

# Check status
GET /status/{quote_id}
Response: {status, transaction_hash, executed_at, error_message}

# Create approval
POST /approval/create
Body: {quote_id}
Response: {approval_id, challenge}

# Submit approval
POST /approval/submit
Body: {approval_id, signature}
Response: {approved: true}
```

### 4. Treasury & Admin

```bash
# Get treasury balances
GET /admin/treasury
Response: [{chain, asset, balance, last_updated}, ...]

# Health check
GET /health
Response: {status, circuit_breakers: [{chain, active, reason}, ...]}
```

---

## Polling Strategy

```javascript
// Poll every 5 seconds while status is "pending" or "executing"
const pollStatus = setInterval(async () => {
  const response = await fetch(`/status/${quoteId}`);
  const data = await response.json();
  
  updateUI(data);
  
  // Stop polling when complete
  if (data.status === 'completed' || data.status === 'failed') {
    clearInterval(pollStatus);
  }
}, 5000);
```

---

## Error Scenarios & Recovery

| Error | What to Show | Recovery |
|-------|--------------|----------|
| Quote expired | "Quote expired. Generate new?" | [Get New Quote] button |
| Daily limit hit | "Daily limit reached: $X/$Y" | "Try tomorrow or smaller amount" |
| Circuit breaker active | "Chain temporarily unavailable" | "Check /health endpoint" |
| Execution failed | Show error message + reason | Offer to retry or support contact |
| Network error | "Connection lost" | Show retry button |
| Trust line missing (Stellar) | "Must add XLM trust line first" | Link to wallet instructions |

---

## State to Maintain (Frontend)

```javascript
{
  user: {
    id: "uuid",
  },
  
  wallets: [
    {
      walletId: "uuid",
      chain: "Solana",
      address: "...",
      status: "verified"
    },
    {
      walletId: "uuid",
      chain: "Stellar",
      address: "...",
      status: "verified"
    }
  ],
  
  currentQuote: {
    quoteId: "uuid",
    fundingAmount: "100",
    fundingAsset: "USDC",
    executionAsset: "XLM",
    expiresAt: "2026-01-07T10:45:00Z",
    status: "pending"
  },
  
  execution: {
    status: "executing",
    fundingTxHash: "...",
    executionTxHash: "...",
    progress: 66
  }
}
```

---

## Transaction Explorers (Add Links)

```typescript
function getExplorerUrl(chain: string, txHash: string): string {
  const urls = {
    Solana: `https://explorer.solana.com/tx/${txHash}`,
    Stellar: `https://stellar.expert/explorer/public/tx/${txHash}`,
    Near: `https://explorer.near.org/transactions/${txHash}`,
  };
  return urls[chain];
}
```

---

## Wallet Integration

### Solana (Phantom)
```javascript
// Request signature
const message = "Sign this to verify ownership";
const encodedMessage = new TextEncoder().encode(message);
const signedMessage = await window.solana.signMessage(encodedMessage);
// Send signedMessage to backend
```

### Stellar (Albedo/Freighter)
```javascript
// Request signature
const signature = await window.albedo.signMessage({
  message: "Sign this to verify ownership"
});
// Send signature to backend
```

### NEAR (NEAR Wallet)
```javascript
// Request signature
const signature = await window.near.signMessage({
  message: "Sign this to verify ownership"
});
// Send signature to backend
```

---

## API Timeout Settings

- **Quote Generation**: 30 seconds
- **Status Polling**: 5-10 second intervals, 2-3 minute timeout
- **Health Check**: 15 seconds
- **Wallet Register**: 10 seconds

---

## Rate Limits

- Wallet registration: 10/minute per user
- Quote generation: 30/minute per user
- Status checks: 100/minute per user
- Health checks: 10/second globally

---

## Performance Tips

1. **Cache OHLC data** - Update every 5 minutes, not every page load
2. **Debounce search** - Wait 300ms after user stops typing to query tokens
3. **Lazy load portfolios** - Don't load on every navigation
4. **Memoize quote calculations** - Prevent re-renders on status updates
5. **Use Web Workers** - For chart rendering if heavy data
6. **Preload explorers** - Prepare explorer links before showing

---

## Testing Checklist

- [ ] Can register wallet on Solana
- [ ] Can register wallet on Stellar  
- [ ] Can register wallet on NEAR
- [ ] Wallet verification works for all chains
- [ ] Can view tokens on each chain
- [ ] Charts display OHLC data correctly
- [ ] Quote generation works
- [ ] Quote countdown timer counts down
- [ ] Quote expires and shows error
- [ ] Status polling updates in real-time
- [ ] Can see transaction hashes
- [ ] Can click explorer links
- [ ] Error messages are user-friendly
- [ ] Daily limit blocking works
- [ ] Circuit breaker affects UI
- [ ] Portfolio shows updated balances

---

## Environment Setup

```bash
# Install dependencies
npm install axios zustand react-router-dom chart.js react-chartjs-2

# Create .env.local
REACT_APP_API_URL=http://localhost:8080
REACT_APP_ENVIRONMENT=development

# Run dev server
npm start
```

---

## Support & Debugging

**Common Issues:**

1. **CORS errors** → Backend needs CORS headers
2. **Quote expires immediately** → Check backend clock sync
3. **Polling doesn't detect changes** → Increase poll frequency
4. **Chart not rendering** → Check OHLC data format
5. **Wallet not signing** → Check if wallet extension installed

**Debug Tips:**

```javascript
// Add to console
localStorage.setItem('DEBUG', 'true');

// View all API calls
window.fetch = (async (url, options) => {
  console.log('API Call:', url, options);
  return originalFetch(url, options);
}).bind(window);
```

---

## Deployment Checklist

- [ ] Set `REACT_APP_ENVIRONMENT=production`
- [ ] Update `REACT_APP_API_URL` to production backend
- [ ] Enable HTTPS only
- [ ] Set Content Security Policy headers
- [ ] Minify bundle
- [ ] Enable caching headers
- [ ] Test all chains before go-live
- [ ] Set up error logging (Sentry)
- [ ] Prepare status page
- [ ] Notify users of maintenance

---

## Production URLs

| Component | Mainnet |
|-----------|---------|
| Solana RPC | https://api.mainnet-beta.solana.com |
| Stellar Horizon | https://horizon.stellar.org |
| NEAR RPC | https://rpc.mainnet.near.org |
| Backend API | https://api.crosschain.example.com |

---

**For Questions**: Contact backend team or check full documentation in:
- `API_FLOW_GUIDE.md` - Detailed API walkthrough
- `SYSTEM_ARCHITECTURE.md` - System diagrams
- `FRONTEND_IMPLEMENTATION_GUIDE.md` - Code examples
