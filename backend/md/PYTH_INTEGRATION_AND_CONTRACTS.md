# Pyth Integration & Smart Contract Strategy for Cross-Chain Purchases

## Part 1: Pyth Price Feed Integration

### Why Pyth?
- **Multi-chain:** Works on Solana, Stellar (via Soroban), and Near (via NEAR Aurora)
- **Low-latency:** Sub-second price updates
- **Security:** Cryptographically signed by validators
- **Fallback:** Multiple data providers prevent single-point failure
- **Real-time:** No block delay like on-chain oracles

### Architecture Overview

```
User Request
    ↓
Your Backend (Rust)
    ├── Query Pyth REST API (real-time)
    ├── Validate price freshness
    ├── Calculate slippage bounds
    └── Generate Quote
        ↓
    Execution Chain
    ├── User sends payment on funding chain
    └── Your smart contract executes using verified price
```

---

## 1. Backend Integration (Rust)

### Step 1: Add Dependencies to Cargo.toml

```toml
[dependencies]
# ... existing deps ...
pyth-sdk-solana = "0.8"           # Solana price feeds
solana-program = "1.18"
reqwest = { version = "0.11", features = ["json"] }  # HTTP client
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Step 2: Create Pyth Price Oracle Module

**File:** `src/quote_engine/pyth_oracle.rs`

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};
use rust_decimal::Decimal;
use std::str::FromStr;

/// Pyth price feed identifiers for different chains/assets
pub struct PythPriceFeedIds {
    /// Solana mainnet price feed IDs
    pub solana_mainnet: HashMap<String, String>,
    /// Solana devnet price feed IDs
    pub solana_devnet: HashMap<String, String>,
}

impl PythPriceFeedIds {
    pub fn new(network: &str) -> Self {
        let mut solana_mainnet = HashMap::new();
        let mut solana_devnet = HashMap::new();

        // Mainnet Price Feed IDs (from Pyth dashboard)
        solana_mainnet.insert(
            "SOL".to_string(),
            "H6ARHf6YXhGrYZZAr3qLq9NNUyRfQAccJFvvrH8B8io2".to_string(),
        );
        solana_mainnet.insert(
            "USDC".to_string(),
            "Gnt27xtC473ZT2Mw5u8wZ68Z3gULkSTb5DuxJN2UYLw6".to_string(),
        );
        solana_mainnet.insert(
            "USDT".to_string(),
            "3Mnn2nBVDwZfWvkjvfYrg57MkeoM6P25TpzxSAzxnUk9".to_string(),
        );
        solana_mainnet.insert(
            "ETH".to_string(),
            "JF3hscKv78LspujqqVANcNhalysumGKPQmdarumdQLe".to_string(),
        );
        solana_mainnet.insert(
            "BTC".to_string(),
            "GVXRSv1FM36is6iNcumEjVc2U2iJ7Gx3bBrYb6Ax9rC2".to_string(),
        );

        // Devnet Price Feed IDs (use for testing)
        solana_devnet.insert(
            "SOL".to_string(),
            "J83w4HKfqxwcq3BEMMkPFSppjv3JW7MVvQ7gdNot2gS7".to_string(),
        );
        solana_devnet.insert(
            "USDC".to_string(),
            "5VAAA8Nm2kDMwkxwqHSEfQD87Lsmz8GPg6gsQkQr5S1j".to_string(),
        );

        Self {
            solana_mainnet,
            solana_devnet,
        }
    }

    pub fn get_feed_id(&self, asset: &str, network: &str) -> Option<String> {
        match network {
            "mainnet" => self.solana_mainnet.get(asset).cloned(),
            "devnet" | "testnet" => self.solana_devnet.get(asset).cloned(),
            _ => None,
        }
    }
}

/// Response from Pyth REST API
#[derive(Debug, Deserialize, Serialize)]
pub struct PythPriceResponse {
    pub id: String,
    pub price: PythPrice,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PythPrice {
    #[serde(rename = "price")]
    pub price: String,
    #[serde(rename = "conf")]
    pub confidence: String,
    #[serde(rename = "expo")]
    pub exponent: i32,
    #[serde(rename = "publish_time")]
    pub publish_time: i64,
}

impl PythPrice {
    /// Convert Pyth price to decimal (handle exponent)
    pub fn to_decimal(&self) -> Result<Decimal, Box<dyn std::error::Error>> {
        let price = Decimal::from_str(&self.price)?;
        let exponent = self.exponent;
        
        // If exponent is negative, divide by 10^|exponent|
        // If exponent is positive, multiply by 10^exponent
        let adjusted = if exponent < 0 {
            price / Decimal::from(10i64.pow((-exponent) as u32))
        } else {
            price * Decimal::from(10i64.pow(exponent as u32))
        };

        Ok(adjusted)
    }

    /// Get confidence interval as percentage
    pub fn confidence_pct(&self) -> Result<Decimal, Box<dyn std::error::Error>> {
        let conf = Decimal::from_str(&self.confidence)?;
        let price = Decimal::from_str(&self.price)?;
        
        // Confidence interval as percentage
        Ok((conf / price) * Decimal::from(100))
    }

    /// Check if price is fresh (less than 5 seconds old)
    pub fn is_fresh(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        (now - self.publish_time).abs() < 5 // 5 second freshness threshold
    }
}

/// Pyth Oracle Client
pub struct PythOracle {
    client: Client,
    base_url: String,
    network: String,
    feed_ids: PythPriceFeedIds,
}

impl PythOracle {
    pub fn new(network: &str) -> Self {
        let base_url = match network {
            "mainnet" => "https://hermes.pyth.network",
            _ => "https://hermes-beta.pyth.network", // Testnet/devnet
        };

        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            network: network.to_string(),
            feed_ids: PythPriceFeedIds::new(network),
        }
    }

    /// Get real-time price for asset pair
    pub async fn get_price(
        &self,
        base: &str,      // e.g., "SOL"
        quote: &str,     // e.g., "USDC"
    ) -> Result<PythPriceData, Box<dyn std::error::Error>> {
        // Get feed IDs
        let base_feed_id = self
            .feed_ids
            .get_feed_id(base, &self.network)
            .ok_or(format!("Unknown asset: {}", base))?;
        
        let quote_feed_id = self
            .feed_ids
            .get_feed_id(quote, &self.network)
            .ok_or(format!("Unknown asset: {}", quote))?;

        // Query both prices
        let base_price = self.fetch_price(&base_feed_id).await?;
        let quote_price = self.fetch_price(&quote_feed_id).await?;

        // Validate freshness
        if !base_price.is_fresh() || !quote_price.is_fresh() {
            warn!(
                "Stale price data for {}/{}: base age={}s, quote age={}s",
                base,
                quote,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
                    - base_price.publish_time,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
                    - quote_price.publish_time
            );
        }

        // Calculate exchange rate
        let base_decimal = base_price.to_decimal()?;
        let quote_decimal = quote_price.to_decimal()?;
        let rate = base_decimal / quote_decimal;

        Ok(PythPriceData {
            base: base.to_string(),
            quote: quote.to_string(),
            rate,
            base_price,
            quote_price,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn fetch_price(&self, feed_id: &str) -> Result<PythPrice, Box<dyn std::error::Error>> {
        let url = format!("{}/api/latest_price_feeds?ids={}", self.base_url, feed_id);

        let response = self.client.get(&url).send().await?;
        let body: serde_json::Value = response.json().await?;

        // Extract price from Pyth response
        if let Some(prices) = body.get("prices").and_then(|p| p.as_array()) {
            if let Some(first) = prices.first() {
                let price_obj = first
                    .get("price")
                    .ok_or("Missing price field")?
                    .clone();

                let price: PythPrice = serde_json::from_value(price_obj)?;
                return Ok(price);
            }
        }

        Err("No price data in response".into())
    }
}

#[derive(Debug, Clone)]
pub struct PythPriceData {
    pub base: String,
    pub quote: String,
    pub rate: Decimal,
    pub base_price: PythPrice,
    pub quote_price: PythPrice,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl PythPriceData {
    /// Calculate amount out with slippage protection
    pub fn calculate_output(
        &self,
        amount_in: Decimal,
        slippage_pct: Decimal,
    ) -> Decimal {
        let output = amount_in * self.rate;
        let slippage_amount = output * (slippage_pct / Decimal::from(100));
        output - slippage_amount
    }

    /// Get confidence bounds for output
    pub fn output_confidence_bounds(
        &self,
        amount_in: Decimal,
    ) -> Result<(Decimal, Decimal), Box<dyn std::error::Error>> {
        let base_conf_pct = self.base_price.confidence_pct()?;
        let quote_conf_pct = self.quote_price.confidence_pct()?;
        
        // Total confidence interval
        let total_conf_pct = base_conf_pct + quote_conf_pct;
        
        let output = amount_in * self.rate;
        let conf_amount = output * (total_conf_pct / Decimal::from(100));

        Ok((output - conf_amount, output + conf_amount))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_exponent_handling() {
        let price = PythPrice {
            price: "50000".to_string(),
            confidence: "100".to_string(),
            exponent: -8,
            publish_time: 1234567890,
        };

        // 50000 * 10^-8 = 0.0005
        let decimal = price.to_decimal().unwrap();
        assert_eq!(decimal, Decimal::from_str("0.5").unwrap());
    }
}
```

