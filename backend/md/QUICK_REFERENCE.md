# Quick Reference: Deployment Cheatsheet

Fast reference guide for deploying the crosschain payments backend.

---

## Pre-Deployment Checklist (30 minutes)

```bash
# 1. Verify code compiles
cargo check --all
cargo build --release

# 2. Run tests
cargo test --lib
cargo test --test '*' 

# 3. Verify no warnings
cargo clippy

# 4. Check security
cargo audit

# 5. Format code
cargo fmt --check
```

---

## Environment Setup

### Solana Setup (15 minutes)

```bash
# Install Solana
sh -c "$(curl -sSfL https://release.solana.com/v1.17.0/install)"

# Create keypair
solana-keygen new --outfile ~/solana-deployment-keypair.json

# Configure testnet
solana config set --url devnet
solana config set --keypair ~/solana-deployment-keypair.json

# Fund account
solana airdrop 10

# Verify
solana balance
```

### Stellar Setup (10 minutes)

```bash
# Use web wallet
# https://testnet.mynearwallet.com (for testnet)
# OR https://stellar.org/wallets (for mainnet)

# Fund via Friendbot (testnet)
curl "https://friendbot.stellar.org?addr=$ACCOUNT"

# Verify
curl https://horizon-testnet.stellar.org/accounts/$ACCOUNT | jq '.balances'
```

### NEAR Setup (15 minutes)

```bash
# Install NEAR CLI
npm install -g near-cli-rs

# Create account
near key-pair-generator testnet > near-keys.json

# Fund via web wallet
# https://testnet.mynearwallet.com

# Verify
near account view-account-summary
```

---

## Deployment Commands

### Solana Deployment

```bash
# 1. Create treasury account
export USDC_MINT="EPjFWaJy47gIdZiohMzvoi52LjxjxJmMu3pfFLsprA7"
spl-token create-account $USDC_MINT \
  --owner $(solana address)

# 2. Configure backend
export SOLANA_RPC_URL="https://api.devnet.solana.com"
export SOLANA_KEYPAIR_PATH="$HOME/solana-deployment-keypair.json"
export SOLANA_TREASURY_ACCOUNT="<account-from-step-1>"

# 3. Start backend
cargo run --release

# 4. Verify on chain explorer
# https://explorer.solana.com (mainnet)
# https://solscan.io/account/$SOLANA_TREASURY_ACCOUNT
```

### Stellar Deployment

```bash
# 1. Create issuer account (web wallet)
# Save: $ISSUER_PUBLIC, $ISSUER_SECRET

# 2. Create treasury account
# Save: $TREASURY_PUBLIC, $TREASURY_SECRET

# 3. Configure backend
export STELLAR_HORIZON_URL="https://horizon-testnet.stellar.org"
export STELLAR_ISSUER_SECRET="SBXXX..."
export STELLAR_TREASURY_ACCOUNT="GBXXX..."

# 4. Set up trust lines (via web wallet)
# 1. Add trust line to USDC
# 2. Add trust line to USDT

# 5. Start backend
cargo run --release

# 6. Verify on explorer
# https://stellar.expert/explorer/testnet/account/$TREASURY_PUBLIC
```

### NEAR Deployment

```bash
# 1. Create account
ACCOUNT="myapp.testnet"
near account create-account implicit-account

# 2. Fund account (web wallet)
# https://testnet.mynearwallet.com

# 3. Deploy contract
cd contracts/treasury
cargo build --target wasm32-unknown-unknown --release

near contract deploy-new $ACCOUNT \
  ./target/wasm32-unknown-unknown/release/treasury.wasm

# 4. Initialize
near contract call-function as-transaction $ACCOUNT \
  new '{"owner_id": "myaccount.testnet"}' \
  prepaid-gas '100000000000000' \
  attached-deposit '0'

# 5. Start backend
cargo run --release

# 6. Verify
near account view-account-summary $ACCOUNT
```

---

## Health Checks (1 minute)

### Solana Health Check

```bash
# RPC connectivity
curl https://api.devnet.solana.com -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}'

# Treasury balance
spl-token balance $TREASURY_ACCOUNT

# Expected: > 0 tokens
```

### Stellar Health Check

```bash
# Horizon connectivity
curl https://horizon-testnet.stellar.org/

# Treasury balance
curl https://horizon-testnet.stellar.org/accounts/$TREASURY_PUBLIC | \
  jq '.balances'

# Expected: USDC balance > 0
```

### NEAR Health Check

