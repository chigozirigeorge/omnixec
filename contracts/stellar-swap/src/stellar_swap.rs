#![no_std]
use soroban_sdk::{
    contract, contractimpl, symbol_short, vec, Address, Env,
    Symbol, Vec, FromVal, IntoVal, TryFromVal, token,
};
use soroban_token_sdk::TokenClient;

#[contract]
pub struct TokenSwapContract;

/// Token Swap Contract for Stellar
/// 
/// Executes atomic token swaps on Stellar with slippage protection
/// 
/// SECURITY:
/// - Requires treasury authorization via require_auth()
/// - Validates minimum output (slippage protection)
/// - All-or-nothing execution (reverts on any error)
#[contractimpl]
impl TokenSwapContract {
    /// Execute a token swap on Stellar DEX
    /// 
    /// # Arguments
    /// * `user` - User's wallet address (receives output tokens)
    /// * `treasury` - Treasury address that holds input tokens
    /// * `input_token` - Token address to send from treasury
    /// * `output_token` - Token address to receive
    /// * `amount_in` - Amount of input token to swap
    /// * `min_amount_out` - Minimum amount of output token (slippage protection)
    /// * `dex_address` - DEX contract address (e.g., Ref Finance)
    /// 
    /// # Returns
    /// Amount sent to user (after fees)
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
        // SECURITY: Verify treasury authorization
        treasury.require_auth();

        env.events().publish(
            ("swap", "initiated"),
            (
                user.clone(),
                amount_in,
                min_amount_out,
            ),
        );

        // Step 1: Transfer input tokens from treasury to this contract
        let token_in_client = TokenClient::new(&env, &input_token);
        token_in_client.transfer(
            &treasury,
            &env.current_contract_address(),
            &amount_in,
        );

        env.events().publish(
            ("swap", "tokens_transferred_to_contract"),
            amount_in,
        );

        // Step 2: Approve DEX to spend the input tokens
        token_in_client.approve(
            &env.current_contract_address(),
            &dex_address,
            &amount_in,
            &(env.ledger().sequence() + 1000),
        );

        env.events().publish(
            ("swap", "dex_approved"),
            amount_in,
        );

        // Step 3: Call DEX swap function
        // Returns the amount of output tokens received
        let amount_out: i128 = env.invoke_contract(
            &dex_address,
            &symbol_short!("swap"),
            &vec![
                &env,
                input_token.clone().into_val(&env),
                output_token.clone().into_val(&env),
                amount_in.into_val(&env),
            ],
        );

        env.events().publish(
            ("swap", "dex_swap_complete"),
            amount_out,
        );

        // Step 4: Verify minimum output amount (slippage check)
        if amount_out < min_amount_out {
            env.panic_with_error(
                soroban_sdk::Error::from_contract_error(1)
            );
        }

        env.events().publish(
            ("swap", "slippage_check_passed"),
            (min_amount_out, amount_out),
        );

        // Step 5: Calculate fee (0.1%)
        let fee = amount_out / 1000;
        let user_amount = amount_out - fee;

        // Step 6: Transfer output tokens to user
        let token_out_client = TokenClient::new(&env, &output_token);
        token_out_client.transfer(
            &env.current_contract_address(),
            &user,
            &user_amount,
        );

        env.events().publish(
            ("swap", "tokens_transferred_to_user"),
            user_amount,
        );

        // Step 7: Emit completion event
        env.events().publish(
            ("swap", "completed"),
            (
                user.clone(),
                user_amount,
                fee,
            ),
        );

        user_amount
    }

    /// Get a quote for a potential swap
    /// View function - does not modify state
    pub fn get_swap_quote(
        env: Env,
        input_token: Address,
        output_token: Address,
        amount_in: i128,
        dex_address: Address,
    ) -> i128 {
        env.invoke_contract(
            &dex_address,
            &symbol_short!("getquote"),
            &vec![
                &env,
                input_token.into_val(&env),
                output_token.into_val(&env),
                amount_in.into_val(&env),
            ],
        )
    }

    /// Get treasury address
    pub fn get_treasury(_env: Env) -> Address {
        Address::from_contract_id(&[0; 32])
    }
}
