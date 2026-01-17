# Smart Contract Execution Flow: From Quote to Token Delivery

## Overview: How Smart Contracts Fit Into Your System

Your architecture has **3 distinct stages:**

```
STAGE 1: QUOTING (Your Backend - No Smart Contract)
├─ User requests: "Swap 100 USDC (Solana) → XLM (Stellar)"
├─ Your backend queries Pyth: USDC/XLM = 0.015
├─ Quote created: 100 USDC → 6666 XLM
└─ Return to user (status: PENDING)

STAGE 2: PAYMENT (Funding Chain - No Smart Contract)
├─ User approves spending on Solana
├─ User transfers 100 USDC to your treasury
├─ Your webhook confirms receipt
└─ Quote status: PENDING → COMMITTED

STAGE 3: EXECUTION (Execution Chain - Smart Contract Required!)
├─ Your backend calls Stellar Smart Contract
├─ Smart Contract executes swap via DEX
├─ User receives tokens automatically
└─ Quote status: COMMITTED → EXECUTED
```

---

## Why Smart Contracts Are Essential for Stage 3

### Without Smart Contract ❌

```
Your Backend                    Stellar DEX
    │                              │
    ├─ Transfer USDC to DEX ────→  │
    │                              │
    │  (Waiting...)                │
    │                              │
    │ ← Get XLM back ─────────────┤
    │                              │
    ├─ Transfer XLM to User ────→  User Wallet
    │                              │
    Problem: Race condition!
    If backend crashes between steps, user loses funds!
```

### With Smart Contract ✅

```
Your Backend                Stellar Smart Contract       DEX
    │                              │                      │
    ├─ Call swap() ─────────────→  │                      │
    │                              │                      │
    │                              ├─ Transfer USDC ────→ │
    │                              │                      │
    │                              │ ← Get XLM ────────── │
    │                              │                      │
    │                              ├─ Transfer XLM ────→ User
    │                              │                      │
    │ ← Return success ──────────  │
    │                              │
    All-or-nothing atomicity!
    If DEX fails, entire contract reverts.
    No partial states.
```

---

## Complete Execution Flow: Stellar Example

### Step-by-Step with Code

#### 1. User Initiates Request (Your Backend)

```rust
// POST /quote
// User sends: { funding_chain: "Solana", execution_chain: "Stellar", ... }

let quote = quote_engine.generate_quote(
    user_id,
    Chain::Solana,
    Chain::Stellar,
    "USDC",
    "XLM",
    instructions,
    estimated_compute_units,
).await?;

// Quote created with status: PENDING
// Return to user
```

**Result:**
- Quote ID: `550e8400-e29b-41d4-a716-446655440000`
- Status: `PENDING`
- Expires: `300 seconds`

---

#### 2. User Sends Payment (Solana - User's Wallet)

**User Action:**
```
1. Open Phantom wallet
2. Approve spending: 100 USDC
3. Send 100 USDC to treasury address
```

**Smart Contract Interaction:** ❌ None - Just a transfer!

```rust
// Solana transfer instruction (not your code - user's wallet does this)
// System Program creates:
// Transfer {
//   from: user_wallet,
//   to: treasury_address,
//   amount: 100 USDC,
// }
```

---

#### 3. Webhook Confirms Payment (Your Backend)

```rust
// POST /webhook/solana (called by Solana blockchain)
pub async fn solana_webhook(
    State(state): State<AppState>,
    Json(payload): Json<SolanaWebhookPayload>,
) -> AppResult<()> {
    info!("💰 Solana payment received: {}", payload.transaction_hash);
    
    // Find quote by nonce
    let quote = state.ledger.get_quote_by_nonce(&payload.memo).await?;
    
    // Verify amount
    if payload.amount < quote.max_funding_amount {
        return Err(QuoteError::InsufficientAmount.into());
    }
    
    // Update quote status: PENDING → COMMITTED
    let mut tx = state.ledger.begin_tx().await?;
    state.ledger
        .update_quote_status(
            &mut tx,
            quote.id,
            QuoteStatus::Pending,
            QuoteStatus::Committed,
        )
        .await?;
    tx.commit().await?;

    info!("✅ Quote {} committed, ready for execution", quote.id);
    
    Ok(())
}
```