```bash
# Account balance
near account view-account-summary $ACCOUNT

# Contract state
near contract call-function as-read-only $ACCOUNT \
  get_balance '{"token_contract":"usdc.testnet","account_id":"alice.testnet"}'
```

### Backend Health Check

```bash
# API health
curl http://localhost:8080/health

# Expected: HTTP 200 with {"status":"healthy"}

# Database connectivity
curl http://localhost:8080/status/test-quote-id

# Expected: HTTP 200 or 404 (not 500)
```

---

## Common Errors & Quick Fixes

### Solana

| Error | Fix |
|-------|-----|
| "Bad sequence number" | Run `solana account <addr>` to refresh |
| "Insufficient balance" | Run `solana airdrop 10` |
| "Invalid public key" | Check key format (starts with G) |
| "RPC timeout" | Switch RPC: `solana config set --url <url>` |

### Stellar

| Error | Fix |
|-------|-----|
| "Invalid sequence" | Create new account (sequence out of sync) |
| "No trust line" | Add trust line via web wallet |
| "Rate limited" | Increase timeout, retry after 5s |
| "Not found" | Verify account exists: `curl horizon-testnet.stellar.org/accounts/$ACCOUNT` |

### NEAR

| Error | Fix |
|-------|-----|
| "Account not found" | Create account via web wallet |
| "Insufficient balance" | Fund account on testnet |
| "Gas limit exceeded" | Increase gas: `prepaid-gas '300000000000000'` |
| "Action/call not found" | Verify contract deployed: `near account view-account-summary $CONTRACT` |

---

## Quick Test Workflows

### Solana Quote Flow (2 minutes)

```bash
# 1. Create quote
curl -X POST http://localhost:8080/quote \
  -H "Content-Type: application/json" \
  -d '{
    "from_asset": "SOL",
    "to_asset": "USDC",
    "amount": "1.0",
    "from_chain": "solana",
    "to_chain": "solana"
  }'
# Response: {"quote_id": "...", ...}

# 2. Commit quote
curl -X POST http://localhost:8080/commit \
  -H "Content-Type: application/json" \
  -d '{"quote_id": "..."}'
# Response: {"status": "Pending", ...}

# 3. Check status
curl http://localhost:8080/status/QUOTE_ID
# Response: {"status": "Executed", "tx_hash": "...", ...}
```

### Stellar Quote Flow (2 minutes)

```bash
# 1. Create quote
curl -X POST http://localhost:8080/quote \
  -H "Content-Type: application/json" \
  -d '{
    "from_asset": "USDC",
    "to_asset": "USDT",
    "amount": "100.00",
    "from_chain": "stellar",
    "to_chain": "stellar"
  }'

# 2. Commit quote
curl -X POST http://localhost:8080/commit \
  -H "Content-Type: application/json" \
  -d '{"quote_id": "..."}'

# 3. Check on explorer
# https://stellar.expert/explorer/testnet/tx/TX_HASH
```

### NEAR Quote Flow (2 minutes)

```bash
# 1. Create quote
curl -X POST http://localhost:8080/quote \
  -H "Content-Type: application/json" \
  -d '{
    "from_asset": "NEAR",
    "to_asset": "USDC",
    "amount": "10.0",
    "from_chain": "near",
    "to_chain": "near"
  }'

# 2. Commit quote
curl -X POST http://localhost:8080/commit \
  -H "Content-Type: application/json" \
  -d '{"quote_id": "..."}'

# 3. Verify on explorer
# https://explorer.mainnet.near.org/transactions/TX_HASH
```

---

## Monitoring Quick Start

### Prometheus Setup (5 minutes)

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'crosschain-backend'
    static_configs:
      - targets: ['localhost:8080']
```

### Key Metrics to Watch

```bash
# Request rate
curl http://localhost:8080/metrics | grep -E "http_requests_total"

# Error rate
curl http://localhost:8080/metrics | grep -E "errors_total"

# Latency
curl http://localhost:8080/metrics | grep -E "request_duration"

# Database connections
curl http://localhost:8080/metrics | grep -E "pool_connections"
```

---

## Production Rollout Checklist

### Day 1: Setup
- [ ] All three blockchains configured
- [ ] Treasury accounts created
- [ ] Backend deployed to staging
- [ ] Health checks passing
- [ ] Monitoring enabled

### Day 2: Testing
- [ ] All workflows tested end-to-end
- [ ] Load test with 50 concurrent users
- [ ] Webhook callbacks working
- [ ] Database backups verified

### Day 3: Monitoring
- [ ] 24-hour monitoring
- [ ] Error rates < 0.1%
- [ ] Response time < 50ms p95
- [ ] No OOM or crashes

### Day 4: Production
- [ ] Deploy to production
- [ ] Gradual traffic ramp (10% → 25% → 50% → 100%)
- [ ] Monitor closely each step
- [ ] Rollback plan ready

---

## Emergency Procedures

### Database Connectivity Lost

```bash
# 1. Check database status
pg_isready -h localhost -p 5432