### Step 3: Integrate into Quote Engine

**File:** `src/quote_engine/engine.rs` (update)

```rust
// Add to imports
use crate::quote_engine::pyth_oracle::PythOracle;
use std::sync::Arc;

// Add to QuoteEngine struct
pub struct QuoteEngine {
    config: QuoteConfig,
    ledger: Arc<LedgerRepository>,
    pyth_oracle: Arc<PythOracle>,  // Add this
}

impl QuoteEngine {
    pub fn new(
        config: QuoteConfig,
        ledger: Arc<LedgerRepository>,
        pyth_oracle: Arc<PythOracle>,  // Add parameter
    ) -> Self {
        Self {
            config,
            ledger,
            pyth_oracle,
        }
    }

    pub async fn generate_quote(
        &self,
        user_id: Uuid,
        funding_chain: Chain,
        execution_chain: Chain,
        funding_asset: String,
        execution_asset: String,
        instructions: Vec<u8>,
        estimated_compute_units: Option<i32>,
    ) -> AppResult<Quote> {
        // ... existing validation ...

        // NEW: Get real-time price from Pyth
        let price_data = self
            .pyth_oracle
            .get_price(&funding_asset, "USDC")
            .await
            .map_err(|e| QuoteError::PriceUnavailable(e.to_string()))?;

        info!(
            "Real-time price: {} {} = {} USDC (confidence: {})",
            price_data.rate,
            funding_asset,
            price_data.rate,
            price_data.base_price.confidence_pct().unwrap_or_default()
        );

        // Calculate execution cost based on real price
        let execution_cost_in_funding_asset = self
            .estimate_execution_cost(execution_chain, estimated_compute_units)
            .await?
            / price_data.rate;  // Convert to funding chain asset

        // ... rest of quote generation ...

        Ok(quote)
    }
}
```

