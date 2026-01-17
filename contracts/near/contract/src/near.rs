use near_sdk::{
    env, ext_contract, log, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseResult
};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde_json::{self,json, Value};
use near_sdk::store::LookupMap;

fn gas_from_tgas(tgas: u64) -> Gas {
    Gas::from_tgas(tgas)
}

const MAX_FEE_BPS: u32 = 500;
const MIN_SLIPPAGE_BPS: u32 = 10;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct TokenSwapContract {
    pub treasury: AccountId,
    pub dex_contract: LookupMap<U64, AccountId>,
    pub fee_bps: u32,
    pub admin: AccountId,
    pub is_paused: bool,
    pub nonce: U64,
    pub pending_nonce: Option<U64>,
    pub whitelisted_tokens: LookupMap<AccountId, bool>,
    pub whitelisted_tokens_count: U64,
    pub swap_events: LookupMap<U64, SwapEvent>,
}

#[near(serializers = [json, borsh])]
#[derive(Clone)]
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

#[ext_contract(ext_self)]
pub trait ExtSelf {
    fn execute_swap_after_quote(
        &mut self,
        user_id: AccountId,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: U128,
        min_amount_out: U128,
        max_slippage_bps: u32,
        tx_hash: String,
    ) -> Promise;

    fn finalize_swap(
        &mut self,
        user_id: AccountId,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: U128,
        min_amount_out: U128,
        expected_output: U128,
        tx_hash: String,
    ) -> Promise;

    fn transfer_to_user_with_fees(
        &mut self,
        user_id: AccountId,
        input_token: AccountId,
        output_token: AccountId,
        amount_in: U128,
        min_amount_out: U128,
        expected_output: U128,
        tx_hash: String,
    ) -> Promise;

    fn log_final_result(&mut self, tx_hash: String);
}

#[near]
impl TokenSwapContract {
    #[init]
    pub fn init(
        treasury: AccountId,
        dex_contract_id: AccountId,
        admin: AccountId,
        fee_bps: u32,
    ) -> Self {
        require!(fee_bps <= MAX_FEE_BPS, "Fee too high");

        let mut dex_contract = LookupMap::new(b"d".to_vec());
        dex_contract.insert(U64(0), dex_contract_id);

        Self {
            treasury,
            dex_contract,
            admin,
            fee_bps,
            is_paused: false,
            nonce: U64(0),
            pending_nonce: None,
            whitelisted_tokens: LookupMap::new(b"w".to_vec()),
            whitelisted_tokens_count: U64(0),
            swap_events: LookupMap::new(b"e".to_vec()),
        }
    }

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
        require!(!self.is_paused, "Contract is paused");
        require!(
            env::predecessor_account_id() == self.treasury,
            "Only treasury can initiate swaps"
        );
        require!(self.pending_nonce.is_none(), "Swap already in progress");

        let expected = U64(self.nonce.0 + 1);
        require!(signature_nonce == expected, "Invalid nonce");
        self.pending_nonce = Some(expected);

        require!(
            self.whitelisted_tokens.contains_key(&input_token),
            "Input token not whitelisted"
        );
        require!(
            self.whitelisted_tokens.contains_key(&output_token),
            "Output token not whitelisted"
        );
        require!(amount_in.0 > 0, "Amount must be positive");
        require!(min_amount_out.0 > 0, "Minimum output must be positive");
        require!(
            max_slippage_bps >= MIN_SLIPPAGE_BPS && max_slippage_bps <= 1000,
            "Slippage invalid"
        );

        let tx_hash = hex::encode(env::sha256(&env::random_seed()));
        let dex = self.dex_contract.get(&U64(0)).expect("DEX not set").clone();

        Promise::new(dex)
            .function_call(
                "get_quote".to_string(),
                json!({
                    "input_token": input_token,
                    "output_token": output_token,
                    "amount_in": amount_in.0.to_string(),
                })
                .to_string()
                .into_bytes(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(5),
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(gas_from_tgas(5))
                    .execute_swap_after_quote(
                        user_id,
                        input_token,
                        output_token,
                        amount_in,
                        min_amount_out,
                        max_slippage_bps,
                        tx_hash,
                    ),
            )
    }

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
        let quote_bytes = match env::promise_result_checked(0, 100) {
           Ok(bytes) => bytes,
           Err(_) => env::panic_str("Quote Promise failed"),
        };

