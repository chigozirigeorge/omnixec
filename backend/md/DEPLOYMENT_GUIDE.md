# Smart Contract Deployment Guide

Your codebase has **three production-ready smart contracts** for Stellar, NEAR, and Solana. This guide walks through deploying them.

## Prerequisites

### All Chains
- Testnet account created and funded
- Basic understanding of each blockchain

### NEAR
```bash
# Install NEAR CLI v4 or higher
npm install -g near-cli-rs

# Or build from source
git clone https://github.com/near/near-cli-rs.git
cd near-cli-rs && make install

# Login to testnet
near-cli-rs account auth-with-keychain --network testnet
```

### Solana
```bash
# Install Solana CLI
bash -c "$(curl -sSfL https://release.solana.com/stable/install)"

# Set cluster to testnet
solana config set --url https://api.testnet.solana.com

# Airdrop SOL
solana airdrop 2
```

### Stellar
```bash
# Install Soroban CLI
npm install -g @stellar/soroban-cli

# Set testnet network
soroban config network add --name testnet \
  --rpc-url https://soroban-testnet.stellar.org:443 \
  --network-passphrase "Test SDF Network ; September 2015"
```

---

## 1. NEAR Smart Contract Deployment

**Account ID:** `omnixec.near`

### Step 1: Build the Contract

```bash
cd contracts/near-swap

# Install dependencies
cargo install near-cli-rs

# Build to WASM
cargo build --release --target wasm32-unknown-unknown
```

### Step 2: Deploy

```bash
# Create subaccount for contract
near-cli-rs account create-account fund-myself omnixec.near 10000000000000000000

# Deploy contract
near-cli-rs contract deploy omnixec.near \
  file target/wasm32-unknown-unknown/release/near_swap.wasm

# Initialize with config
near-cli-rs call omnixec.near new json-args \
  '{"treasury":"treasury.omnixec.near","dex_contract":"ref-finance.testnet","fee_bps":10}'
```

### Step 3: Verify Deployment

```bash
# Check contract state
near-cli-rs view omnixec.near get_treasury

# Should return: "treasury.omnixec.near"
```

---

## 2. Solana Program Deployment

### Step 1: Build the Program

```bash
cd contracts/solana

# Install target
rustup target add wasm32-unknown-unknown

# Build
cargo build --release --target wasm32-unknown-unknown
```

### Step 2: Deploy to Testnet

```bash
# Get your wallet address
solana address

# Airdrop more SOL if needed
solana airdrop 2

# Deploy
solana deploy target/wasm32-unknown-unknown/release/solana_swap.so \
  --url https://api.testnet.solana.com
```

### Step 3: Verify

```bash
# Check account info
solana account <PROGRAM_ID> --url https://api.testnet.solana.com

# Should show: "executable: true"
```

---

## 3. Stellar Soroban Smart Contract Deployment

### Step 1: Build the Contract

```bash
cd contracts/stellar

# Install dependencies
rustup target add wasm32-unknown-unknown

# Build contract
cargo build --release --target wasm32-unknown-unknown
```

### Step 2: Deploy to Testnet

```bash
# Create a Stellar testnet account (if you don't have one)
# Go to: https://lab.stellar.org

# Deploy the contract
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/stellar_swap.wasm \
  --source omnixec-testnet-account \
  --network testnet
```

### Step 3: Invoke Contract

```bash
# Invoke get_treasury function
soroban contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source omnixec-testnet-account \
  --function get_treasury
```

---

## Environment Setup for Testing

### Create `.env` for Backend

```env
# NEAR Contract
NEAR_SWAP_CONTRACT=omnixec.near
NEAR_TREASURY=treasury.omnixec.near
NEAR_DEX=ref-finance.testnet

# Solana Program
SOLANA_SWAP_PROGRAM=<YOUR_PROGRAM_ID>
SOLANA_TREASURY=<YOUR_WALLET_ADDRESS>

# Stellar Contract
STELLAR_SWAP_CONTRACT=<YOUR_CONTRACT_ID>
STELLAR_TREASURY=<YOUR_STELLAR_ACCOUNT>
```

---

## Testing the Contracts

### NEAR: Execute a Swap

```bash
near-cli-rs call omnixec.near swap json-args \
  '{
    "user_id":"user.testnet",
    "input_token":"token.omnixec.near",
    "output_token":"other-token.testnet",
    "amount_in":"1000000000000000000",
    "min_amount_out":"900000000000000000"
  }' \
  --depositor treasury.omnixec.near \
  --attached-gas 200000000000000
```

### Solana: Execute a Swap

```bash
# Send instruction to program
solana program invoke-method <PROGRAM_ID> swap \
  --method-args '[amount_in:1000000, min_amount_out:900000]'
```

### Stellar: Execute a Swap

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source omnixec-testnet-account \
  --function swap \
  --function-args '{"user":"GAA...","treasury":"GBB...","input_token":"GCC...","output_token":"GDD...","amount_in":1000000,"min_amount_out":900000,"dex_address":"GEE..."}'
```

---

## Troubleshooting

### NEAR Build Fails with Secp256k1 Error
```bash
# Use an older SDK or wait for fix
cd contracts/near-swap
cargo clean
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --release --target wasm32-unknown-unknown
```

### Solana Deployment Insufficient Funds
```bash
# Request more SOL on testnet
solana airdrop 5 --url https://api.testnet.solana.com
```

### Stellar Contract Not Found
```bash
# Verify contract was deployed
soroban contract info <CONTRACT_ID> --network testnet --source account-name
```

---

## Next Steps

1. **Set environment variables** in your backend `.env`
2. **Update executor implementations** to use actual deployed contract addresses
3. **Run integration tests** against testnet contracts
4. **Monitor gas usage** to optimize for mainnet deployment