### Step 4: Update main.rs to Initialize Pyth

```rust
// In src/main.rs
use crate::quote_engine::pyth_oracle::PythOracle;

// ... in main() ...

// Initialize Pyth Oracle
let network = std::env::var("NETWORK").unwrap_or_else(|_| "testnet".to_string());
let pyth_oracle = Arc::new(PythOracle::new(&network));
info!("✅ Pyth price oracle initialized for {}", network);

// Pass to quote engine
let quote_engine = Arc::new(quote_engine::QuoteEngine::new(
    quote_config,
    ledger.clone(),
    pyth_oracle.clone(),  // Add this
));
```

### Step 5: Add Error Variant

**File:** `src/error.rs`

```rust
#[error("Price unavailable: {0}")]
PriceUnavailable(String),
```

---

## Part 2: Smart Contract Strategy for Token Purchases

### Do You Need a Smart Contract?

**Short Answer:** YES - But only on the EXECUTION chain (where tokens are bought)

**Why:**
- Users send tokens/SOL on funding chain → your backend
- Your backend must then **swap those tokens on the execution chain**
- Swaps MUST happen atomically to prevent fund loss
- You need a smart contract to interact with DEXes

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ FUNDING CHAIN (e.g., Solana)                                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  User Wallet                    Your Treasury               │
│  ├─ SOL: 5 SOL    ──[transfer]──>  ├─ SOL: 100 SOL        │
│  └─ USDC: 100     ──[transfer]──>  └─ USDC: 10000         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                         ↓ Your Backend
┌─────────────────────────────────────────────────────────────┐
│ EXECUTION CHAIN (e.g., Stellar)                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Your Smart Contract ────[calls Dex]──> DEX (e.g., Ref)   │
│  ├─ Receives: 100 USDC from Treasury                       │
│  ├─ Swaps: 100 USDC → XLM (via Dex)                        │
│  ├─ Pays: Gas fee (0.1 XLM)                                │
│  └─ Sends: XLM to User's Wallet                            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Smart Contracts Needed

