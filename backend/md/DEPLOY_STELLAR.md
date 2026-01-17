# Stellar Deployment Guide

Complete step-by-step guide for deploying crosschain payments backend on Stellar.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Network Setup](#network-setup)
3. [Account Configuration](#account-configuration)
4. [Issuing Assets](#issuing-assets)
5. [Federated Server Integration](#federated-server-integration)
6. [Callback URL Configuration](#callback-url-configuration)
7. [Security Considerations](#security-considerations)
8. [Testing Procedures](#testing-procedures)
9. [Monitoring & Maintenance](#monitoring--maintenance)

---

## Prerequisites

### Required Software

```bash
# Install Stellar CLI tools
# Option 1: Homebrew (macOS)
brew install stellar-core

# Option 2: Docker
docker pull stellar/stellar-core:latest

# Option 3: Build from source
git clone https://github.com/stellar/stellar-core.git
cd stellar-core && git checkout master
./autogen.sh && ./configure && make && make install

# Verify installation
stellar-core --version

# Install Rust toolchain
rustup default stable
cargo --version
```

### SDK Setup

```bash
# Stellar Rust SDK
cargo add stellar-sdk
cargo add stellar-strkey
cargo add sha2
cargo add ed25519-dalek

# For integration tests
cargo add stellar_base_node
```

### Environment Configuration

```bash
# Create .env.stellar
cat > .env.stellar << 'EOF'
# Network Selection
STELLAR_NETWORK=testnet  # testnet, public (mainnet)

# API Endpoints
STELLAR_HORIZON_URL=https://horizon-testnet.stellar.org
STELLAR_HORIZON_URL_PUBLIC=https://horizon.stellar.org

# Account Configuration
STELLAR_ISSUER_SECRET=SBXXX...  # Keep secure!
STELLAR_TREASURY_ACCOUNT=GBXXX...
STELLAR_DISTRIBUTION_ACCOUNT=GBXXX...

# Asset Configuration
STELLAR_USDC_CODE=USDC
STELLAR_USDC_ISSUER=GBBD47UZQ5DO5LVMUG46COGGBKHVL243XFXEA3E4NBVW7OSJBNDCXTQU

STELLAR_USDT_CODE=USDT
STELLAR_USDT_ISSUER=GBXVJAFAN4XLEPHUOREOUWHLFJAJGVHFXY3DXTHVYDCES6LAYVULTKM

# Federation
STELLAR_FEDERATION_URL=https://your-domain.com/.well-known/stellar.toml
STELLAR_FEDERATION_DOMAIN=your-domain.com

# Thresholds (signers required)
STELLAR_MASTER_WEIGHT=1
STELLAR_MEDIUM_THRESHOLD=2
STELLAR_HIGH_THRESHOLD=3

# Fee Configuration
STELLAR_BASE_FEE=100  # stroops (1/10,000,000 XLM)
EOF

source .env.stellar
```

---

## Network Setup

### 1. Create Stellar Accounts

```bash
#!/bin/bash
# Generate keypairs for different roles

# Master issuer account (stores the asset)
stellar-keygen --network testnet > issuer.json
ISSUER=$(cat issuer.json | jq -r '.public_key')

# Treasury account (holds issued assets)
stellar-keygen --network testnet > treasury.json
TREASURY=$(cat treasury.json | jq -r '.public_key')

# Distribution account (distributes to users)
stellar-keygen --network testnet > distribution.json
DISTRIBUTION=$(cat distribution.json | jq -r '.public_key')

# Cold storage (offline backup)
stellar-keygen --network testnet > cold.json

# Operational account (online operations)
stellar-keygen --network testnet > operations.json

echo "Issuer: $ISSUER"
echo "Treasury: $TREASURY"
echo "Distribution: $DISTRIBUTION"

# Save to environment
cat >> .env.stellar << EOF
STELLAR_ISSUER_PUBLIC=$ISSUER
STELLAR_TREASURY_PUBLIC=$TREASURY
STELLAR_DISTRIBUTION_PUBLIC=$DISTRIBUTION
EOF
```

### 2. Fund Accounts on Testnet

```bash
#!/bin/bash
# Get testnet XLM from friendbot (free testnet faucet)

ISSUER=$1
TREASURY=$2
DISTRIBUTION=$3

# Fund via Friendbot (testnet only)
curl "https://friendbot.stellar.org?addr=$ISSUER"
curl "https://friendbot.stellar.org?addr=$TREASURY"
curl "https://friendbot.stellar.org?addr=$DISTRIBUTION"

# Verify funding (should have 10,000 XLM each)
sleep 2
curl "https://horizon-testnet.stellar.org/accounts/$ISSUER"
```

### 3. Configure on Mainnet

For production (Mainnet):

```bash
#!/bin/bash
# Fund accounts via exchange or wallet
# Each account needs ~10 XLM for operations

# Create account on mainnet
# 1. Use Stellar web wallet: https://stellar.org/wallets
# 2. Transfer XLM from exchange
# 3. OR use hardware wallet (Ledger, Trezor)

# Verify funding
curl "https://horizon.stellar.org/accounts/$ISSUER"

# Expected response includes:
# - id: account public key
# - balances: array of asset balances
# - signers: list of authorized signers
# - thresholds: multisig configuration
```

---

## Account Configuration

### 1. Set Account Flags

```rust
// src/execution/stellar.rs

use stellar_sdk::{
    Account, TransactionBuilder, Asset, Operation,
    AccountFlags, Keypair,
};

pub async fn setup_issuer_account(
    horizon_url: &str,
    issuer_secret: &str,
) -> Result<String> {
    let keypair = Keypair::from_secret(issuer_secret)?;
    let account = get_account_details(horizon_url, &keypair.public_key()).await?;

    let mut tx_builder = TransactionBuilder::new(&account, horizon_url);

    // Revoke authorization
    // Prevents issuer from issuing new units after full supply issued
    let op = Operation::SetOptions {
        inflation_destination: None,
        set_flags: Some(AccountFlags::AuthRevocable as u32),
        clear_flags: Some(AccountFlags::AuthRequired as u32),
        master_weight: Some(1),
        low_threshold: Some(1),
        med_threshold: Some(2),
        high_threshold: Some(3),
        home_domain: Some("your-domain.com".to_string()),
        signer: None,
    };

    tx_builder.add_operation(op)?;
    let tx = tx_builder.build()?;
    let envelope = tx.into_transaction_envelope();
    
    let response = submit_transaction(horizon_url, &envelope).await?;
    Ok(response.id)
}
```

### 2. Add Signers (Multisig)

```rust
pub async fn add_signer(
    horizon_url: &str,
    account_secret: &str,
    signer_public: &str,
    weight: u32,
) -> Result<String> {
    let keypair = Keypair::from_secret(account_secret)?;
    let account = get_account_details(horizon_url, &keypair.public_key()).await?;

    let signer_keypair = Keypair::from_public_key(signer_public)?;
    let signer_key = stellar_strkey::SignedPayloadSigner {
        payload: signer_keypair.public_key().as_bytes().to_vec(),
        signer_weight: weight,
    };

    let mut tx_builder = TransactionBuilder::new(&account, horizon_url);
    
    let op = Operation::SetOptions {
        signer: Some((signer_key, weight)),
        ..Default::default()
    };

    tx_builder.add_operation(op)?;
    let tx = tx_builder.build()?;
    
    submit_transaction(horizon_url, &tx.into_transaction_envelope()).await
}
```

### 3. Set Up Trust Lines

```rust
pub async fn setup_trust_lines(
    horizon_url: &str,
    account_secret: &str,
    issuer_public: &str,
    asset_codes: &[&str],
) -> Result<Vec<String>> {
    let keypair = Keypair::from_secret(account_secret)?;
    let account = get_account_details(horizon_url, &keypair.public_key()).await?;

    let mut results = Vec::new();

    for asset_code in asset_codes {
        let asset = Asset::new(asset_code, issuer_public)?;
        
        let mut tx_builder = TransactionBuilder::new(&account, horizon_url);
        
        let op = Operation::ChangeTrust {
            asset,
            limit: Some("922337203685.4775807".to_string()),  // Max 64-bit amount
            ..Default::default()
        };

        tx_builder.add_operation(op)?;
        let tx = tx_builder.build()?;
        
        let response = submit_transaction(horizon_url, &tx.into_transaction_envelope()).await?;
        results.push(response.id);
    }

    Ok(results)
}
```

---

## Issuing Assets

### 1. Create Asset

```rust
pub async fn create_asset(
    horizon_url: &str,
    issuer_secret: &str,
    asset_code: &str,
    initial_supply: &str,
) -> Result<Asset> {
    let keypair = Keypair::from_secret(issuer_secret)?;
    let issuer_public = keypair.public_key();

    // Asset identified by code + issuer
    let asset = Asset::new(asset_code, &issuer_public)?;

    // Create trust line on distribution account first
    // Then issue to distribution account
    // Then distribute to users

    Ok(asset)
}
```

### 2. Payment Operations

```rust
pub async fn issue_to_distribution(
    horizon_url: &str,
    issuer_secret: &str,
    distribution_account: &str,
    asset_code: &str,
    amount: &str,
) -> Result<String> {
    let keypair = Keypair::from_secret(issuer_secret)?;
    let account = get_account_details(horizon_url, &keypair.public_key()).await?;

    let asset = Asset::new(asset_code, &keypair.public_key())?;

    let mut tx_builder = TransactionBuilder::new(&account, horizon_url);
    
    let op = Operation::Payment {
        destination: distribution_account.to_string(),
        asset,
        amount: amount.to_string(),
        ..Default::default()
    };

    tx_builder.add_operation(op)?;
    let tx = tx_builder.build()?;
    
    submit_transaction(horizon_url, &tx.into_transaction_envelope()).await
}

pub async fn send_payment(
    horizon_url: &str,
    from_secret: &str,
    to_account: &str,
    asset: Asset,
    amount: &str,
) -> Result<String> {
    let keypair = Keypair::from_secret(from_secret)?;
    let account = get_account_details(horizon_url, &keypair.public_key()).await?;

    let mut tx_builder = TransactionBuilder::new(&account, horizon_url);
    
    let op = Operation::Payment {
        destination: to_account.to_string(),
        asset,
        amount: amount.to_string(),
        ..Default::default()
    };

    tx_builder.add_operation(op)?;
    let tx = tx_builder.build()?;
    
    submit_transaction(horizon_url, &tx.into_transaction_envelope()).await
}
```

### 3. Path Payments (Cross-Asset)

For swaps between different assets:

```rust
pub async fn send_path_payment(
    horizon_url: &str,
    from_secret: &str,
    to_account: &str,
    send_asset: Asset,
    send_amount: &str,
    dest_asset: Asset,
    dest_amount: &str,
    path: Vec<Asset>,
) -> Result<String> {
    let keypair = Keypair::from_secret(from_secret)?;
    let account = get_account_details(horizon_url, &keypair.public_key()).await?;

    let mut tx_builder = TransactionBuilder::new(&account, horizon_url);
    
    let op = Operation::PathPayment {
        send_asset,
        send_max: send_amount.to_string(),
        destination: to_account.to_string(),
        dest_asset,
        dest_amount: dest_amount.to_string(),
        path: Some(path),
        ..Default::default()
    };

    tx_builder.add_operation(op)?;
    let tx = tx_builder.build()?;
    
    submit_transaction(horizon_url, &tx.into_transaction_envelope()).await
}
```

---

## Federated Server Integration

### 1. Implement Federation Endpoint

Federation enables user lookup by username:

```rust
// src/api/handler.rs - Add federation endpoint

use axum::{extract::Query, Json};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct FederationQuery {
    q: String,  // username@domain or account id
    #[serde(default)]
    type_: String,  // "name" or "id"
}

#[derive(Serialize)]
pub struct FederationResponse {
    stellar_address: String,
    account_id: String,
    memo_type: Option<String>,  // "text", "id", "hash"
    memo: Option<String>,
}

pub async fn federation_lookup(
    Query(query): Query<FederationQuery>,
) -> Result<Json<FederationResponse>> {
    // Parse input
    let (username, domain) = if query.type_ == "id" {
        // Account ID lookup
        validate_account_id(&query.q)?;
        return Ok(Json(FederationResponse {
            stellar_address: query.q.clone(),
            account_id: query.q,
            memo_type: None,
            memo: None,
        }));
    } else {
        // Username lookup
        let parts: Vec<&str> = query.q.split('@').collect();
        if parts.len() != 2 {
            return Err(AppError::BadRequest("Invalid federation address".into()));
        }
        (parts[0], parts[1])
    };

    // Verify domain matches
    let expected_domain = std::env::var("STELLAR_FEDERATION_DOMAIN")?;
    if domain != expected_domain {
        return Err(AppError::NotFound("Domain mismatch".into()));
    }

    // Lookup user in database
    let user = db_lookup_user_by_username(username).await?;

    Ok(Json(FederationResponse {
        stellar_address: format!("{}@{}", username, domain),
        account_id: user.stellar_account_id,
        memo_type: user.memo_type,
        memo: user.memo,
    }))
}
```

### 2. Create stellar.toml

```bash
# Create .well-known/stellar.toml (served on HTTPS)

cat > ./public/.well-known/stellar.toml << 'EOF'
FEDERATION_SERVER="https://your-domain.com/federation"
SIGNING_KEY="GBBD47UZQ5DO5LVMUG46COGGBKHVL243XFXEA3E4NBVW7OSJBNDCXTQU"
AUTH_SERVER="https://your-domain.com/auth"
TRANSFER_SERVER="https://your-domain.com/transfer"
TRANSFER_SERVER_SEP0024="https://your-domain.com/transfer/interactive"
WEB_AUTH_ENDPOINT="https://your-domain.com/auth"

# Supported assets
[[CURRENCIES]]
code="USDC"
issuer="GBBD47UZQ5DO5LVMUG46COGGBKHVL243XFXEA3E4NBVW7OSJBNDCXTQU"
name="USDC Stablecoin"
display_decimals=2
deposit_enabled=true
withdrawal_enabled=true

[[CURRENCIES]]
code="USDT"
issuer="GBXVJAFAN4XLEPHUOREOUWHLFJAJGVHFXY3DXTHVYDCES6LAYVULTKM"
name="USDT Stablecoin"
display_decimals=2
deposit_enabled=true
withdrawal_enabled=true

# Organization details
[DOCUMENTATION]
ORG_NAME="Your Organization"
ORG_URL="https://your-domain.com"
ORG_LOGO="https://your-domain.com/logo.png"
ORG_DESCRIPTION="Crosschain payment platform"
ORG_TWITTER="@yourhandle"
ORG_OFFICIAL_EMAIL="support@your-domain.com"
ORG_SUPPORT_EMAIL="support@your-domain.com"
ORG_SUPPORT_PHONE="+1-800-..."

# Validators (for consensus)
[[VALIDATORS]]
ALIAS="validator1"
DISPLAY_NAME="Validator 1"
PUBLIC_KEY="GBXXX..."
HISTORY="https://your-domain.com/stellar-history/validator1"
EOF
```

### 3. Serve stellar.toml

```bash
# Add to server configuration (nginx/Apache)
# Ensure HTTPS and CORS headers set correctly

# Nginx example:
location /.well-known/stellar.toml {
    add_header 'Access-Control-Allow-Origin' '*';
    add_header 'Access-Control-Allow-Methods' 'GET, OPTIONS';
    add_header 'Content-Type' 'text/plain';
}
```

---

## Callback URL Configuration

### 1. Transaction Callback Handler

```rust
// src/api/handler.rs - Stellar webhook

use axum::extract::{State, Json};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct StellarTransactionCallback {
    tx_id: String,
    tx_hash: String,
    ledger: u32,
    created_at: String,
    from: String,
    to: String,
    asset_code: String,
    amount: String,
    memo: Option<String>,
    memo_type: Option<String>,
}

pub async fn stellar_transaction_callback(
    State(state): State<AppState>,
    Json(callback): Json<StellarTransactionCallback>,
) -> Result<StatusCode> {
    // Verify webhook signature (if implemented)
    // verify_stellar_webhook_signature(&callback)?;

    // Log transaction
    tracing::info!(
        "Stellar transaction callback: {} -> {} {} {}",
        callback.from,
        callback.to,
        callback.amount,
        callback.asset_code,
    );

    // Update order status in database
    let quote = state
        .ledger
        .get_quote_by_memo(&callback.memo.unwrap_or_default())
        .await?;

    state
        .ledger
        .update_quote_status(
            &quote.id,
            QuoteStatus::Executed,
            Some(&callback.tx_hash),
        )
        .await?;

    // Notify user (webhook, email, etc)
    // state.notification_queue.send(...).await?;

    Ok(StatusCode::OK)
}
```

### 2. Configure Webhook in Horizon

```bash
# Subscribe to Horizon events via streaming

curl "https://horizon-testnet.stellar.org/transactions?cursor=now" \
  --header "Content-Type: application/json" \
  -X GET

# For production, use EventSource/SSE library
# Or poll periodically for new transactions
```

---

## Security Considerations

### 1. Key Management

```bash
# NEVER store secrets in .env or version control
# Use environment variables or secret manager

# AWS Secrets Manager
aws secretsmanager create-secret \
  --name stellar/issuer-key \
  --secret-string "SBXXX..."

# HashiCorp Vault
vault kv put secret/stellar issuer_key="SBXXX..."

# Load in code:
let issuer_secret = std::env::var("STELLAR_ISSUER_SECRET")
    .expect("STELLAR_ISSUER_SECRET not set");
```

### 2. Signature Verification

```rust
use ed25519_dalek::{PublicKey, Signature, VerifyingKey};
use sha2::{Sha256, Digest};

pub fn verify_stellar_signature(
    public_key: &str,
    message: &[u8],
    signature: &str,
) -> Result<()> {
    // Decode public key from base32
    let public_bytes = stellar_strkey::decode_check(
        stellar_strkey::VersionByte::PublicKeyTypeEd25519,
        public_key
    )?;

    let verifying_key = VerifyingKey::from_bytes(&public_bytes)?;

    // Decode signature from base64
    let sig_bytes = base64_decode(signature)?;
    let sig = Signature::from_slice(&sig_bytes)?;

    // Verify signature
    verifying_key.verify(message, &sig)?;
    Ok(())
}
```

### 3. Transaction Validation

```rust
pub fn validate_stellar_transaction(
    tx: &StellarTransaction,
) -> Result<()> {
    // Check network
    if !tx.network_id.ends_with("Testnet") 
        && !tx.network_id.ends_with("Public") {
        return Err("Invalid network".into());
    }

    // Check fee
    let max_fee = 10_000_000;  // 1 XLM
    if tx.fee > max_fee {
        return Err("Fee exceeds maximum".into());
    }

    // Check timeout
    if tx.timeout_seconds.is_some() && tx.timeout_seconds > Some(3600) {
        return Err("Timeout too long".into());
    }

    // Check memo for duplicates (prevent replay)
    if let Some(memo) = &tx.memo {
        if db_check_memo_exists(memo).await? {
            return Err("Memo already used".into());
        }
    }

    Ok(())
}
```

### 4. Rate Limiting

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    accounts: HashMap<String, Vec<Instant>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn is_allowed(&mut self, account: &str) -> bool {
        let now = Instant::now();
        let cutoff = now - self.window;

        let times = self.accounts.entry(account.to_string()).or_insert_with(Vec::new);
        times.retain(|&t| t > cutoff);

        if times.len() < self.max_requests {
            times.push(now);
            true
        } else {
            false
        }
    }
}
```

---

## Testing Procedures

### 1. Unit Tests

```bash
cargo test --features stellar
cargo test stellar_executor -- --nocapture
```

### 2. Integration Tests - Testnet

```rust
#[tokio::test]
async fn test_issue_asset_testnet() {
    let horizon_url = "https://horizon-testnet.stellar.org";
    let issuer_secret = "SBXXX...";  // Testnet keypair
    
    let result = create_asset(
        horizon_url,
        issuer_secret,
        "USDC",
        "1000000.00",
    ).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_path_payment() {
    let result = send_path_payment(
        horizon_url,
        payer_secret,
        recipient_account,
        send_asset,
        "100",
        dest_asset,
        "100",
        vec![],
    ).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_federation_lookup() {
    let response = federation_lookup(
        Query(FederationQuery {
            q: "alice@your-domain.com".to_string(),
            type_: "name".to_string(),
        }),
    ).await;

    assert!(response.is_ok());
    let fed = response.unwrap().0;
    assert!(fed.account_id.starts_with("G"));
}
```

### 3. Manual Testing with Stellar Lab

```bash
# Use Stellar Lab for interactive testing
# https://laboratory.stellar.org

# 1. Create transaction
# 2. Sign with testnet keypair
# 3. Submit and verify
# 4. Check on Stellar Expert:
#    https://stellar.expert/explorer/testnet/tx/...
```

### 4. Load Testing

```bash
# Simulate multiple transactions
wrk -t12 -c400 -d30s http://localhost:8080/quote \
  -s load_test.lua
```

---

## Monitoring & Maintenance

### 1. Health Check Endpoint

```rust
pub async fn stellar_health(
    State(state): State<AppState>,
) -> Json<HealthResponse> {
    let mut healthy = true;
    let mut details = Vec::new();

    // Check Horizon connectivity
    match check_horizon_connection().await {
        Ok(_) => details.push("Horizon: OK".to_string()),
        Err(e) => {
            healthy = false;
            details.push(format!("Horizon: ERROR - {}", e));
        }
    }

    // Check issuer account balance
    match get_stellar_account(issuer_public).await {
        Ok(account) => {
            let native_balance = account
                .balances
                .iter()
                .find(|b| b.asset_type == "native")
                .map(|b| b.balance.parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);
            
            if native_balance < 1.0 {
                healthy = false;
                details.push(format!("Issuer balance low: {}", native_balance));
            } else {
                details.push(format!("Issuer balance: {} XLM", native_balance));
            }
        }
        Err(e) => {
            healthy = false;
            details.push(format!("Account check: ERROR - {}", e));
        }
    }

    Json(HealthResponse {
        status: if healthy { "healthy" } else { "unhealthy" },
        details,
    })
}
```

### 2. Monitoring Metrics

```rust
// Track these metrics in Prometheus/Datadog:

- stellar_transactions_total (counter)
- stellar_transaction_duration_ms (histogram)
- stellar_transaction_fee_stroops (gauge)
- stellar_account_balance (gauge)
- stellar_ledger_height (gauge)
- stellar_api_errors_total (counter)
- stellar_federation_lookups_total (counter)
```

### 3. Alert Rules

```yaml
alerts:
  - name: stellar_issuer_balance_low
    threshold: 1.0  # Less than 1 XLM
    action: alert_ops
  
  - name: stellar_transaction_failure_rate
    threshold: 0.05  # > 5% failures
    action: investigate
  
  - name: horizon_api_latency_high
    threshold: 5000  # > 5 seconds
    action: failover
  
  - name: invalid_transactions
    threshold: 10  # > 10 per minute
    action: investigate
```

### 4. Maintenance Tasks

**Weekly:**
- Check issuer account balance
- Review transaction logs for errors
- Monitor Horizon API status

**Monthly:**
- Update Stellar SDK versions
- Review and rotate if needed (keys)
- Check for new SEP standards
- Run full disaster recovery test

**Quarterly:**
- Security audit
- Load testing with increased volume
- Review federation configuration

---

## Deployment Checklist

Before going to production:

- [ ] Stellar CLI installed
- [ ] Keypairs generated and secured
- [ ] Testnet accounts funded and tested
- [ ] Assets created and distributed
- [ ] Federation server implemented
- [ ] stellar.toml deployed on HTTPS
- [ ] Webhook callbacks tested
- [ ] All unit tests passing
- [ ] Integration tests passing on testnet
- [ ] Load tests successful
- [ ] Security audit completed
- [ ] Monitoring/alerting configured
- [ ] Disaster recovery plan created
- [ ] Team trained on operations

---

## Troubleshooting

### Transaction Rejected: Bad Sequence Number
```bash
# Account sequence number is out of sync
# Solution: Fetch latest sequence from Horizon
curl https://horizon-testnet.stellar.org/accounts/GXXX | jq .sequence
```

### Insufficient Balance
```bash
# Check issuer account balance
curl https://horizon-testnet.stellar.org/accounts/GXXX | jq '.balances'

# On testnet, request more XLM
curl "https://friendbot.stellar.org?addr=GXXX"
```

### Federation Lookup Not Found
```bash
# Verify stellar.toml is accessible
curl https://your-domain.com/.well-known/stellar.toml

# Check federation endpoint
curl "https://your-domain.com/federation?q=user@domain&type=name"
```

### Asset Issuance Not Working
```bash
# 1. Verify issuer has funded account
curl https://horizon-testnet.stellar.org/accounts/$ISSUER

# 2. Verify distribution account has trust line
curl https://horizon-testnet.stellar.org/accounts/$DISTRIBUTION | jq '.balances'

# 3. Try manual issuance
stellar-sdk payment \
  --from $ISSUER \
  --to $DISTRIBUTION \
  --amount 1000 \
  --asset USDC:$ISSUER
```

---

## Additional Resources

- [Stellar Documentation](https://developers.stellar.org)
- [Stellar Expert](https://stellar.expert)
- [Stellar Laboratory](https://laboratory.stellar.org)
- [Horizon API Reference](https://developers.stellar.org/api/introduction/index.html)
- [SEP Standards](https://github.com/stellar/stellar-protocol/tree/master/core/cap-0001.md)

---

**Last Updated**: 2024
**Maintenance**: Monthly review recommended
