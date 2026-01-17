use near_sdk::{
    AccountId, Gas, NearToken, PanicOnDefault, Promise, borsh::{self, BorshDeserialize, BorshSerialize}, env, log, near_bindgen, serde, state::ContractState, store::LookupMap
};
use near_sdk::serde_json::{json, Value};
use near_sdk::json_types::{U128, U64};


fn gas_from_tgas(tgas: U64) -> Gas {
    Gas::from_tgas(tgas)
}

const MAX_FEE_BPS: u32 = 500; // 5% maximum fee
const MIN_SLIPPAGE_BPS: u32 = 10; // 0.1% minimum slippage protection

/// Token Swap Contract for NEAR - SECURE VERSION
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct TokenSwapContract {
    /// Treasury account (holds tokens)
    pub treasury: AccountId,
    /// DEX contract (must be verified)
    pub dex_contract: LookupMap<U64, AccountId>,
    /// Fee percentage (basis points: 10 = 0.1%)
    pub fee_bps: u32,
    /// Admin account (can update settings)
    pub admin: AccountId,
    /// Is contract paused
    pub is_paused: bool,
    /// last finanlized Nonce to prevent replay attacks
    pub nonce: U64,
    /// in- flight execution nonces
    pub pending_nonce: Option<U64>,
    /// Whitelisted tokens
    pub whitelisted_tokens: LookupMap<AccountId, bool>,
    /// Count of whitelisted tokens
    pub whitelisted_tokens_count: U64,
    /// Swap events tracking
    pub swap_events: LookupMap<U64, SwapEvent>,
}


#[derive(BorshDeserialize, BorshSerialize, serde::Serialize, serde::Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SwapEvent {
    pub user_id: AccountId,
    pub input_token: AccountId,
    pub output_token: AccountId,
    pub amount_in: U128,
    pub amount_out: U128,
    pub fee_amount: U128,
    pub timestamp: U64,
    pub tx_hash: String,
}