### 1. Solana - Treasury Swap Program (Optional - Can Use SPL Swap)

**When:** If building for Solana

**What it does:**
- Receives USDC/SOL
- Swaps via Raydium/Orca DEX
- Sends output token to user wallet
- Pays gas fee from output

**Alternative:** Use existing DEX aggregators (1inch, Jupiter)

### 2. Stellar - Token Swap Contract (Required)

**Language:** Soroban (Rust)

**File:** `contracts/stellar_swap.rs`

```rust
#![no_std]
use soroban_sdk::{
    contract, contractimpl, symbol_short, vec, Address, Env, 
    Symbol, Vec, FromVal, IntoVal, TryFromVal, String,
};
use soroban_token_sdk::TokenClient;

#[contract]
pub struct TokenSwapContract;

#[contractimpl]
impl TokenSwapContract {
    /// Execute a token swap on Stellar DEX
    /// 
    /// # Arguments
    /// * `env` - Soroban environment
    /// * `user` - User's wallet address
    /// * `treasury` - Treasury address that holds tokens
    /// * `input_token` - Token address to send from treasury
    /// * `output_token` - Token address to receive
    /// * `amount_in` - Amount of input token to swap
    /// * `min_amount_out` - Minimum amount of output token (slippage protection)
    /// * `dex_address` - DEX contract address (e.g., Ref Finance)
    pub fn swap(
        env: Env,
        user: Address,
        treasury: Address,
        input_token: Address,
        output_token: Address,
        amount_in: i128,
        min_amount_out: i128,
        dex_address: Address,
    ) -> i128 {
        // Verify treasury authorization
        treasury.require_auth();

        // 1. Transfer input tokens from treasury to this contract
        let token_in = TokenClient::new(&env, &input_token);
        token_in.transfer(
            &treasury,
            &env.current_contract_address(),
            &amount_in,
        );

        // 2. Approve DEX to spend tokens
        token_in.approve(
            &env.current_contract_address(),
            &dex_address,
            &amount_in,
            &(env.ledger().sequence() + 1000), // Expiration ledger
        );

        // 3. Call DEX swap function
        // This depends on which DEX you're using (Ref Finance, etc.)
        let amount_out: i128 = env
            .invoke_contract(
                &dex_address,
                &symbol_short!("swap"),
                vec![
                    &env,
                    input_token.into_val(&env),
                    output_token.into_val(&env),
                    amount_in.into_val(&env),
                ],
            );

        // 4. Verify minimum output amount (slippage check)
        if amount_out < min_amount_out {
            panic!("Slippage exceeded: got {}, expected min {}", amount_out, min_amount_out);
        }

        // 5. Transfer output tokens to user
        let token_out = TokenClient::new(&env, &output_token);
        let gas_fee = amount_out / 1000; // 0.1% gas fee
        let user_amount = amount_out - gas_fee;

        token_out.transfer(
            &env.current_contract_address(),
            &user,
            &user_amount,
        );

        // 6. Keep gas fee in contract for treasury
        user_amount
    }

    /// Get quote for swap (view function)
    pub fn get_swap_quote(
        env: Env,
        input_token: Address,
        output_token: Address,
        amount_in: i128,
        dex_address: Address,
    ) -> i128 {
        env
            .invoke_contract(
                &dex_address,
                &symbol_short!("getquote"),
                vec![
                    &env,
                    input_token.into_val(&env),
                    output_token.into_val(&env),
                    amount_in.into_val(&env),
                ],
            )
    }
}
```