**Result:**
- Quote Status: `COMMITTED`
- Treasury: +100 USDC
- Ready for execution on Stellar

---

#### 4. Your Backend Calls Smart Contract (Stellar)

```rust
// In src/execution/stellar.rs::execute()

pub async fn execute(&self, quote: Quote) -> AppResult<Execution> {
    info!("🚀 Starting Stellar execution for quote {}", quote.id);

    // Step 1: Verify quote is ready
    if !quote.can_execute() {
        return Err(QuoteError::InvalidState.into());
    }

    // Step 2: Calculate output amount with slippage protection
    let min_output = quote.max_funding_amount * Decimal::from(0.999); // 0.1% slippage

    // Step 3: Call Stellar Smart Contract
    // This is the critical part!
    
    let swap_result = self
        .call_stellar_swap_contract(
            user_address: &quote.user_id.to_string(),
            treasury_address: env::var("STELLAR_TREASURY_ADDRESS")?,
            input_token: "USDC".to_string(),
            output_token: "XLM".to_string(),
            amount_in: quote.max_funding_amount,
            min_amount_out: min_output,
            dex_address: env::var("STELLAR_DEX_ADDRESS")?,
        )
        .await?;

    info!("✅ Swap executed: {} XLM transferred to user", swap_result.amount_out);

    // Step 4: Record execution in database
    let mut tx = self.ledger.begin_tx().await?;

    self.ledger
        .update_quote_status(
            &mut tx,
            quote.id,
            QuoteStatus::Committed,
            QuoteStatus::Executed,
        )
        .await?;

    let execution = self.ledger.create_execution(
        &mut tx,
        quote.id,
        Chain::Stellar,
        &swap_result.transaction_hash,
        swap_result.amount_out,
    ).await?;

    tx.commit().await?;

    info!("📝 Execution recorded: {:?}", execution);

    Ok(execution)
}

// Call the smart contract
async fn call_stellar_swap_contract(
    &self,
    user_address: &str,
    treasury_address: String,
    input_token: String,
    output_token: String,
    amount_in: Decimal,
    min_amount_out: Decimal,
    dex_address: String,
) -> AppResult<SwapResult> {
    // Build Soroban transaction to call smart contract
    
    let client = soroban_rpc::Client::new(STELLAR_HORIZON_URL);
    
    // 1. Load treasury account
    let treasury_account = client.get_account(&treasury_address).await?;
    
    // 2. Create transaction to call smart contract
    let tx_builder = TransactionBuilder::new()
        .set_base_fee(100)
        .set_timeout(300)
        .add_operation(
            // Invoke the smart contract
            soroban_rpc::Operation::InvokeContract {
                contract_id: SWAP_CONTRACT_ADDRESS.to_string(),
                function_name: "swap".to_string(),
                parameters: vec![
                    soroban_rpc::ContractParameter::Address(user_address.to_string()),
                    soroban_rpc::ContractParameter::Address(treasury_address.clone()),
                    soroban_rpc::ContractParameter::Address(USDC_ADDRESS.to_string()),
                    soroban_rpc::ContractParameter::Address(XLM_ADDRESS.to_string()),
                    soroban_rpc::ContractParameter::Int128(amount_in.to_i128()),
                    soroban_rpc::ContractParameter::Int128(min_amount_out.to_i128()),
                    soroban_rpc::ContractParameter::Address(dex_address),
                ],
            }
        )
        .build()?;
    
    // 3. Sign with treasury key
    let tx = tx_builder.sign_with(&treasury_signer, &NETWORK_PASSPHRASE)?;
    
    // 4. Submit to blockchain
    let result = client.submit_tx(&tx).await?;
    
    if !result.is_success {
        return Err(ExecutionError::TransactionFailed(result.error).into());
    }

    Ok(SwapResult {
        transaction_hash: result.hash,
        amount_out: Decimal::from(result.output),
    })
}
```

