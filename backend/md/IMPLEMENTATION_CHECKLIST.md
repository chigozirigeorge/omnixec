# Quick Implementation Checklist: Pyth + Smart Contracts

## Phase 1: Backend Pyth Integration (This Week)

### Step 1: Add Pyth Module
Create `src/quote_engine/pyth_oracle.rs` with the code from the guide.

### Step 2: Update Cargo.toml
```toml
[dependencies]
pyth-sdk-solana = "0.8"
reqwest = { version = "0.11", features = ["json"] }
```

### Step 3: Add to quote_engine/mod.rs
```rust
pub mod pyth_oracle;
```

### Step 4: Update main.rs
```rust
// After creating ledger
let network = std::env::var("NETWORK").unwrap_or_else(|_| "testnet".to_string());
let pyth_oracle = Arc::new(quote_engine::pyth_oracle::PythOracle::new(&network));
info!("✅ Pyth oracle initialized for {}", network);

// Pass to quote engine
let quote_engine = Arc::new(quote_engine::QuoteEngine::new(
    quote_config,
    ledger.clone(),
    pyth_oracle.clone(),
));
```

### Step 5: Update QuoteEngine struct
In `src/quote_engine/engine.rs`:
```rust
use crate::quote_engine::pyth_oracle::PythOracle;

pub struct QuoteEngine {
    config: QuoteConfig,
    ledger: Arc<LedgerRepository>,
    pyth_oracle: Arc<PythOracle>,  // Add this field
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
}
```

### Step 6: Test Pyth (Testnet)
```bash
# Add to .env
NETWORK=testnet

# Run
cargo run
# Should see: ✅ Pyth oracle initialized for testnet
```

---

## Phase 2: Smart Contracts (Next 2 Weeks)

### For Stellar (Priority #1)

**Setup:**
```bash
cd contracts && mkdir stellar_swap && cd stellar_swap

# Create Soroban contract
cargo init --lib

# Add to Cargo.toml
[dependencies]
soroban-sdk = "21"

[lib]
crate-type = ["cdylib"]
```

**Contract Code:** Use the code from PYTH_INTEGRATION_AND_CONTRACTS.md

**Deploy:**
```bash
soroban contract build
soroban contract deploy \
  --network testnet \
  --source treasury \
  --wasm target/wasm32-unknown-unknown/release/stellar_swap.wasm
```

### For NEAR (Priority #2)

**Setup:**
```bash
cd contracts && mkdir near_swap && cd near_swap

# Create NEAR contract
cargo init --lib

# Add to Cargo.toml
[dependencies]
near-sdk = "4.1"
serde = { version = "1", features = ["derive"] }

[profile.release]
opt-level = "z"
lto = true
```

**Contract Code:** Use the code from PYTH_INTEGRATION_AND_CONTRACTS.md

**Deploy:**
```bash
cargo build --target wasm32-unknown-unknown --release
near dev-deploy --wasmFile target/wasm32-unknown-unknown/release/near_swap.wasm \
  --accountId treasury.testnet
```

### For Solana (Priority #3 - Optional)

**Alternative:** Use Jupiter Aggregator
```bash
# No smart contract needed!
# Just call REST API: https://api.jup.ag/swap
```

---

## Integration Points in Your Code

### 1. Update get_quote to use Pyth prices

**File:** `src/quote_engine/engine.rs`

```rust
pub async fn generate_quote(...) -> AppResult<Quote> {
    // Existing validation...

    // NEW: Get real-time price from Pyth
    let price_data = self
        .pyth_oracle
        .get_price(&funding_asset, "USDC")
        .await
        .map_err(|e| QuoteError::PriceUnavailable(e.to_string()))?;

    info!(
        "Pyth price: {} {} = {} USDC",
        price_data.rate, funding_asset, price_data.rate
    );

    // Use price_data.rate for calculations
    // ...existing code...
}
```

### 2. Call Smart Contract on Execution

**File:** `src/execution/stellar.rs` (update execute method)