### 3. NEAR - Token Swap Contract (Required)

**Language:** Rust with NEAR SDK

**File:** `contracts/near_swap/src/lib.rs`

```rust
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LazyOption;
use near_sdk::{
    env, log, near_bindgen, AccountId, Balance, PanicOnDefault, 
    Promise, PromiseOrValue,
};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct TokenSwapContract {
    treasury: AccountId,
    dex_contract: AccountId,
}

#[near_bindgen]
impl TokenSwapContract {
    #[init]
    pub fn new(treasury: AccountId, dex_contract: AccountId) -> Self {
        Self {
            treasury,
            dex_contract,
        }
    }

    /// Execute token swap on NEAR DEX
    pub fn swap(
        &mut self,
        user: AccountId,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: Balance,
        min_amount_out: Balance,
    ) -> Promise {
        // 1. Verify treasury authorization
        assert_eq!(
            env::predecessor_account_id(),
            self.treasury,
            "Only treasury can initiate swaps"
        );

        // 2. Transfer input token from treasury to this contract
        let contract_id = env::current_account_id();
        
        Promise::new(input_token.clone())
            .function_call(
                b"ft_transfer".to_vec(),
                serde_json::json!({
                    "receiver_id": contract_id,
                    "amount": amount_in.to_string(),
                })
                .to_string()
                .as_bytes()
                .to_vec(),
                1, // 1 yoctoNEAR
                env::prepaid_gas() / 4,
            )
            .then(
                Promise::new(self.dex_contract.clone())
                    .function_call(
                        b"swap".to_vec(),
                        serde_json::json!({
                            "input_token": input_token,
                            "output_token": output_token,
                            "amount_in": amount_in.to_string(),
                            "min_amount_out": min_amount_out.to_string(),
                        })
                        .to_string()
                        .as_bytes()
                        .to_vec(),
                        0,
                        env::prepaid_gas() / 2,
                    )
                    .then(
                        Promise::new(output_token)
                            .function_call(
                                b"ft_transfer".to_vec(),
                                serde_json::json!({
                                    "receiver_id": user,
                                    "amount": min_amount_out.to_string(),
                                })
                                .to_string()
                                .as_bytes()
                                .to_vec(),
                                1,
                                env::prepaid_gas() / 4,
                            )
                    )
            )
    }

    /// Get swap quote from DEX
    pub fn get_swap_quote(
        &self,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: Balance,
    ) -> Promise {
        Promise::new(self.dex_contract.clone())
            .function_call(
                b"get_quote".to_vec(),
                serde_json::json!({
                    "input_token": input_token,
                    "output_token": output_token,
                    "amount_in": amount_in.to_string(),
                })
                .to_string()
                .as_bytes()
                .to_vec(),
                0,
                env::prepaid_gas() / 2,
            )
    }
}
```

---

## Implementation Flow: User Payment → Token Purchase

### Complete Testnet Flow

