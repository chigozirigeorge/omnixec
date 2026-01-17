use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::{
    env, ext_contract, log, near, require,
    AccountId, Gas, NearToken, PanicOnDefault,
    Promise, serde_json,
};
use near_sdk::json_types::U128;
use near_sdk::store::LookupMap;
use near_sdk::serde::{Deserialize, Serialize};

const MAX_FEE_BPS: u32 = 500;
const STORAGE_PREFUND: u128 = 12_500_000_000_000_000_000_000; // 0.0125 NEAR
const GAS_STORAGE: Gas = Gas::from_tgas(5);
const GAS_SWAP: Gas = Gas::from_tgas(30);
const GAS_CALLBACK: Gas = Gas::from_tgas(10);

/// -----------------------------
/// External Interfaces
/// -----------------------------

#[ext_contract(ext_ft)]
pub trait FungibleToken {
    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance>;
    fn storage_deposit(&mut self, account_id: Option<AccountId>, registration_only: Option<bool>);
    fn ft_balance_of(&self, account_id: AccountId) -> U128;
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}

#[ext_contract(ext_dex)]
pub trait DexAdapter {
    fn swap(
        &mut self,
        actions: Vec<DexSwapAction>,
        referral_id: Option<AccountId>,
    );
}

#[derive(Deserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct StorageBalance {
    pub total: U128,
    pub available: U128,
}

/// -----------------------------
/// Swap Action
/// -----------------------------

#[derive(Deserialize, Serialize, Clone, BorshDeserialize, BorshSerialize)]
#[serde(crate = "near_sdk::serde")]
pub struct DexSwapAction {
    pub pool_id: u64,
    pub token_in: AccountId,
    pub token_out: AccountId,
    pub amount_in: U128,
    pub min_amount_out: U128,
}

/// -----------------------------
/// Swap Record
/// -----------------------------

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct SwapEvent {
    pub user: AccountId,
    pub dex: AccountId,
    pub token_in: AccountId,
    pub token_out: AccountId,
    pub amount_in: U128,
    pub amount_out: U128,
    pub fee_paid: U128,
    pub timestamp: u64,
}

/// -----------------------------
/// Contract State
/// -----------------------------

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct TokenSwapContract {
    pub admin: AccountId,
    pub treasury: AccountId,
    pub fee_bps: u32,

    pub whitelisted_tokens: LookupMap<AccountId, bool>,
    pub whitelisted_dexes: LookupMap<AccountId, bool>,

    pub swap_nonce: u64,
    pub swaps: LookupMap<u64, SwapEvent>,
}

/// -----------------------------
/// Init
/// -----------------------------

#[near]
impl TokenSwapContract {
    #[init]
    pub fn init(admin: AccountId, treasury: AccountId, fee_bps: u32) -> Self {
        require!(fee_bps <= MAX_FEE_BPS, "Fee too high");

        Self {
            admin,
            treasury,
            fee_bps,
            whitelisted_tokens: LookupMap::new(b"t"),
            whitelisted_dexes: LookupMap::new(b"d"),
            swap_nonce: 0,
            swaps: LookupMap::new(b"s"),
        }
    }