#[near_bindgen]
impl TokenSwapContract {
    /// Initialize the swap contract with admin controls
    #[init]
   pub fn new() -> Self {
    Self {
        treasury: env::current_account_id(),
        dex_contract: LookupMap::new(b"d".to_vec()),
        admin: env::current_account_id(),
        fee_bps: 0,  // Will be updated later
        is_paused: false,
        nonce: 0,
        pending_nonce: None,
        whitelisted_tokens: LookupMap::new(b"w".to_vec()),
        whitelisted_tokens_count: 0,
        swap_events: LookupMap::new(b"e".to_vec()),
    }
}

#[payable]
pub fn initialize(&mut self, treasury: AccountId, dex_contract_id: AccountId, admin: AccountId, fee_bps: u32) {
    assert_eq!(env::predecessor_account_id(), env::current_account_id(), "Only contract can initialize");
    assert!(fee_bps <= MAX_FEE_BPS, "Fee too high");
    
    self.treasury = treasury;
    self.dex_contract.insert(0U64, dex_contract_id);
    self.admin = admin;
    self.fee_bps = fee_bps;
}
    /// Execute a token swap with comprehensive security checks
    /// 
    /// # Arguments
    /// * `user_id` - User account ID (receives output tokens)
    /// * `input_token` - Input token contract (must be whitelisted)
    /// * `output_token` - Output token contract (must be whitelisted)
    /// * `amount_in` - Amount of input token
    /// * `min_amount_out` - Minimum output amount (slippage protection)
    /// * `max_slippage_bps` - Maximum slippage in basis points
    /// * `signature_nonce` - Unique nonce to prevent replay
    #[payable]
    pub fn swap(
        &mut self,
        user_id: AccountId,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: U128,
        min_amount_out: U128,
        max_slippage_bps: u32,
        signature_nonce: U64,
    ) -> Promise {
        // SECURITY: Check contract not paused
        assert!(!self.is_paused, "Contract is paused");
        
        // SECURITY: Only treasury can initiate swaps
        assert_eq!(
            env::predecessor_account_id(),
            self.treasury,
            "Only treasury can initiate swaps"
        );
        
        // Enforce single in-flight execution
        assert!(
            self.pending_nonce.is_none(),
            "Another swap is already in progress"
        );
        
        // // SECURITY: Update nonce before any external calls
        let expected = self.nonce + 1;
        assert_eq!(
            signature_nonce.0,
            expected,
            "Invalid nonce"
        );

        // Lock nonce
        self.pending_nonce = Some(expected);
        
        // SECURITY: Validate tokens are whitelisted
        assert!(
            self.whitelisted_tokens.contains_key(&input_token),
            "Input token not whitelisted"
        );
        assert!(
            self.whitelisted_tokens.contains_key(&output_token),
            "Output token not whitelisted"
        );
        
        // SECURITY: Validate amounts
        let amount_in_U128: U128 = amount_in.into();
        let min_amount_out_U128: U128 = min_amount_out.into();
        assert!(amount_in_U128 > 0, "Amount must be positive");
        assert!(min_amount_out_U128 > 0, "Minimum output must be positive");
        
        // SECURITY: Validate slippage
        assert!(
            max_slippage_bps >= MIN_SLIPPAGE_BPS,
            "Slippage protection too low"
        );
        assert!(max_slippage_bps <= 1000, "Slippage too high"); // Max 10%
        
        let _contract_id = env::current_account_id();
        let tx_hash = env::sha256(&env::random_seed()).iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        
        log!("🔄 Swap #{}, TX: {}", expected, tx_hash);
        log!("From: {} -> To: {}", input_token, output_token);
        log!("Amount: {}, Min Out: {}", amount_in_U128, min_amount_out_U128);

        let dex_contract = self.dex_contract.get(&0U64).expect("No DEX contract configured").clone();
        // Get quote from DEX first to estimate output
        let quote_promise = Promise::new(dex_contract.clone())
            .function_call(
                "get_quote".to_string(),
                json!({
                    "input_token": input_token.clone(),
                    "output_token": output_token.clone(),
                    "amount_in": amount_in_U128.to_string(),
                })
                .to_string()
                .as_bytes()
                .to_vec(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(5),
            );
             
        // After getting quote, execute the swap
        quote_promise.then(
            // This callback receives the quote result
            Self::ext(env::current_account_id())
                .with_static_gas(gas_from_tgas(5))
                .execute_swap_after_quote(
                    user_id,
                    input_token,
                    output_token,
                    amount_in,
                    min_amount_out,
                    max_slippage_bps,
                    tx_hash,
                )
        )
    }
    
    /// Private callback: Executes swap after receiving quote
    #[private]
    pub fn execute_swap_after_quote(
        &mut self,
        user_id: AccountId,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: U128,
        min_amount_out: U128,
        max_slippage_bps: u32,
        tx_hash: String,
    ) -> Promise {
        // Parse the quote result from DEX
        let quote_result = match env::promise_result_checked(0, 100) {
            Ok(result) => result,
            Err(_) => {
                self.pending_nonce = None;
                panic!("Promise failed")
            },
        };
        let quote_str = String::from_utf8(quote_result).unwrap_or_default();
        
        log!("📊 Quote received: {}", quote_str);
        
        // Parse expected output from quote (simplified - adjust based on DEX response format)
        let expected_output: U128 = if quote_str.contains("amount_out") {
            // Parse JSON response
            let v: Value = serde_json::from_str(&quote_str).unwrap_or_default();
            v["amount_out"].as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0)
        } else {
            // Fallback: extract number from string
            quote_str.split(':')
                .last()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0)
        };
        
        let amount_in_U128: U128 = amount_in.into();
        let min_amount_out_U128: U128 = min_amount_out.into();
        