---

#### 5. Smart Contract Executes on Stellar (On-Chain)

```rust
// This runs ON STELLAR BLOCKCHAIN - Not on your backend!

#[contractimpl]
impl TokenSwapContract {
    pub fn swap(
        env: Env,
        user: Address,                    // User's wallet
        treasury: Address,                // Your treasury
        input_token: Address,             // USDC contract
        output_token: Address,            // XLM contract
        amount_in: i128,                  // 100 USDC
        min_amount_out: i128,             // 6600 XLM (with slippage)
        dex_address: Address,             // Ref Finance
    ) -> i128 {
        // --- ATOMIC EXECUTION ---
        // Either ALL these steps succeed, or ALL revert!
        
        // Step 1: Verify treasury authorized this call
        treasury.require_auth();
        println!("✅ Treasury authorized");

        // Step 2: Transfer USDC from treasury to this contract
        let token_in = TokenClient::new(&env, &input_token);
        token_in.transfer(
            &treasury,                           // From treasury
            &env.current_contract_address(),    // To this contract
            &amount_in,                         // 100 USDC
        );
        println!("✅ Transferred {} USDC to contract", amount_in);

        // Step 3: Approve DEX to spend the USDC
        token_in.approve(
            &env.current_contract_address(),
            &dex_address,
            &amount_in,
            &(env.ledger().sequence() + 1000),
        );
        println!("✅ Approved DEX to spend {} USDC", amount_in);

        // Step 4: Call DEX to swap USDC → XLM
        let amount_out: i128 = env.invoke_contract(
            &dex_address,
            &symbol_short!("swap"),
            &vec![
                &env,
                input_token.into_val(&env),
                output_token.into_val(&env),
                amount_in.into_val(&env),
            ],
        );
        println!("✅ DEX returned {} XLM", amount_out);

        // Step 5: Verify we got minimum output (slippage check)
        if amount_out < min_amount_out {
            env.panic_with_error(ContractError::SlippageExceeded);
            // Everything reverts here!
        }
        println!("✅ Slippage check passed: {} >= {}", amount_out, min_amount_out);

        // Step 6: Transfer XLM to user
        let token_out = TokenClient::new(&env, &output_token);
        
        // Calculate fee (0.1%)
        let fee = amount_out / 1000;
        let user_amount = amount_out - fee;

        token_out.transfer(
            &env.current_contract_address(),
            &user,
            &user_amount,
        );
        println!("✅ Transferred {} XLM to user", user_amount);

        // Step 7: Keep fee in contract (for treasury later)
        // (Contract keeps the fee_amount automatically)
        println!("✅ Kept {} XLM as fee", fee);

        // Done! Return amount sent to user
        user_amount
    }
}
```

**What Happens Here:**
1. ✅ Transfers 100 USDC from treasury to contract
2. ✅ Approves DEX to spend the USDC
3. ✅ Calls DEX (Ref Finance) to swap
4. ✅ Receives 6666 XLM back
5. ✅ Verifies minimum output (slippage protection)
6. ✅ Splits output: 6659.34 XLM to user, 6.66 XLM fee
7. ✅ Returns success

**Atomicity:** If ANY step fails, EVERYTHING reverts (no partial states)

---

#### 6. User Receives Tokens (Stella Network)

```
User's Stellar Wallet:
├─ Balance Before: 0 XLM
├─ Transaction Confirmed
├─ Balance After: +6659.34 XLM
├─ Gas Fee Paid: Included in output (0.1%)
└─ Status: ✅ RECEIVED
```

**User sees on blockchain explorer:**
```
Transaction: 550e8400e29b41d4a716446655440000
From: SmartContractAddress
To: UserAddress
Amount: 6659.34 XLM
Memo: "Swap from USDC (Solana) → XLM (Stellar)"
Status: ✅ Confirmed
```

---

#### 7. Your Backend Records Completion