# 2. Restart database
docker restart postgres

# 3. Check migrations ran
sqlx migrate --database-url $DATABASE_URL status

# 4. Restart backend
cargo run --release
```

### RPC Node Unreachable

```bash
# Solana: Switch RPC
export SOLANA_RPC_URL="https://api.mainnet-beta.solana.com"

# Stellar: Switch Horizon
export STELLAR_HORIZON_URL="https://horizon.stellar.org"

# NEAR: Switch RPC
export NEAR_RPC_URL="https://rpc.mainnet.near.org"

# Restart backend with new RPC
cargo run --release
```

### High Error Rate

```bash
# 1. Check logs
tail -f backend.log | grep ERROR

# 2. Check database connections
curl http://localhost:8080/health

# 3. Check RPC status
curl $BLOCKCHAIN_RPC_URL

# 4. Check rate limits
# If rate limited, implement exponential backoff

# 5. Temporarily disable affected chain
# Set SOLANA_ENABLED=false in .env
# Restart backend
```

---

## Useful Commands

### Backend

```bash
# Start with logging
RUST_LOG=debug cargo run --release

# Run specific test
cargo test test_quote_creation -- --nocapture

# Format code
cargo fmt

# Check for issues
cargo clippy

# Build release
cargo build --release
```

### Solana

```bash
# Check key
solana-keygen pubkey ~/solana-deployment-keypair.json

# View transactions
solana transaction-history $ADDRESS

# Create token account
spl-token create-account $MINT
```

### Stellar

```bash
# Generate keypair
stellar-keygen

# View account
curl https://horizon-testnet.stellar.org/accounts/$ACCOUNT

# Build transaction
stellar tx build --source $ACCOUNT --sequence 1

# Sign transaction
stellar tx sign --keypair $SECRET
```

### NEAR

```bash
# Create key
near key-pair-generator testnet

# Check balance
near account view-account-summary $ACCOUNT

# Call contract
near contract call-function as-read-only $CONTRACT function '{}' \

# Call with state change
near contract call-function as-transaction $CONTRACT \
  function '{}' prepaid-gas '100000000000000'
```

---

## Links & Resources

### Documentation
- [API Reference](./API_DOCUMENTATION.md)
- [Code Refactoring](./CODE_REFACTORING_GUIDE.md)
- [Solana Guide](./DEPLOY_SOLANA.md)
- [Stellar Guide](./DEPLOY_STELLAR.md)
- [NEAR Guide](./DEPLOY_NEAR.md)

### Explorers
- Solana: https://explorer.solana.com (use devnet dropdown)
- Stellar: https://stellar.expert/explorer/testnet
- NEAR: https://explorer.testnet.near.org

### Developer Docs
- Solana: https://docs.solana.com
- Stellar: https://developers.stellar.org
- NEAR: https://docs.near.org

### CLI Tools
- Solana: `solana --version`
- Stellar: `stellar --help`
- NEAR: `near --help`

---

## Performance Targets

| Metric | Target | How to Check |
|--------|--------|--------------|
| API Response Time | <50ms p95 | `curl -w "@curl-format.txt"` |
| DB Connections | 200 max | Check `src/main.rs` pool config |
| Error Rate | <0.1% | Monitor dashboard or logs |
| Uptime | >99.9% | Monitor alerting system |
| Quote TTL | 10 minutes | Config in code |
| Cache Hit Rate | >90% | Monitor metrics |

---

## Success Indicators

✅ Deployment successful if:
1. All health checks pass
2. Quote creation works end-to-end
3. Transactions settle on blockchain
4. No errors in logs over 24 hours
5. Response time < 50ms p95
6. All three blockchains operational

---

**Quick Start**: 
1. Follow "Pre-Deployment Checklist"
2. Choose blockchain (Solana/Stellar/NEAR)
3. Follow "X Deployment" section
4. Run health checks
5. Test quote workflow
6. Monitor metrics

**Estimated Time to Production**: 2-3 days (setup + testing + monitoring)

