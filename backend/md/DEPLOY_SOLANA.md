# Solana Deployment Guide

Complete step-by-step guide for deploying crosschain payments backend on Solana.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Infrastructure Setup](#infrastructure-setup)
3. [Treasury Account Configuration](#treasury-account-configuration)
4. [Token Program Integration](#token-program-integration)
5. [RPC Configuration](#rpc-configuration)
6. [Security Considerations](#security-considerations)
7. [Testing Procedures](#testing-procedures)
8. [Monitoring & Maintenance](#monitoring--maintenance)

---

## Prerequisites

### Required Software

```bash
# Install Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/v1.17.0/install)"

# Verify installation
solana --version
# Expected: solana-cli 1.17.0

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Create Solana wallet for deployment
solana-keygen new --outfile ~/solana-deployment-keypair.json
```

### Environment Variables

```bash
# Create .env file for Solana deployment
cat > .env.solana << 'EOF'
# RPC Configuration
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
SOLANA_RPC_URL_DEVNET=https://api.devnet.solana.com
SOLANA_RPC_URL_TESTNET=https://api.testnet.solana.com

# Network (mainnet-beta, devnet, testnet)
SOLANA_NETWORK=devnet

# Wallet Configuration
SOLANA_KEYPAIR_PATH=~/.config/solana/id.json
SOLANA_DEPLOYMENT_KEYPAIR=./solana-deployment-keypair.json

# Token Mint Addresses (create these after setup)
USDC_MINT=EPjFWaJy47gIdZiohMzvoi52LjxjxJmMu3pfFLsprA7  # Devnet
USDT_MINT=Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenEqw  # Devnet
SOL_MINT=So11111111111111111111111111111111111111112    # Native SOL

# Program IDs (after deployment)
STOKEN_PROGRAM_ID=TokenkegQfeZyiNwAJsyFbPMwotPtiqQy5w1aSKgS6  # SPL Token Program
TOKEN_2022_PROGRAM_ID=TokenzQdBbjWhAw8ns6mfPC3p4CPysxFYdpa4Yb22  # Token 2022 Program

# Treasury Account Configuration
TREASURY_OWNER=<your-deployment-keypair-pubkey>
TREASURY_SEED=crosschain_treasury_v1
EOF

source .env.solana
```

### Project Dependencies

Ensure your `Cargo.toml` includes:

```toml
[dependencies]
solana-client = "1.17.0"
solana-sdk = "1.17.0"
solana-program = "1.17.0"
spl-token = "0.21.0"
spl-associated-token-account = "0.21.0"
spl-token-2022 = "0.10.0"
anchor-lang = "0.29.0"
anchor-spl = "0.29.0"
solana-account-decoder = "1.17.0"
tokio = { version = "1", features = ["full"] }
reqwest = "0.11"
```

---

## Infrastructure Setup

### 1. RPC Node Configuration

Choose a provider:

```bash
# Option A: Public RPC (free, rate-limited)
export SOLANA_RPC_URL=https://api.devnet.solana.com

# Option B: Helius (recommended for production)
export SOLANA_RPC_URL=https://devnet.helius-rpc.com/?api-key=YOUR_KEY

# Option C: QuickNode
export SOLANA_RPC_URL=https://solana-devnet.quiknode.pro/...

# Option D: Magic Eden
export SOLANA_RPC_URL=https://mainnet.magiceden.io

# Test RPC connectivity
curl https://api.devnet.solana.com -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}'
```

### 2. Set Default Configuration

```bash
# Configure Solana CLI to use devnet (recommended for testing)
solana config set --url devnet
solana config set --keypair ~/solana-deployment-keypair.json

# Verify configuration
solana config get
# Output:
# Config File: ~/.config/solana/cli/config.yml
# RPC URL: https://api.devnet.solana.com
# WebSocket URL: wss://api.devnet.solana.com/ (computed)
# Keypair Path: /home/user/solana-deployment-keypair.json
# Commitment: confirmed
```

### 3. Fund Deployment Account

**For Devnet:**

```bash
# Get your public key
solana address

# Airdrop SOL (devnet only - free)
solana airdrop 10

# Check balance
solana balance
# Expected: 10.00000000 SOL
```

**For Mainnet:**

```bash
# Purchase SOL from exchange and transfer to:
solana address
# Send at least 1 SOL to cover transaction fees
```

---

## Treasury Account Configuration

### 1. Create Treasury PDA (Program Derived Address)

```bash
# Treasury accounts use PDA for security (no private key needed)

# Create in Rust code (executed by your backend):
use solana_sdk::{
    pubkey::Pubkey,
    account::Account,
};
use spl_associated_token_account::get_associated_token_address;

pub async fn create_treasury_account(
    payer: &Pubkey,
    mint: &Pubkey,
) -> Result<Pubkey, Box<dyn std::error::Error>> {
    // Associated Token Account (ATA) - standard pattern
    let treasury_token_account = get_associated_token_address(payer, mint);
    
    println!("Treasury Token Account: {}", treasury_token_account);
    Ok(treasury_token_account)
}
```

### 2. Initialize Treasury Accounts for Each Token

```bash
#!/bin/bash
# Create treasury accounts for USDC, USDT, SOL

# Get deployment keypair pubkey
DEPLOYER=$(solana-keygen pubkey ./solana-deployment-keypair.json)

# USDC Treasury
USDC_TREASURY=$(spl-token create-account EPjFWaJy47gIdZiohMzvoi52LjxjxJmMu3pfFLsprA7 \
  --owner "$DEPLOYER")

# USDT Treasury
USDT_TREASURY=$(spl-token create-account Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenEqw \
  --owner "$DEPLOYER")

# Store in environment
cat >> .env.solana << EOF
USDC_TREASURY=$USDC_TREASURY
USDT_TREASURY=$USDT_TREASURY
EOF

echo "Treasury accounts created successfully"
```

### 3. Verify Treasury Setup

```bash
# Check token account balances
spl-token accounts

# Check specific account
spl-token balance $USDC_TREASURY

# Check account details
solana account $USDC_TREASURY
```

### 4. Set Up Multisig Treasury (Production)

For production, use multisig to prevent single-point-of-failure:

```bash
#!/bin/bash
# Create 3-of-5 multisig for treasury approval

# Generate 5 signing keys
solana-keygen new --no-bip39-passphrase -o signer1.json
solana-keygen new --no-bip39-passphrase -o signer2.json
solana-keygen new --no-bip39-passphrase -o signer3.json
solana-keygen new --no-bip39-passphrase -o signer4.json
solana-keygen new --no-bip39-passphrase -o signer5.json

# Create multisig (requires spl-token-multisig CLI)
# 3 required signers out of 5 total
# This is complex - consider using Squads for UI
```

---

## Token Program Integration

### 1. Configure SPL Token Support

In `src/execution/solana.rs`:

```rust
use solana_client::rpc_client::RpcClient;
use spl_token::instruction::initialize_mint;
use spl_associated_token_account::instruction::create_associated_token_account;

pub struct SolanaExecutor {
    client: RpcClient,
    treasury_owner: Pubkey,
}

impl SolanaExecutor {
    pub fn new(rpc_url: &str, treasury_owner: &str) -> Self {
        Self {
            client: RpcClient::new(rpc_url.to_string()),
            treasury_owner: treasury_owner.parse().unwrap(),
        }
    }

    pub async fn get_token_balance(&self, token_account: &str) -> Result<u64> {
        let account = self.client.get_account(&token_account.parse()?)?;
        let parsed = spl_token::state::Account::unpack(&account.data)?;
        Ok(parsed.amount)
    }

    pub async fn transfer_token(
        &self,
        from_account: &str,
        to_account: &str,
        amount: u64,
        payer_keypair: &Keypair,
    ) -> Result<String> {
        let instruction = spl_token::instruction::transfer(
            &spl_token::id(),
            &from_account.parse()?,
            &to_account.parse()?,
            &self.treasury_owner,
            &[&self.treasury_owner],
            amount,
        )?;

        let recent_blockhash = self.client.get_latest_blockhash()?;
        let mut tx = Transaction::new_with_payer(
            &[instruction],
            Some(&payer_keypair.pubkey()),
        );
        tx.sign(&[payer_keypair], recent_blockhash);

        let sig = self.client.send_and_confirm_transaction(&tx)?;
        Ok(sig.to_string())
    }
}
```

### 2. Handle Wrapped Tokens

For tokens not natively on Solana (bridged via Wormhole):

```rust
// Configuration
const WORMHOLE_BRIDGE_ADDRESS: &str = "WormholeQaA835AY4FwNLGVQFVn...";

pub struct BridgedTokenConfig {
    pub chain_id: u16,  // 1=Solana, 2=Ethereum, etc
    pub mint: Pubkey,
    pub decimals: u8,
}

impl BridgedTokenConfig {
    pub fn usdc_from_ethereum() -> Self {
        Self {
            chain_id: 2,
            mint: "EPjFWaJy47gIdZiohMzvoi52LjxjxJmMu3pfFLsprA7".parse().unwrap(),
            decimals: 6,
        }
    }
}
```

### 3. Token 2022 Support (Optional)

For newer token features:

```rust
pub async fn handle_token2022(
    &self,
    mint: &Pubkey,
) -> Result<TokenMetadata> {
    let account = self.client.get_account(mint)?;
    
    // Check if Token 2022
    if account.owner == spl_token_2022::id() {
        // Parse extended token metadata
        let metadata = spl_token_2022::extension::metadata_pointer::get_metadata(
            &self.client,
            mint,
        )?;
        Ok(metadata)
    } else {
        Err("Not a Token 2022 token".into())
    }
}
```

---

## RPC Configuration

### 1. Custom RPC Endpoint Setup

In `src/config.rs`:

```rust
#[derive(Debug, Clone)]
pub struct SolanaRpcConfig {
    pub url: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub commitment: String,  // "confirmed" or "finalized"
}

impl SolanaRpcConfig {
    pub fn devnet() -> Self {
        Self {
            url: "https://api.devnet.solana.com".to_string(),
            timeout_secs: 30,
            max_retries: 3,
            commitment: "confirmed".to_string(),
        }
    }

    pub fn mainnet() -> Self {
        Self {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            timeout_secs: 30,
            max_retries: 5,
            commitment: "finalized".to_string(),
        }
    }
}
```

### 2. Connection Pooling

```rust
pub struct SolanaConnectionPool {
    clients: Vec<RpcClient>,
    current: AtomicUsize,
}

impl SolanaConnectionPool {
    pub fn new(rpc_urls: &[&str]) -> Self {
        let clients = rpc_urls
            .iter()
            .map(|url| RpcClient::new(url.to_string()))
            .collect();

        Self {
            clients,
            current: AtomicUsize::new(0),
        }
    }

    pub fn get_client(&self) -> &RpcClient {
        let idx = self.current.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % self.clients.len();
        &self.clients[idx]
    }
}
```

### 3. Rate Limiting

```bash
# RPC providers rate limits:

# Public RPC:
# - Requests: 100/second
# - Concurrent connections: 40

# Helius (paid):
# - Requests: Depends on tier
# - Transactions/second: 10+

# QuickNode (paid):
# - Requests: Unlimited
# - Transactions/second: 100+

# Configure backoff in code:
const INITIAL_BACKOFF: u64 = 100;  // 100ms
const MAX_BACKOFF: u64 = 32000;    // 32 seconds
const BACKOFF_MULTIPLIER: u64 = 2;
```

---

## Security Considerations

### 1. Keypair Management

```bash
# NEVER commit keypairs to version control
echo "*.json" >> .gitignore
echo ".env*" >> .gitignore

# Store keypairs securely
# Option A: AWS Secrets Manager
aws secretsmanager create-secret \
  --name solana/deployment-key \
  --secret-string file://solana-deployment-keypair.json

# Option B: HashiCorp Vault
vault kv put secret/solana/keys deployment-key=@solana-deployment-keypair.json

# Option C: Google Secret Manager
gcloud secrets create solana-deployment-key \
  --data-file=solana-deployment-keypair.json
```

### 2. Transaction Signing Validation

```rust
use solana_sdk::signature::Signature;
use solana_sdk::transaction::Transaction;

pub fn validate_transaction_signature(
    tx: &Transaction,
    expected_signer: &Pubkey,
) -> Result<()> {
    // Verify signer is in the transaction
    if !tx.message.account_keys.contains(expected_signer) {
        return Err("Expected signer not found in transaction".into());
    }

    // Verify message hash
    let message_bytes = tx.message.serialize();
    ed25519_dalek::Verifier::verify_strict(
        &expected_signer,
        &message_bytes,
        &tx.signatures[0],
    )?;

    Ok(())
}
```

### 3. Account Validation

```rust
// Always verify account ownership
pub fn validate_token_account(
    account: &Account,
    expected_owner: &Pubkey,
) -> Result<()> {
    if account.owner != spl_token::id() {
        return Err("Account not owned by token program".into());
    }
    Ok(())
}

// Verify mint
pub fn validate_mint(
    account_data: &[u8],
    expected_mint: &Pubkey,
) -> Result<()> {
    let token_account = spl_token::state::Account::unpack(account_data)?;
    if token_account.mint != *expected_mint {
        return Err("Account mint mismatch".into());
    }
    Ok(())
}
```

### 4. Transaction Fee Limits

```rust
pub const MAX_TRANSACTION_FEE: u64 = 100_000;  // 0.0001 SOL

pub fn validate_transaction_fee(
    fee_payer_balance: u64,
    estimated_fee: u64,
) -> Result<()> {
    if estimated_fee > MAX_TRANSACTION_FEE {
        return Err(format!(
            "Estimated fee {} exceeds max {}",
            estimated_fee, MAX_TRANSACTION_FEE
        ).into());
    }

    if fee_payer_balance < estimated_fee {
        return Err("Insufficient balance for transaction fee".into());
    }

    Ok(())
}
```

---

## Testing Procedures

### 1. Unit Tests

```bash
# Run all tests
cargo test --features solana

# Run Solana-specific tests
cargo test solana_executor

# With output
cargo test solana_executor -- --nocapture
```

### 2. Integration Tests

Create `tests/solana_integration.rs`:

```rust
#[tokio::test]
async fn test_treasury_creation() {
    let config = SolanaRpcConfig::devnet();
    let executor = SolanaExecutor::new(&config.url, "treasury_owner_pubkey");
    
    let result = executor
        .create_token_account(
            &USDC_MINT,
            &executor.treasury_owner,
        )
        .await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_token_transfer() {
    let executor = SolanaExecutor::new("https://api.devnet.solana.com", "treasury");
    
    let tx_sig = executor
        .transfer_token(
            "source_account",
            "dest_account",
            1_000_000,
            &keypair,
        )
        .await
        .expect("Transfer should succeed");
    
    // Verify transaction
    let status = executor.verify_transaction(&tx_sig).await;
    assert_eq!(status, "Finalized");
}

#[tokio::test]
async fn test_token_balance() {
    let executor = SolanaExecutor::new("https://api.devnet.solana.com", "treasury");
    
    let balance = executor
        .get_token_balance("token_account")
        .await
        .expect("Should get balance");
    
    assert!(balance >= 0);
}
```

### 3. Load Testing

```bash
# Using Apache Bench
ab -n 1000 -c 50 http://localhost:8080/health

# Using wrk
wrk -t12 -c400 -d30s http://localhost:8080/health
```

### 4. Manual Testing with CLI

```bash
# Check treasury balance
spl-token balance $USDC_TREASURY

# Create test transfer
spl-token transfer EPjFWaJy47gIdZiohMzvoi52LjxjxJmMu3pfFLsprA7 \
  1000000 \
  <recipient_account> \
  --owner ~/solana-deployment-keypair.json

# Verify transaction
solana confirm <transaction_signature>

# Check account info
solana account <account_address>
```

---

## Monitoring & Maintenance

### 1. Health Checks

```rust
pub async fn health_check_solana(executor: &SolanaExecutor) -> HealthStatus {
    // Check RPC connectivity
    match executor.client.get_slot() {
        Ok(slot) => println!("✓ RPC connected, slot: {}", slot),
        Err(e) => return HealthStatus::Unhealthy(format!("RPC error: {}", e)),
    }

    // Check treasury accounts
    for (token, account) in [("USDC", usdc_account), ("USDT", usdt_account)] {
        match executor.client.get_account(&account.parse()?) {
            Ok(acc) => println!("✓ {} account healthy", token),
            Err(e) => return HealthStatus::Unhealthy(format!("{} error: {}", token, e)),
        }
    }

    HealthStatus::Healthy
}
```

### 2. Monitoring Dashboard Metrics

```rust
// Track in your monitoring system (Prometheus, Datadog, etc)

// Transaction metrics
- solana_transactions_total (counter)
- solana_transaction_duration_ms (histogram)
- solana_transaction_fee_lamports (gauge)

// Account metrics
- solana_treasury_balance_tokens (gauge)
- solana_token_accounts_count (gauge)

// RPC metrics
- solana_rpc_latency_ms (histogram)
- solana_rpc_errors_total (counter)
- solana_rpc_rate_limit_hit (counter)
```

### 3. Alert Thresholds

```yaml
alerts:
  - name: treasury_balance_low
    threshold: 100  # Less than 100 SOL
    action: alert_ops
  
  - name: rpc_latency_high
    threshold: 5000  # > 5 seconds
    action: failover_rpc
  
  - name: transaction_failure_rate
    threshold: 0.05  # > 5% failures
    action: alert_and_pause_transactions
  
  - name: rate_limit_exceeded
    threshold: 10  # > 10 times/hour
    action: implement_backoff
```

### 4. Maintenance Procedures

**Weekly:**
- Check treasury balances
- Review transaction logs for errors
- Monitor RPC provider status

**Monthly:**
- Update Solana CLI and dependencies
- Rotate keypairs (if possible)
- Review security logs

**Quarterly:**
- Full security audit
- Load testing
- Disaster recovery drill

---

## Deployment Checklist

Before going to production:

- [ ] Solana CLI installed and configured
- [ ] Deployment keypair generated and secured
- [ ] Treasury accounts created for all tokens
- [ ] RPC endpoint configured and tested
- [ ] Connection pool scaling verified
- [ ] All unit tests passing
- [ ] Integration tests passing on devnet
- [ ] Load tests successful
- [ ] Security audit completed
- [ ] Monitoring/alerting configured
- [ ] Documentation updated
- [ ] Team trained on operations
- [ ] Disaster recovery plan created

---

## Troubleshooting

### Transaction Timeouts
```bash
# Increase commitment level
solana config set --commitment finalized

# Check slot progress
solana slot
```

### Insufficient Balance
```bash
# Check balance
solana balance

# Airdrop (devnet only)
solana airdrop 10
```

### RPC Rate Limit
```bash
# Switch to different provider
export SOLANA_RPC_URL=https://mainnet.helius-rpc.com/?api-key=YOUR_KEY
solana config set --url $SOLANA_RPC_URL
```

### Token Account Issues
```bash
# List all token accounts
spl-token accounts

# Create missing account
spl-token create-account <MINT_ADDRESS>

# Close empty account
spl-token close <ACCOUNT_ADDRESS>
```

---

## Additional Resources

- [Solana Documentation](https://docs.solana.com)
- [SPL Token Documentation](https://spl.solana.com)
- [Anchor Framework](https://book.anchor-lang.com)
- [Solana Pay](https://solanaspay.com)
- [Metaplex Programs](https://www.metaplex.com)

---

**Last Updated**: 2024
**Maintenance**: Monthly review recommended