        // SECURITY: Validate quote meets minimum requirements
        assert!(
            expected_output >= min_amount_out_U128,
            "Quote insufficient: {} < {}",
            expected_output,
            min_amount_out_U128
        );
        
        // SECURITY: Apply dynamic slippage check
        let max_slippage_amount = expected_output
            .checked_mul(max_slippage_bps as U128)
            .unwrap()
            .checked_div(10_000)
            .unwrap();
        
        let min_acceptable_output = expected_output
            .checked_sub(max_slippage_amount)
            .unwrap();
        
        assert!(
            min_acceptable_output >= min_amount_out_U128,
            "Slippage check failed: {} < {}",
            min_acceptable_output,
            min_amount_out_U128
        );
        
        let contract_id = env::current_account_id();

        let nonce = self.pending_nonce.expect("Missing pending nonce");
        // Step 1: Transfer input tokens from treasury to this contract
        let transfer_promise = Promise::new(input_token.clone())
            .function_call(
                "ft_transfer".to_string(),
                json!({
                    "receiver_id": contract_id.clone(),
                    "amount": amount_in_U128.to_string(),
                    "memo": format!("Swap #{}", nonce),
                })
                .to_string()
                .as_bytes()
                .to_vec(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(5),
            );
        
        let dex_contract = self.dex_contract.get(&0U64).expect("No DEX contract configured").clone();

        // Step 2: Execute DEX swap with slippage protection
        let swap_promise = transfer_promise.then(
            Promise::new(dex_contract.clone())
                .function_call(
                    "swap".to_string(),
                    json!({
                        "input_token": input_token.clone(),
                        "output_token": output_token.clone(),
                        "amount_in": amount_in_U128.to_string(),
                        "min_amount_out": min_amount_out_U128.to_string(),
                        "receiver_id": contract_id.clone(),
                    })
                    .to_string()
                    .as_bytes()
                    .to_vec(),
                    NearToken::from_yoctonear(0),
                    gas_from_tgas(10),
                )
        );
        
        // Step 3: After swap, transfer output to user and handle fees
        swap_promise.then(
            Self::ext(env::current_account_id())
                .with_static_gas(gas_from_tgas(1))
                .finalize_swap(
                    user_id,
                    input_token,
                    output_token,
                    amount_in,
                    min_amount_out,
                    expected_output.into(),
                    tx_hash,
                )
        )
    }
    