```
1. USER INITIATES ON FUNDING CHAIN (Solana Testnet)
   POST /quote {
     "funding_chain": "Solana",
     "execution_chain": "Stellar",
     "funding_asset": "USDC",
     "execution_asset": "XLM",
     "amount": 100
   }
   ↓
2. YOUR BACKEND (Rust)
   ├─ Get Pyth price: USDC/XLM = 0.015
   ├─ Calculate gas: 2 stroops = $0.00001
   ├─ Create Quote (status: PENDING)
   └─ Return: {
        "quote_id": "uuid",
        "payment_address": "Gc...",
        "amount_to_send": 100 USDC,
        "expected_output": 6666 XLM,
        "expires_in": 300 seconds
      }
   ↓
3. USER APPROVES SPENDING (On Solana)
   ├─ Signs transaction: "Approve 100 USDC spending"
   ├─ Wallet broadcasts to Solana
   └─ Your backend listens for webhook
   ↓
4. USER SENDS PAYMENT (On Solana)
   ├─ Sends 100 USDC to your treasury address
   ├─ Your webhook listener confirms
   └─ Quote status: PENDING → COMMITTED
   ↓
5. YOUR BACKEND EXECUTES (On Stellar)
   ├─ Retrieve Quote (status: COMMITTED)
   ├─ Call Smart Contract: swap(
        user_address: "G...",
        input_token: USDC_contract,
        output_token: XLM_native,
        amount_in: 100,
        min_amount_out: 6600,  // With 0.1% slippage
        dex: Ref_Finance
      )
   ├─ Smart Contract:
   │  ├─ Transfers 100 USDC from treasury
   │  ├─ Calls Ref Finance DEX
   │  ├─ Receives 6666 XLM
   │  ├─ Keeps 6.66 XLM as fee
   │  └─ Sends 6659.34 XLM to user wallet
   ├─ Poll for Stellar confirmation
   └─ Quote status: COMMITTED → EXECUTED
   ↓
6. USER RECEIVES TOKENS
   └─ Stellar wallet shows: +6659.34 XLM
```

---

## Testnet Deployment Checklist

### Solana Testnet

```bash
# 1. Deploy to Solana devnet
solana program deploy --url devnet target/deploy/your_program.so

# 2. Fund treasury address
solana airdrop 1 <treasury_address> --url devnet

# 3. Create USDC token mint
spl-token create-token --url devnet

# 4. Update .env
SOLANA_RPC_URL=https://api.devnet.solana.com
SOLANA_NETWORK=devnet
SOLANA_TREASURY_KEY=<from solana-keygen>
```

### Stellar Testnet

```bash
# 1. Build Soroban contract
soroban contract build

# 2. Deploy to testnet
soroban contract deploy \
  --network testnet \
  --source treasury \
  --wasm target/wasm32-unknown-unknown/release/stellar_swap.wasm

# 3. Update .env
STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
STELLAR_HORIZON_URL=https://horizon-testnet.stellar.org
STELLAR_TREASURY_KEY=<from stellar-keygen>
```

### NEAR Testnet

```bash
# 1. Build contract
cd contracts/near_swap && cargo build --target wasm32-unknown-unknown

# 2. Deploy
near deploy --wasmFile target/wasm32-unknown-unknown/release/near_swap.wasm \
  --accountId treasury.testnet \
  --networkId testnet

# 3. Update .env
NEAR_RPC_URL=https://rpc.testnet.near.org
NEAR_NETWORK=testnet
NEAR_ACCOUNT_ID=treasury.testnet
```

---

## Summary: Pyth + Smart Contracts

| Component | Needed? | Language | Purpose |
|-----------|---------|----------|---------|
| Pyth Oracle Integration | ✅ YES | Rust | Real-time prices for quotes |
| Solana Smart Contract | ⚠️ Optional* | Rust/Anchor | Swap on Solana |
| Stellar Smart Contract | ✅ YES | Rust/Soroban | Swap on Stellar |
| NEAR Smart Contract | ✅ YES | Rust/NEAR-SDK | Swap on NEAR |
| Backend Integration | ✅ YES | Rust | Call smart contracts |

**\* Solana:** Can use Jupiter Aggregator API instead of building your own

---

## Production Considerations

1. **Price Staleness:** Reject quotes if price >5 seconds old
2. **Slippage:** Hard limit 0.5%, adjustable by user
3. **Gas Estimation:** Account for chain congestion
4. **Fallback Pricing:** Use Chainlink as fallback if Pyth unavailable
5. **Contract Audits:** Get smart contracts audited before mainnet
6. **Rate Limiting:** Limit quote generation to prevent abuse
7. **Monitoring:** Alert if price feeds go offline

This gives you a complete production-ready solution!
