# NEAR Deployment Guide

Complete step-by-step guide for deploying crosschain payments backend on NEAR.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Network Setup](#network-setup)
3. [Smart Contract Development](#smart-contract-development)
4. [Contract Deployment](#contract-deployment)
5. [Account Management](#account-management)
6. [Function Calls & State Updates](#function-calls--state-updates)
7. [Cross-Contract Calls](#cross-contract-calls)
8. [Security Considerations](#security-considerations)
9. [Testing Procedures](#testing-procedures)
10. [Monitoring & Maintenance](#monitoring--maintenance)

---

## Prerequisites

### Required Software

```bash
# Install Node.js and npm
curl https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18
nvm use 18
node --version  # v18.x.x

# Install NEAR CLI
npm install -g near-cli-rs

# Verify installation
near --version
near account list

# Install Rust (if developing smart contracts)
rustup default stable
rustup target add wasm32-unknown-unknown
cargo --version
```

### Environment Configuration

```bash
# Create .env.near
cat > .env.near << 'EOF'
# Network Selection
NEAR_NETWORK=testnet  # testnet or mainnet

# RPC Endpoints
NEAR_RPC_URL=https://rpc.testnet.near.org
NEAR_RPC_URL_MAINNET=https://rpc.mainnet.near.org
NEAR_INDEXER_URL=https://indexer.near.org

# Account Configuration
NEAR_ACCOUNT_ID=your-account.testnet
NEAR_MASTER_ACCOUNT=your-account.testnet

# Contract Accounts
NEAR_TREASURY_CONTRACT=treasury.your-account.testnet
NEAR_TOKEN_CONTRACT=token.your-account.testnet
NEAR_EXCHANGE_CONTRACT=exchange.your-account.testnet

# Credentials
NEAR_PRIVATE_KEY=ed25519:XXXX...
NEAR_PUBLIC_KEY=ed25519:XXXX...

# Gas Configuration
NEAR_GAS_LIMIT=300000000000000  # 300 TGas
NEAR_GAS_PRICE=100000000  # 0.1 yocto per gas

# USDC/USDT Configuration
USDC_CONTRACT=usdc.testnet
USDT_CONTRACT=usdt.testnet
EOF

source .env.near
```

---

## Network Setup

### 1. Create NEAR Account

```bash
#!/bin/bash
# Generate keypair
near key-pair-generator testnet > near-keys.json

# Create account on testnet
ACCOUNT="yourname.testnet"
near account create-account implicit-account > $ACCOUNT.json

# Or use NEAR web wallet
# https://testnet.mynearwallet.com

# View account details
near account view-account-summary
```

### 2. Fund NEAR Account

**Testnet (Free):**

```bash
# Request testnet tokens
curl -X POST https://helper.testnet.near.org/account/$ACCOUNT/allowance \
  -H 'Content-Type: application/json' \
  -d '{"newAccountId":"'"$ACCOUNT"'","publicKey":"'"$PUBLIC_KEY"'"}'

# Should receive 200 NEAR
near account view-account-summary
```

**Mainnet (Paid):**

```bash
# Purchase NEAR from exchange
# Send to your account address from step 1
# Minimum: ~10 NEAR (depends on contract size)

near account view-account-summary
# Expected: balance in NEAR
```

### 3. Verify Account Setup

```bash
# Check account balance
near account view-account-summary

# Check access keys
near account list-keys

# Expected output:
# Network: testnet
# Account ID: yourname.testnet
# Balance: 200.000000000000000000000000
# Keys: [...]
```

---

## Smart Contract Development

### 1. Project Structure

```bash
mkdir near-contracts
cd near-contracts

# Create Rust smart contract project
cargo init --name treasury --lib

cat > Cargo.toml << 'EOF'
[package]
name = "treasury"
version = "0.1.0"
edition = "2021"

[dependencies]
near-sdk = "4.0"
near-contract-standards = "4.0"

[[bin]]
name = "treasury"
path = "src/lib.rs"
crate-type = ["cdylib"]

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
EOF
```

### 2. Create Treasury Contract

```rust
// contracts/treasury/src/lib.rs

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{
    env, near_bindgen, AccountId, Balance, Promise,
    PromiseOrValue, log, ext_contract,
};
use near_sdk::collections::LookupMap;
use near_contract_standards::fungible_token::FungibleTokenCore;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Treasury {
    // Treasury owner (multisig or DAO)
    owner_id: AccountId,
    
    // Authorized payment processors
    processors: LookupMap<AccountId, bool>,
    
    // Token balances by token contract and owner
    balances: LookupMap<(AccountId, AccountId), Balance>,
    
    // Daily spending limits per account
    daily_limits: LookupMap<AccountId, Balance>,
    
    // Daily spending tracked (resets daily)
    daily_spent: LookupMap<(AccountId, u64), Balance>,
}

impl Default for Treasury {
    fn default() -> Self {
        Self {
            owner_id: env::signer_account_id(),
            processors: LookupMap::new(b"p"),
            balances: LookupMap::new(b"b"),
            daily_limits: LookupMap::new(b"l"),
            daily_spent: LookupMap::new(b"s"),
        }
    }
}

#[near_bindgen]
impl Treasury {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            processors: LookupMap::new(b"p"),
            balances: LookupMap::new(b"b"),
            daily_limits: LookupMap::new(b"l"),
            daily_spent: LookupMap::new(b"s"),
        }
    }

    // ====== Admin Functions ======

    pub fn add_processor(&mut self, processor_id: AccountId) {
        self.assert_owner();
        self.processors.insert(&processor_id, &true);
        log!("✓ Processor {} added", processor_id);
    }

    pub fn remove_processor(&mut self, processor_id: AccountId) {
        self.assert_owner();
        self.processors.remove(&processor_id);
        log!("✓ Processor {} removed", processor_id);
    }

    pub fn set_daily_limit(&mut self, account_id: AccountId, limit: Balance) {
        self.assert_owner();
        self.daily_limits.insert(&account_id, &limit);
        log!("✓ Daily limit for {} set to {}", account_id, limit);
    }

    // ====== Payment Functions ======

    pub fn check_daily_limit(&self, account_id: &AccountId) -> bool {
        let limit = self.daily_limits.get(account_id).unwrap_or(u128::MAX);
        let today = env::block_timestamp() / (24 * 60 * 60 * 1_000_000_000);
        let spent = self.daily_spent.get(&(account_id.clone(), today)).unwrap_or(0);
        spent < limit
    }

    pub fn transfer_from_treasury(
        &mut self,
        token_contract: AccountId,
        recipient: AccountId,
        amount: Balance,
    ) -> Promise {
        self.assert_processor();

        // Check daily limit
        assert!(
            self.check_daily_limit(&env::signer_account_id()),
            "Daily limit exceeded"
        );

        // Update spent amount
        let today = env::block_timestamp() / (24 * 60 * 60 * 1_000_000_000);
        let mut spent = self.daily_spent
            .get(&(env::signer_account_id(), today))
            .unwrap_or(0);
        spent += amount;
        self.daily_spent.insert(
            &(env::signer_account_id(), today),
            &spent,
        );

        // Call fungible token contract to transfer
        ext_ft::ext(token_contract)
            .transfer(recipient, amount.into(), None)
    }

    // ====== View Functions ======

    pub fn get_balance(
        &self,
        token_contract: AccountId,
        account_id: AccountId,
    ) -> Balance {
        self.balances
            .get(&(token_contract, account_id))
            .unwrap_or(0)
    }

    pub fn get_daily_limit(&self, account_id: AccountId) -> Balance {
        self.daily_limits.get(&account_id).unwrap_or(u128::MAX)
    }

    pub fn get_daily_spent(&self, account_id: AccountId) -> Balance {
        let today = env::block_timestamp() / (24 * 60 * 60 * 1_000_000_000);
        self.daily_spent
            .get(&(account_id, today))
            .unwrap_or(0)
    }

    // ====== Internal Functions ======

    fn assert_owner(&self) {
        assert_eq!(
            env::signer_account_id(),
            self.owner_id,
            "Only owner can call this"
        );
    }

    fn assert_processor(&self) {
        assert!(
            self.processors
                .get(&env::signer_account_id())
                .is_some(),
            "Not authorized processor"
        );
    }
}

// External contract interface for fungible tokens
#[ext_contract(ext_ft)]
pub trait FungibleToken {
    fn transfer(&mut self, receiver_id: AccountId, amount: String, memo: Option<String>);
}
```

### 3. Create Token Contract (Optional)

```rust
// contracts/token/src/lib.rs

use near_sdk::near_bindgen;
use near_contract_standards::fungible_token::FungibleToken;
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::impl_fungible_token_core;
use near_contract_standards::impl_fungible_token_storage;

#[near_bindgen]
pub struct Contract {
    token: FungibleToken,
}

impl_fungible_token_core!(Contract, token);
impl_fungible_token_storage!(Contract, token);

#[near_bindgen]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        FungibleTokenMetadata {
            spec: FT_METADATA_SPEC.to_string(),
            name: "USDC".to_string(),
            symbol: "USDC".to_string(),
            icon: Some(
                "https://..."
                    .to_string(),
            ),
            reference: None,
            reference_hash: None,
            decimals: 6,
        }
    }
}
```

---

## Contract Deployment

### 1. Build Contract

```bash
cd contracts/treasury

# Build WebAssembly binary
cargo build --target wasm32-unknown-unknown --release

# Output: target/wasm32-unknown-unknown/release/treasury.wasm
# Size should be < 5MB

ls -lh target/wasm32-unknown-unknown/release/treasury.wasm
```

### 2. Deploy Contract

```bash
# Create subaccount for contract
TREASURY_ACCOUNT="treasury.yourname.testnet"
near account create-account $TREASURY_ACCOUNT \
  --using-account yourname.testnet

# Deploy contract code
near contract deploy-new $TREASURY_ACCOUNT \
  ./target/wasm32-unknown-unknown/release/treasury.wasm

# Verify deployment
near account view-account-summary $TREASURY_ACCOUNT

# Call initialize function
near contract call-function \
  as-transaction $TREASURY_ACCOUNT \
  new '{"owner_id": "yourname.testnet"}' \
  prepaid-gas '100000000000000' \
  attached-deposit '0'
```

### 3. Update Contract (After Changes)

```bash
# Rebuild
cargo build --target wasm32-unknown-unknown --release

# Deploy new version (replaces old code)
near contract deploy-new $TREASURY_ACCOUNT \
  ./target/wasm32-unknown-unknown/release/treasury.wasm \
  --from yourname.testnet

# Verify update
near contract view-code $TREASURY_ACCOUNT | head -20
```

---

## Account Management

### 1. Access Keys Management

```bash
#!/bin/bash
# Different key types for different purposes

# Full access key (for admin)
near keys add full-access-key \
  --account-id yourname.testnet

# Function-call key (limited access)
near keys add function-call-key \
  --account-id treasury.yourname.testnet \
  --access-key \
  --allowance 100000000000000 \
  --receiver-id treasury.yourname.testnet \
  --method-names transfer,add_processor

# View all keys
near account list-keys
```

### 2. Delegate Key Management

```bash
# Share a limited-access key with a processor
# 1. Generate key
# 2. Add to contract as authorized
# 3. Share securely (never commit to repo)

near keys add processor-key \
  --account-id treasury.yourname.testnet
```

### 3. Storage Management

```bash
# NEAR contracts charge for storage
# Storage: 100,000 Yocto per byte

# Check storage usage
near contract get-storage $TREASURY_ACCOUNT | jq '.storage_usage'

# Reserve storage by sending NEAR
near tokens send $TREASURY_ACCOUNT '10' \
  --from yourname.testnet
```

---

## Function Calls & State Updates

### 1. Call Write Functions (Change State)

```bash
#!/bin/bash
# Functions that modify state cost gas

# Add processor
near contract call-function \
  as-transaction $TREASURY_ACCOUNT \
  add_processor \
  '{"processor_id": "processor.yourname.testnet"}' \
  prepaid-gas '100000000000000' \
  attached-deposit '0' \
  --from yourname.testnet

# Set daily limit
near contract call-function \
  as-transaction $TREASURY_ACCOUNT \
  set_daily_limit \
  '{"account_id": "alice.testnet", "limit": "1000000000"}' \
  prepaid-gas '100000000000000' \
  attached-deposit '0'
```

### 2. Call Read Functions (No Cost)

```bash
#!/bin/bash
# View functions are free and instant

# Check balance
near contract call-function \
  as-read-only $TREASURY_ACCOUNT \
  get_balance \
  '{"token_contract": "usdc.testnet", "account_id": "alice.testnet"}'

# Get daily limit
near contract call-function \
  as-read-only $TREASURY_ACCOUNT \
  get_daily_limit \
  '{"account_id": "alice.testnet"}'

# Get daily spent
near contract call-function \
  as-read-only $TREASURY_ACCOUNT \
  get_daily_spent \
  '{"account_id": "alice.testnet"}'
```

### 3. Gas Optimization

```rust
// Tips to minimize gas costs

// Use efficient serialization
use near_sdk::borsh;

// Minimize state writes
// ✓ Batch updates
// ✗ Individual updates

// Avoid loops in write functions
// ✓ Use iterator
// ✗ Manual loop

// Optimize data structures
// ✓ LookupMap (O(1) lookup)
// ✗ Vec (O(n) search)

// Estimate gas before deployment
let gas_burned = env::used_gas();
log!("Gas used: {}", gas_burned);
```

---

## Cross-Contract Calls

### 1. Call Fungible Token Contract

```rust
// In your Treasury contract

#[ext_contract(ext_ft)]
pub trait FungibleToken {
    fn ft_transfer(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
    );

    fn ft_balance_of(
        &self,
        account_id: AccountId,
    ) -> U128;
}

pub fn transfer_token(
    &mut self,
    token_contract: AccountId,
    recipient: AccountId,
    amount: U128,
) -> Promise {
    ext_ft::ext(token_contract)
        .with_static_gas(Gas::ONE_TERA * 10)  // 10 TGas
        .ft_transfer(recipient, amount, None)
}

pub fn check_token_balance(
    &self,
    token_contract: AccountId,
    account: AccountId,
) -> Promise {
    ext_ft::ext(token_contract)
        .with_static_gas(Gas::ONE_TERA * 5)
        .ft_balance_of(account)
}
```

### 2. Handle Callback Results

```rust
#[private]
pub fn handle_transfer_callback(
    &mut self,
    #[callback_result]
    result: Result<(), PromiseError>,
) -> bool {
    match result {
        Ok(_) => {
            log!("✓ Transfer successful");
            true
        }
        Err(_) => {
            log!("✗ Transfer failed");
            false
        }
    }
}
```

---

## Security Considerations

### 1. Contract Validation

```rust
pub fn assert_valid_account(&self, account: &AccountId) {
    assert!(
        account.len() >= 2 && account.len() <= 64,
        "Invalid account ID length"
    );
}

pub fn assert_valid_amount(&self, amount: Balance) {
    assert!(amount > 0, "Amount must be positive");
    assert!(amount < u128::MAX / 2, "Amount too large");
}

pub fn assert_not_paused(&self) {
    assert!(!self.paused, "Contract is paused");
}
```

### 2. Access Control

```rust
// Owner-only functions
fn assert_owner(&self) {
    assert_eq!(
        env::signer_account_id(),
        self.owner_id,
        "Only owner"
    );
}

// Role-based access
fn assert_processor(&self) {
    assert!(
        self.processors.get(&env::signer_account_id()).is_some(),
        "Not a processor"
    );
}

// Time-based access
fn assert_not_frozen(&self) {
    assert!(
        env::block_timestamp() >= self.unfreeze_time,
        "Account frozen"
    );
}
```

### 3. Reentrancy Protection

```rust
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Treasury {
    locked: bool,
    // ... other fields
}

pub fn transfer(&mut self) -> Promise {
    assert!(!self.locked, "Reentrancy detected");
    self.locked = true;

    let result = ext_ft::ext(token_contract)
        .ft_transfer(recipient, amount, None)
        .then(
            Self::ext(env::current_account_id())
                .with_static_gas(Gas::ONE_TERA * 5)
                .handle_transfer_callback()
        );

    self.locked = false;
    result
}
```

### 4. Input Validation

```rust
pub fn validate_transfer(
    amount: &Balance,
    recipient: &AccountId,
) -> Result<(), String> {
    if *amount == 0 {
        return Err("Amount cannot be zero".into());
    }

    if recipient == &env::signer_account_id() {
        return Err("Cannot transfer to self".into());
    }

    if recipient.len() > 64 {
        return Err("Invalid recipient".into());
    }

    Ok(())
}
```

---

## Testing Procedures

### 1. Unit Tests

```bash
cargo test --lib

# Test specific function
cargo test treasury::tests::test_add_processor -- --nocapture
```

### 2. Integration Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{
        get_context, VMContext,
    };

    fn get_context_with_account(account_id: &str) -> VMContext {
        let mut context = get_context(vec![]);
        context.signer_account_id = account_id.parse().unwrap();
        context.is_view = false;
        context
    }

    #[test]
    fn test_add_processor() {
        let context = get_context_with_account("owner.testnet");
        testing_env!(context);

        let mut contract = Treasury::new("owner.testnet".parse().unwrap());

        contract.add_processor("processor.testnet".parse().unwrap());

        // Verify processor was added
        assert!(contract.is_processor(&"processor.testnet".parse().unwrap()));
    }

    #[test]
    fn test_daily_limit() {
        let context = get_context_with_account("owner.testnet");
        testing_env!(context);

        let mut contract = Treasury::new("owner.testnet".parse().unwrap());

        contract.set_daily_limit(
            "alice.testnet".parse().unwrap(),
            1_000_000_000_000u128,
        );

        let limit = contract.get_daily_limit("alice.testnet".parse().unwrap());
        assert_eq!(limit, 1_000_000_000_000u128);
    }
}
```

### 3. Testnet Testing

```bash
# Deploy to testnet
near contract deploy-new treasury.testnet \
  ./target/wasm32-unknown-unknown/release/treasury.wasm

# Initialize
near contract call-function as-transaction treasury.testnet \
  new '{"owner_id": "yourname.testnet"}' \
  prepaid-gas '100000000000000' \
  attached-deposit '0'

# Add processor
near contract call-function as-transaction treasury.testnet \
  add_processor '{"processor_id": "test.testnet"}' \
  prepaid-gas '100000000000000' \
  attached-deposit '0'

# Verify
near contract call-function as-read-only treasury.testnet \
  is_processor '{"account_id": "test.testnet"}'
```

---

## Monitoring & Maintenance

### 1. Contract State Monitoring

```bash
#!/bin/bash
# Monitor contract storage and state

# View contract info
near contract get-storage $TREASURY_ACCOUNT

# View recent transactions
near transaction list-transactions $TREASURY_ACCOUNT | head -20

# Check account balance
near account view-account-summary $TREASURY_ACCOUNT
```

### 2. Health Check Function

```rust
pub fn health_check(&self) -> HealthStatus {
    HealthStatus {
        is_operational: !self.paused,
        total_processing_time_ms: 0,
        recent_errors: 0,
        contract_version: "0.1.0".to_string(),
    }
}

// Call periodically
near contract call-function as-read-only $TREASURY_ACCOUNT \
  health_check '{}'
```

### 3. Monitoring Metrics

Track these in your monitoring system:

```yaml
metrics:
  - contract_storage_bytes: Size of contract state
  - contract_execution_gas: Gas per function call
  - contract_errors_total: Error count
  - contract_function_calls: Call count per function
  - near_account_balance: Account NEAR balance
  - daily_spending_total: Total spent daily
  - processor_count: Number of authorized processors
```

### 4. Maintenance Tasks

**Weekly:**
- Check contract storage usage
- Review transaction logs for errors
- Monitor daily spending against limits

**Monthly:**
- Update dependencies
- Review access keys for unused ones
- Audit processor list

**Quarterly:**
- Full security audit
- Load testing with high volume
- Update contract code if needed

---

## Deployment Checklist

Before going to production:

- [ ] NEAR CLI installed and configured
- [ ] Test account created and funded on testnet
- [ ] Smart contracts developed and tested locally
- [ ] Unit tests passing (100% coverage)
- [ ] Integration tests passing on testnet
- [ ] Contract deployment script created
- [ ] Access keys configured with least privilege
- [ ] Daily limits and spending controls configured
- [ ] Multi-signature setup for admin functions (if needed)
- [ ] Monitoring and alerting configured
- [ ] Disaster recovery plan created
- [ ] Team trained on operations
- [ ] Documentation updated
- [ ] Security audit completed

---

## Troubleshooting

### Contract Deployment Fails

```bash
# Check contract size
ls -lh target/wasm32-unknown-unknown/release/treasury.wasm

# Must be < 50MB
# If too large, optimize with:
# 1. cargo build --release --no-default-features
# 2. Use wasm-opt tool

# Check WASM validity
wasm-validate target/wasm32-unknown-unknown/release/treasury.wasm
```

### Insufficient Balance for Deployment

```bash
# Check balance
near account view-account-summary

# Need NEAR for storage: ~50KB = 0.5 NEAR + gas

# On testnet, request more
curl -X POST https://helper.testnet.near.org/account/$ACCOUNT/allowance
```

### Function Call Timeout

```bash
# Increase gas limit
near contract call-function as-transaction $CONTRACT \
  function_name '{}' \
  prepaid-gas '300000000000000' \
  attached-deposit '0'
```

### Access Denied

```bash
# Verify access key
near account list-keys

# Add access key if needed
near keys add key-name --account-id $ACCOUNT

# Or use signer from environment
export NEAR_SIGNER_ACCOUNT=$ACCOUNT
near contract call-function ...
```

---

## Additional Resources

- [NEAR Documentation](https://docs.near.org)
- [NEAR Rust SDK](https://docs.near.org/sdk/rust)
- [NEAR CLI Reference](https://github.com/near/near-cli-rs)
- [NEAR Contract Standards](https://github.com/near/near-sdk-rs/tree/master/near-contract-standards)
- [NEAR Explorer](https://explorer.mainnet.near.org)

---

**Last Updated**: 2024
**Maintenance**: Monthly review recommended