    /// Finalize swap: Transfer to user, take fees, emit event
    #[private]
    pub fn finalize_swap(
        &mut self,
        user_id: AccountId,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: U128,
        min_amount_out: U128,
        expected_output: U128,
        tx_hash: String,
    ) -> Promise {
        let promise_index = 0;
        // Check DEX swap result
        let swap_success = match env::promise_result_checked(promise_index, 100) {
            Ok(_result) => true,
            _ => false,
        };

        if !swap_success {
            self.pending_nonce = None;
            panic!("Dex swap failed");
        }
        
        
        let _amount_in_U128: U128 = amount_in.into();
        let _min_amount_out_U128: U128 = min_amount_out.into();
        let _expected_output_U128: U128 = expected_output.into();
        
        // Get actual output balance from contract (after DEX swap)
        let output_token_balance_promise = Promise::new(output_token.clone())
            .function_call(
                "ft_balance_of".to_string(),
                json!({
                    "account_id": env::current_account_id(),
                })
                .to_string()
                .as_bytes()
                .to_vec(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(10).saturating_div(2),
            );
        
        output_token_balance_promise.then(
            Self::ext(env::current_account_id())
                .with_static_gas(gas_from_tgas(5))
                .transfer_to_user_with_fees(
                    user_id,
                    input_token,
                    output_token,
                    amount_in,
                    min_amount_out,
                    expected_output,
                    tx_hash,
                )
        )
    }
    
    /// Transfer tokens to user with fee deduction
    #[private]
    pub fn transfer_to_user_with_fees(
        &mut self,
        user_id: AccountId,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: U128,
        min_amount_out: U128,
        expected_output: U128,
        tx_hash: String,
    ) -> Promise {
        // Get actual output balance
        // let balance_result = env::promise_result(0).try_into().unwrap();
        let promise_index = 0;

        let balance_result = match env::promise_result_checked(promise_index, 100) {
            Ok(result) => String::from_utf8(result).unwrap_or_default(),
            Err(_) => {
                self.pending_nonce = None;
                panic!("Failed to get balance")
            }
        };
        

        let actual_output: U128 = balance_result.parse().unwrap_or(0);
        
        let amount_in_U128: U128 = amount_in.into();
        let min_amount_out_U128: U128 = min_amount_out.into();
        let _expected_output_U128: U128 = expected_output.into();
        
        // SECURITY: Verify we received at least minimum output
        assert!(
            actual_output >= min_amount_out_U128,
            "Insufficient output received: {} < {}",
            actual_output,
            min_amount_out_U128
        );
        
        // Calculate and deduct fees
        let fee_amount = if self.fee_bps > 0 {
            actual_output
                .checked_mul(self.fee_bps as U128)
                .unwrap()
                .checked_div(10_000)
                .unwrap()
        } else {
            0
        };
        
        let user_amount = actual_output.checked_sub(fee_amount).unwrap();
        
        // SECURITY: Ensure user gets at least something
        assert!(user_amount > 0, "User amount must be positive");
        
        // Create promises for transfers
        // let mut promises = Vec::new();

        // // Transfer to user
        // if user_amount > 0 {
        //     promises.push(
        //         Promise::new(output_token.clone())
        //             .function_call(
        //                 "ft_transfer".to_string(),
        //                 json!({
        //                     "receiver_id": user_id.clone(),
        //                     "amount": user_amount.to_string(),
        //                     "memo": format!("Swap #{}, TX: {}", self.nonce, tx_hash),
        //                 })
        //                 .to_string()
        //                 .as_bytes()
        //                 .to_vec(),
        //                 NearToken::from_yoctonear(0),
        //                 gas_from_tgas(1),
        //             )
        //     );
        // }
        
        // // Transfer fees to treasury
        // if fee_amount > 0 {
        //     promises.push(
        //         Promise::new(output_token.clone())
        //             .function_call(
        //                 "ft_transfer".to_string(),
        //                 json!({
        //                     "receiver_id": self.treasury.clone(),
        //                     "amount": fee_amount.to_string(),
        //                     "memo": "Swap fee",
        //                 })
        //                 .to_string()
        //                 .as_bytes()
        //                 .to_vec(),
        //                 NearToken::from_yoctonear(0),
        //                 gas_from_tgas(1),
        //             )
        //     );
        // }

        let transfer_user = if user_amount > 0 {
            Some(
                Promise::new(output_token.clone()).function_call(
                    "ft_transfer".to_string(), 
                    json!({
                        "receiver_id": user_id.clone(),
                        "amount": user_amount.to_string(),
                        "memo": format!("Swap #{}, TX: {}", self.nonce, tx_hash),
                    })
                    .to_string()
                    .as_bytes(),
                    NearToken::from_yoctonear(0), 
                    gas_from_tgas(1),
                )
            )
        } else {
            None
        };

        let transfer_fee = if fee_amount > 0 {
            Some(
                Promise::new(output_token.clone()).function_call(
                    "ft_transfer".to_string(), 
                    json!({
                        "receiver_id": self.treasury.clone(),
                        "amount": fee_amount.to_string(),
                        "memo": "Swap fee",
                    })
                    .to_string()
                    .as_bytes(),
                    NearToken::from_yoctonear(0), 
                    gas_from_tgas(1),
                )
            )
        } else {
            None
        };
        
        // Emit swap event
        let swap_event = SwapEvent {
            user_id: user_id.clone(),
            input_token: input_token.clone(),
            output_token: output_token.clone(),
            amount_in: amount_in_U128,
            amount_out: user_amount,
            fee_amount,
            timestamp: env::block_timestamp(),
            tx_hash: tx_hash.clone(),
        };
        
        let nonce = self.pending_nonce.expect("Missing pending nonce");
        self.swap_events.insert(nonce.clone(), swap_event);

        log!("✅ Swap #{} completed successfully", nonce);
        log!("   User: {}, Received: {} {}", user_id, user_amount, output_token);
        log!("   Fee: {} {}", fee_amount, output_token);
        log!("   TX: {}", tx_hash);
        
       let final_promise = match (transfer_user, transfer_fee) {
        (Some(p1), Some(p2)) => p1.and(p2),
        (Some(p), None) | (None, Some(p)) => p,
        (None, None) => {
            return Promise::new(env::current_account_id());
        }
       };

       final_promise.then(
        Self::ext(env::current_account_id())
        .with_static_gas(gas_from_tgas(2))
        .log_final_result(tx_hash)
       )
    }

    
    /// Final logging
    #[private]
    pub fn log_final_result(&mut self, tx_hash: String) -> () {
        let committed = self
            .pending_nonce
            .take()
            .expect("No pending nonce to finalize");

        self.nonce = committed;
        log!("🎉 Swap finalized.");
        log!("Nonce committed: {}", self.nonce);
        log!("TX: {}", tx_hash);
    }
    
    /// --- ADMIN FUNCTIONS ---
    
    /// Update treasury (admin only)
    #[private]
    pub fn update_treasury(&mut self, new_treasury: AccountId) {
        self.assert_admin();
        self.treasury = new_treasury;
        log!("Treasury updated to: {}", self.treasury);
    }
    
    /// Update DEX contract (admin only)
    #[private]
    pub fn update_dex_contract(&mut self, new_dex: AccountId) {
        self.assert_admin();
        self.dex_contract.insert(0, new_dex);
        log!("DEX contract updated to: {}", self.dex_contract.get(&0U64).expect("No DEX contract configured"));
    }
    
    /// Update fees (admin only)
    #[private]
    pub fn update_fees(&mut self, new_fee_bps: u32) {
        self.assert_admin();
        assert!(new_fee_bps <= MAX_FEE_BPS, "Fee too high");
        self.fee_bps = new_fee_bps;
        log!("Fees updated to: {} bps", self.fee_bps);
    }
    
    /// Toggle pause (admin only)
    #[private]
    pub fn toggle_pause(&mut self, paused: bool) {
        self.assert_admin();
        self.is_paused = paused;
        log!("Contract {} by admin", 
             if paused { "paused" } else { "unpaused" });
    }
    
    /// Add whitelisted token (admin only)
    #[private]
    pub fn add_whitelisted_token(&mut self, token: AccountId) {
        self.assert_admin();
        let exists = self.whitelisted_tokens.get(&token);
        if exists.is_none() {
            self.whitelisted_tokens.insert(token.clone(), true);
            self.whitelisted_tokens_count += 1;
            log!("Token whitelisted: {}", token);
        }
    }
    
    /// Remove whitelisted token (admin only)
    #[private]
    pub fn remove_whitelisted_token(&mut self, token: AccountId) {
        self.assert_admin();
        self.whitelisted_tokens.remove(&token);
        log!("Token removed from whitelist: {}", token);
    }
    
    /// Emergency withdrawal (admin only) - in case tokens get stuck
    #[private]
    pub fn emergency_withdraw(
        &mut self,
        token: AccountId,
        amount: U128,
        recipient: AccountId,
    ) -> Promise {
        self.assert_admin();
        
        let amount_U128: U128 = amount.into();
        log!("⚠️ Emergency withdrawal: {} {} to {}", 
             amount_U128, token, recipient);
        
        Promise::new(token)
            .function_call(
                "ft_transfer".to_string(),
                json!({
                    "receiver_id": recipient,
                    "amount": amount_U128.to_string(),
                    "memo": "Emergency withdrawal",
                })
                .to_string()
                .as_bytes()
                .to_vec(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(1),
            )
    }

    #[private]
    pub fn on_transfers_complete(&mut self) {
        let promise_index = 0;
        let result = match env::promise_result_checked(promise_index, 100) {
            Ok(_result) => true,
            _ => false,
        };

        if !result {
            self.pending_nonce = None;
            panic!("Token transfer failed");
        }

        self.log_final_result("ok".to_string());
    }
    
    /// --- VIEW FUNCTIONS ---
    
    /// Get swap quote (read-only)
    pub fn get_swap_quote(
        &self,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: U128,
    ) -> Promise {

        let dex_contract = self.dex_contract.get(&0U64).expect("No DEX contract configured").clone();
        Promise::new(dex_contract.clone())
            .function_call(
                "get_quote".to_string(),
                json!({
                    "input_token": input_token,
                    "output_token": output_token,
                    "amount_in": amount_in.0.to_string(),
                })
                .to_string()
                .as_bytes()
                .to_vec(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(1),
            )
    }
    
    /// Check if token is whitelisted
    pub fn is_token_whitelisted(&self, token: AccountId) -> bool {
        self.whitelisted_tokens.contains_key(&token)
    }
    
    /// Get contract status
    pub fn get_status(&self) -> Value {
        json!({
            "treasury": self.treasury,
            "dex_contract": self.dex_contract.get(&0U64).map(|c| c.to_string()).unwrap_or_else(|| "Not set".to_string()),
            "admin": self.admin,
            "fee_bps": self.fee_bps,
            "is_paused": self.is_paused,
            "nonce": self.nonce,
            "whitelisted_tokens_count": self.whitelisted_tokens_count,
        })
    }
    
    /// Get swap event by nonce
    pub fn get_swap_event(&self, nonce: U64) -> Option<&SwapEvent> {
        self.swap_events.get(&nonce)
    }
    
    /// --- PRIVATE HELPERS ---
    
    /// Assert caller is admin
    fn assert_admin(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.admin,
            "Only admin can call this"
        );
    }
}

impl ContractState for TokenSwapContract {}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{VMContextBuilder, get_logs};
    use near_sdk::{testing_env, AccountId};
    use near_sdk::MockedBlockchain;

    #[test]
    fn test_secure_initialization() {
        let treasury = AccountId::new_unchecked("treasury.near".to_string());
        let dex = AccountId::new_unchecked("dex.near".to_string());
        let admin = AccountId::new_unchecked("admin.near".to_string());
        
        let contract = TokenSwapContract::new(
            treasury.clone(),
            dex.clone(),
            admin.clone(),
            10
        );
        
        assert_eq!(contract.treasury, treasury);
        assert_eq!(contract.dex_contract, dex);
        assert_eq!(contract.admin, admin);
        assert_eq!(contract.fee_bps, 10);
        assert!(!contract.is_paused);
        assert_eq!(contract.nonce, 0);
    }
    
    #[test]
    #[should_panic(expected = "Fee too high")]
    fn test_fee_too_high() {
        let context = VMContextBuilder::new()
            .current_account_id(AccountId::new_unchecked("contract.near".to_string()))
            .predecessor_account_id(AccountId::new_unchecked("admin.near".to_string()))
            .build();
        
        testing_env!(context);
        
        // Should panic because fee > MAX_FEE_BPS (500)
        let _ = TokenSwapContract::new(
            AccountId::new_unchecked("treasury.near".to_string()),
            AccountId::new_unchecked("dex.near".to_string()),
            AccountId::new_unchecked("admin.near".to_string()),
            600 // 6% > 5% max
        );
    }
}