```rust
// Pseudo-code - implement based on your needs
pub async fn execute(&self, quote: Quote) -> AppResult<Execution> {
    // ... existing validation ...

    // Call Stellar smart contract
    let contract_result = soroban_invoke(
        "swap_contract_address",
        "swap",
        vec![
            user_address,
            input_token,
            output_token,
            quote.max_funding_amount,
            min_amount_out,  // With slippage
        ],
    ).await?;

    // Mark quote as executed
    self.ledger.update_quote_status(
        &mut tx,
        quote.id,
        QuoteStatus::Committed,
        QuoteStatus::Executed
    ).await?;

    Ok(execution)
}
```

### 3. Add Error Variant

**File:** `src/error.rs`

```rust
#[error("Price unavailable: {0}")]
PriceUnavailable(String),

#[error("Smart contract call failed: {0}")]
ContractExecutionFailed(String),
```

---

## Testing Checklist

### Pyth Tests
- [ ] Connect to testnet Pyth API
- [ ] Fetch SOL/USDC price
- [ ] Verify price freshness
- [ ] Test confidence calculation
- [ ] Handle stale price error

### Stellar Contract Tests
- [ ] Deploy to testnet
- [ ] Verify contract can transfer tokens
- [ ] Test swap with Ref Finance
- [ ] Confirm tokens reach user wallet
- [ ] Validate slippage protection

### NEAR Contract Tests
- [ ] Deploy to testnet
- [ ] Test token transfer from treasury
- [ ] Verify DEX swap execution
- [ ] Check gas fee calculation
- [ ] Confirm user receives tokens

### Integration Tests
- [ ] Create quote (testnet)
- [ ] User sends payment (testnet)
- [ ] Backend calls smart contract (testnet)
- [ ] User receives tokens on destination chain

---

## Environment Variables to Add

```bash
# Pyth
NETWORK=testnet
PYTH_MAINNET_URL=https://hermes.pyth.network
PYTH_TESTNET_URL=https://hermes-beta.pyth.network

# Stellar Smart Contract
STELLAR_SWAP_CONTRACT_ID=CxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxS
STELLAR_TREASURY_ADDRESS=GxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxS

# NEAR Smart Contract
NEAR_SWAP_CONTRACT_ID=treasury.testnet
NEAR_NETWORK=testnet

# Solana (if using Jupiter)
JUPITER_API_URL=https://quote-api.jup.ag
```

---

## Timeline

| Phase | Week | Tasks |
|-------|------|-------|
| 1 | This | Add Pyth module, integrate into quote engine, test on testnet |
| 2 | Next | Deploy Stellar contract, test swaps, integrate with backend |
| 3 | Week 3 | Deploy NEAR contract, test swaps, integrate with backend |
| 4 | Week 4 | End-to-end testing: payment → swap → token delivery |
| 5 | Week 5 | Audit & security review before mainnet |

---

## Quick Start: Run with Pyth Today

1. Create `src/quote_engine/pyth_oracle.rs` ✅
2. Update `Cargo.toml` ✅
3. Update `QuoteEngine` struct ✅
4. Update `main.rs` ✅
5. Add `.env` variable: `NETWORK=testnet`
6. Run: `cargo run`
7. Expected output: `✅ Pyth oracle initialized for testnet`

That's it! Pyth integration ready to use in `quote_engine.pyth_oracle`.

---

## Do I need Smart Contracts?

**YES - On Execution Chain Only**

Why:
- ✅ Atomic swaps (no slippage, no stuck funds)
- ✅ User receives tokens directly
- ✅ Gas paid from output (your fee covers it)
- ✅ Transparent on-chain audit trail

What you DON'T need:
- ❌ Contract on funding chain (just transfer tokens)
- ❌ Contract for quoting (Pyth API is enough)
- ❌ Custom DEX (use existing DEXes)

---

This is your complete roadmap to go live! Start with Pyth this week, then add contracts progressively.