        let v: Value = serde_json::from_slice(&quote_bytes).unwrap_or_default();
        let expected_output = U128(
            v["amount_out"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        );

        require!(
            expected_output.0 >= min_amount_out.0,
            "Quote below minimum"
        );

        let dex = self.dex_contract.get(&U64(0)).unwrap().clone();
        Promise::new(dex)
            .function_call(
                "swap".to_string(),
                json!({
                    "input_token": input_token,
                    "output_token": output_token,
                    "amount_in": amount_in.0.to_string(),
                    "min_amount_out": min_amount_out.0.to_string(),
                    "receiver_id": env::current_account_id(),
                })
                .to_string()
                .into_bytes(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(10),
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(gas_from_tgas(5))
                    .finalize_swap(
                        user_id,
                        input_token,
                        output_token,
                        amount_in,
                        min_amount_out,
                        expected_output,
                        tx_hash,
                    ),
            )
    }

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
        Promise::new(output_token.clone())
            .function_call(
                "ft_balance_of".to_string(),
                json!({ "account_id": env::current_account_id() })
                    .to_string()
                    .into_bytes(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(5),
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(gas_from_tgas(5))
                    .transfer_to_user_with_fees(
                        user_id,
                        input_token,
                        output_token,
                        amount_in,
                        min_amount_out,
                        expected_output,
                        tx_hash,
                    ),
            )
    }

    #[private]
    pub fn transfer_to_user_with_fees(
        &mut self,
        user_id: AccountId,
        input_token: AccountId,
        output_token: AccountId,
        _amount_in: U128,
        min_amount_out: U128,
        expected_output: U128,
        tx_hash: String,
    ) -> Promise {
        let balance_bytes = match env::promise_result_checked(0, 100) {
            Ok(bytes) => bytes,
            Err(_) => env::panic_str("Balance Promise failed"),
        };
        let actual_output = U128(String::from_utf8(balance_bytes).unwrap().parse().unwrap());

        require!(actual_output.0 >= min_amount_out.0, "Insufficient output");

        let fee = U128(actual_output.0 * self.fee_bps as u128 / 10_000);
        let user_amount = actual_output.0 - fee.0;

        let nonce = self.pending_nonce.take().unwrap(); // clear pending
        self.swap_events.insert(
            nonce,
            SwapEvent {
                user_id: user_id.clone(),
                input_token,
                output_token: output_token.clone(),
                amount_in: _amount_in,
                amount_out: U128(user_amount),
                fee_amount: fee,
                timestamp: U64(env::block_timestamp()),
                tx_hash: tx_hash.clone(),
            },
        );

        // Transfer fee and user amount
        let fee_promise = if fee.0 > 0 {
            Some(Promise::new(output_token.clone()).function_call(
                "ft_transfer".to_string(),
                json!({
                    "receiver_id": self.treasury.clone(),
                    "amount": fee.0.to_string(),
                    "memo": "Swap fee",
                })
                .to_string()
                .into_bytes(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(1),
            ))
        } else {
            None
        };

        let user_promise = if user_amount > 0 {
            Some(Promise::new(output_token.clone()).function_call(
                "ft_transfer".to_string(),
                json!({
                    "receiver_id": user_id.clone(),
                    "amount": user_amount.to_string(),
                    "memo": format!("Swap #{}, TX: {}", nonce.0, tx_hash),
                })
                .to_string()
                .into_bytes(),
                NearToken::from_yoctonear(0),
                gas_from_tgas(1),
            ))
        } else {
            None
        };

        let final_promise = match (fee_promise, user_promise) {
            (Some(f), Some(u)) => f.and(u),
            (Some(f), None) => f,
            (None, Some(u)) => u,
            (None, None) => Promise::new(env::current_account_id()),
        };

        final_promise.then(
            ext_self::ext(env::current_account_id())
                .with_static_gas(gas_from_tgas(2))
                .log_final_result(tx_hash),
        )
    }

    #[private]
    pub fn log_final_result(&mut self, _tx_hash: String) {
        log!("✅ Swap finalized, nonce {}", self.nonce.0);
    }

    pub fn get_swap_event(&self, nonce: U64) -> Option<SwapEvent> {
        self.swap_events.get(&nonce).cloned()
    }

    // --- ADMIN FUNCTIONS ---

    pub fn update_treasury(&mut self, new_treasury: AccountId) {
        self.assert_admin();
        self.treasury = new_treasury;
        log!("Treasury updated to {}", self.treasury);
    }

    pub fn update_dex(&mut self, new_dex: AccountId) {
        self.assert_admin();
        self.dex_contract.insert(U64(0), new_dex.clone());
        log!("DEX updated to {}", new_dex);
    }

    pub fn update_fee(&mut self, new_fee_bps: u32) {
        self.assert_admin();
        require!(new_fee_bps <= MAX_FEE_BPS, "Fee too high");
        self.fee_bps = new_fee_bps;
        log!("Fee updated to {} bps", self.fee_bps);
    }

    pub fn toggle_pause(&mut self, pause: bool) {
        self.assert_admin();
        self.is_paused = pause;
        log!("Contract {}", if pause { "paused" } else { "unpaused" });
    }

    pub fn add_whitelist(&mut self, token: AccountId) {
        self.assert_admin();
        if !self.whitelisted_tokens.contains_key(&token) {
            self.whitelisted_tokens.insert(token.clone(), true);
            self.whitelisted_tokens_count.0 += 1;
            log!("Token whitelisted: {}", token);
        }
    }

    pub fn remove_whitelist(&mut self, token: AccountId) {
        self.assert_admin();
        if self.whitelisted_tokens.remove(&token).is_some() {
            self.whitelisted_tokens_count.0 -= 1;
            log!("Token removed from whitelist: {}", token);
        }
    }

    pub fn clear_pending(&mut self) {
        self.assert_admin();
        self.pending_nonce = None;
        log!("Pending nonce cleared");
    }

    pub fn emergency_withdraw(
        &mut self,
        token: AccountId,
        amount: U128,
        recipient: AccountId,
    ) -> Promise {
        self.assert_admin();
        log!("⚠️ Emergency withdraw {} {} to {}", amount.0, token, recipient);
        Promise::new(token).function_call(
            "ft_transfer".to_string(),
            json!({
                "receiver_id": recipient,
                "amount": amount.0.to_string(),
                "memo": "Emergency withdrawal",
            })
            .to_string()
            .into_bytes(),
            NearToken::from_yoctonear(0),
            gas_from_tgas(1),
        )
    }

    fn assert_admin(&self) {
        require!(env::predecessor_account_id() == self.admin, "Admin only");
    }
}