        #[payable]
    pub fn swap(
        &mut self,
        user: AccountId,
        dex: AccountId,
        actions: Vec<DexSwapAction>,
    ) -> Promise {
        require!(env::predecessor_account_id() == self.treasury, "Treasury only");
        require!(self.whitelisted_dexes.contains_key(&dex), "DEX not allowed");
        require!(!actions.is_empty(), "No actions");

        let token_out = actions.last().unwrap().token_out.clone();

        for a in &actions {
            require!(self.whitelisted_tokens.contains_key(&a.token_in), "Token in not allowed");
            require!(self.whitelisted_tokens.contains_key(&a.token_out), "Token out not allowed");
        }

        // Step 1: ensure user storage
        ext_ft::ext(token_out.clone())
            .with_static_gas(GAS_STORAGE)
            .storage_balance_of(user.clone())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CALLBACK)
                    .on_storage_checked(user, dex, actions),
            )
    }

        #[private]
    pub fn on_storage_checked(
        &mut self,
        user: AccountId,
        dex: AccountId,
        actions: Vec<DexSwapAction>,
    ) -> Promise {
        let needs_storage = match env::promise_result_checked(0, 100) {
            Ok(bytes) => {
                serde_json::from_slice::<Option<StorageBalance>>(&bytes)
                    .unwrap()
                    .is_none()
            }
            _ => true,
        };

        let token_out = actions.last().unwrap().token_out.clone();

        if needs_storage {
            ext_ft::ext(token_out)
                .with_attached_deposit(NearToken::from_yoctonear(STORAGE_PREFUND))
                .with_static_gas(GAS_STORAGE)
                .storage_deposit(Some(user), Some(true))
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_CALLBACK)
                        .execute_swap(dex, actions),
                )
        } else {
            self.execute_swap(dex, actions)
        }
    }

        #[private]
    pub fn execute_swap(
        &mut self,
        dex: AccountId,
        actions: Vec<DexSwapAction>,
    ) -> Promise {
        let token_out = actions.last().unwrap().token_out.clone();

        ext_ft::ext(token_out.clone())
            .with_static_gas(GAS_STORAGE)
            .ft_balance_of(env::current_account_id())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CALLBACK)
                    .on_pre_balance(dex, actions),
            )
    }

        #[private]
    pub fn on_pre_balance(
        &mut self,
        dex: AccountId,
        actions: Vec<DexSwapAction>,
    ) -> Promise {
        let pre: U128 = near_sdk::serde_json::from_slice(
            &env::promise_result_checked(0, 100).unwrap(),
        )
        .unwrap();

        ext_dex::ext(dex)
            .with_static_gas(GAS_SWAP)
            .swap(actions.clone(), None)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CALLBACK)
                    .on_post_balance(pre, actions),
            )
    }

    #[private]
    pub fn on_post_balance(
        &mut self,
        pre: U128,
        actions: Vec<DexSwapAction>,
    ) -> Promise {
        let token_out = actions.last().unwrap().token_out.clone();
        let user = env::predecessor_account_id();

        ext_ft::ext(token_out)
            .with_static_gas(GAS_STORAGE)
            .ft_balance_of(env::current_account_id())
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CALLBACK)
                    .finalize_swap(user, pre, actions),
            )
    }

        #[private]
    pub fn finalize_swap(
        &mut self,
        user: AccountId,
        pre: U128,
        actions: Vec<DexSwapAction>,
    ) -> Promise {
        let post: U128 =
            near_sdk::serde_json::from_slice(&env::promise_result_checked(0, 100).unwrap()).unwrap();

        let delta = post.0.saturating_sub(pre.0);
        require!(delta > 0, "No output");

        let fee = delta * self.fee_bps as u128 / 10_000;
        let user_amount = delta - fee;

        let token_out = actions.last().unwrap().token_out.clone();

        self.swap_nonce += 1;
        self.swaps.insert(
            self.swap_nonce,
            SwapEvent {
                user: user.clone(),
                dex: env::predecessor_account_id(),
                token_in: actions[0].token_in.clone(),
                token_out: token_out.clone(),
                amount_in: actions[0].amount_in,
                amount_out: U128(user_amount),
                fee_paid: U128(fee),
                timestamp: env::block_timestamp(),
            },
        );

        let mut p = Promise::new(token_out.clone());

        if fee > 0 {
            p = p.function_call(
                "ft_transfer",
                serde_json::json!({
                    "receiver_id": self.treasury,
                    "amount": fee.to_string(),
                    "memo": "Swap fee"
                })
                .to_string()
                .into_bytes(),
                NearToken::from_yoctonear(0),
                Gas::from_tgas(5),
            );
        }

        p.function_call(
            "ft_transfer",
            serde_json::json!({
                "receiver_id": user,
                "amount": user_amount.to_string(),
                "memo": "Swap output"
            })
            .to_string()
            .into_bytes(),
            NearToken::from_yoctonear(0),
            Gas::from_tgas(5),
        )
    }

    fn assert_admin(&self) {
        require!(
            env::predecessor_account_id() == self.admin,
            "Admin only"
        );
    }

    pub fn add_token(&mut self, token: AccountId) {
        self.assert_admin();
        require!(
            !self.whitelisted_tokens.contains_key(&token),
            "Token already whitelisted"
        );

        self.whitelisted_tokens.insert(token.clone(), true);
        log!("Token whitelisted: {}", token);
    }

    pub fn remove_token(&mut self, token: AccountId) {
        self.assert_admin();
        require!(
            self.whitelisted_tokens.remove(&token).is_some(),
            "Token not whitelisted"
        );

        log!("Token removed: {}", token);
    }

    pub fn add_dex(&mut self, dex: AccountId) {
        self.assert_admin();
        require!(
            !self.whitelisted_dexes.contains_key(&dex),
            "DEX already whitelisted"
        );

        self.whitelisted_dexes.insert(dex.clone(), true);
        log!("DEX whitelisted: {}", dex);
    }

    pub fn remove_dex(&mut self, dex: AccountId) {
        self.assert_admin();
        require!(
            self.whitelisted_dexes.remove(&dex).is_some(),
            "DEX not whitelisted"
        );

        log!("DEX removed: {}", dex);
    }
}