```rust
// Quote status updated: COMMITTED → EXECUTED

Quote {
    id: 550e8400-e29b-41d4-a716-446655440000,
    user_id: user123,
    funding_chain: Solana,
    execution_chain: Stellar,
    funding_asset: "USDC",
    execution_asset: "XLM",
    max_funding_amount: 100,
    execution_cost: 6659.34,
    service_fee: 6.66,
    status: Executed,  // ← Changed from Committed
    created_at: 2026-01-03T10:00:00Z,
    executed_at: 2026-01-03T10:05:30Z,
}
```

---

## Key Points About Smart Contracts

### 1. Why They're Needed

| Component | Smart Contract Needed? | Reason |
|-----------|----------------------|--------|
| Quote generation | ❌ No | Just math (use Pyth API) |
| User payment | ❌ No | Simple transfer |
| **Token swap** | ✅ **YES** | Must be atomic & secure |
| Settlement | ❌ No | Your backend handles |
| Audit trail | ❌ No | Database sufficient |

### 2. What They Do

```
┌─────────────────────────────────────────┐
│  Smart Contract Responsibilities         │
├─────────────────────────────────────────┤
│ ✅ Transfer tokens from treasury        │
│ ✅ Call DEX for swap                    │
│ ✅ Validate output amount (slippage)    │
│ ✅ Send tokens to user                  │
│ ✅ Atomicity (all-or-nothing)          │
│ ✅ Revert on errors (no stuck funds)    │
└─────────────────────────────────────────┘
```

### 3. Atomicity Guarantee

```
Example: User sends 100 USDC, expects 6600 XLM

Scenario A: Success
├─ 100 USDC received ✅
├─ Swap executed ✅
├─ 6659.34 XLM sent to user ✅
└─ Result: User has XLM

Scenario B: Slippage exceeded
├─ 100 USDC received ✅
├─ DEX returns only 6500 XLM (not enough)
├─ Contract reverts ← EVERYTHING UNDONE
├─ 100 USDC returned to treasury ✅
└─ Result: No funds lost, try again

Scenario C: DEX fails
├─ 100 USDC received ✅
├─ DEX crashes/returns error
├─ Contract reverts ← EVERYTHING UNDONE
├─ 100 USDC returned to treasury ✅
└─ Result: No funds lost, retry possible
```

### 4. No Other Smart Contracts Needed

- ❌ Contract on Solana: Just transfer, no swap needed
- ❌ Contract for quoting: Pyth API handles pricing
- ❌ Contract for settlement: Backend handles DB updates
- ✅ Contracts on EXECUTION chains: For DEX swaps only

---

## Implementation Roadmap

```
Week 1: Setup & Testing
├─ [ ] Create Stellar contract
├─ [ ] Deploy to testnet
├─ [ ] Test manual contract calls
└─ [ ] Verify DEX integration

Week 2: Backend Integration
├─ [ ] Update execution/stellar.rs
├─ [ ] Add contract calling logic
├─ [ ] Create test transactions
└─ [ ] End-to-end test (Q → payment → swap → token)

Week 3: NEAR + Solana
├─ [ ] Create NEAR contract
├─ [ ] Deploy to testnet
├─ [ ] Create Solana program (or use Jupiter API)
└─ [ ] Multi-chain end-to-end test

Week 4: Production
├─ [ ] Audit all contracts
├─ [ ] Deploy to mainnet
├─ [ ] Go live!
└─ [ ] Monitor & alert
```

---

## Summary

**Smart contracts are the final piece of your cross-chain puzzle:**

1. **Quote Generation:** Pyth (no contract)
2. **Payment Collection:** User sends tokens (no contract)
3. **Token Swap:** Smart contract (required) ← **This is Stage 3**
4. **Token Delivery:** Smart contract sends to user (included in contract)
5. **Settlement:** Your backend records (no contract)

**Without smart contracts:** You'd have to manually manage swaps, risking fund loss if your backend crashes.

**With smart contracts:** Everything is atomic, transparent, and trustless on-chain!

