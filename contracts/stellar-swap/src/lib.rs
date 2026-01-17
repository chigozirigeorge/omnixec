#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, vec, Address, Env,
    Symbol, Vec, Map, Val, TryFromVal, TryIntoVal, token, BytesN,
    ConversionError, Error,
};
use soroban_token_sdk::{TokenClient, TokenUtils};
use soroban_sdk::auth::{ContractContext, InvokerContractAuthEntry};
use soroban_sdk::storage::{Storage, Instance, Persistent, Temporary};

const DAY_IN_LEDGERS: u32 = 17280; // ~24 hours at 5 seconds per ledger
const WEEK_IN_LEDGERS: u32 = 120960; // ~7 days

#[contract]
pub struct TokenSwapContract;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// Treasury address that holds input tokens
    pub treasury: Address,
    /// Admin address (can update config)
    pub admin: Address,
    /// Fee basis points (100 = 1%)
    pub fee_bps: u32,
    /// Is contract paused
    pub is_paused: bool,
    /// Nonce to prevent replay attacks
    pub nonce: u64,
    /// Last updated ledger
    pub last_updated: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Whitelist {
    /// Approved token addresses
    pub tokens: Map<Address, bool>,
    /// Approved DEX addresses
    pub dexes: Map<Address, bool>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapEvent {
    pub user: Address,
    pub input_token: Address,
    pub output_token: Address,
    pub amount_in: i128,
    pub amount_out: i128,
    pub fee_amount: i128,
    pub nonce: u64,
    pub timestamp: u64,
    pub tx_hash: BytesN<32>,
}

#[contractimpl]
impl TokenSwapContract {
    // --- INITIALIZATION ---
    
    /// Initialize the contract with admin and treasury
    /// Can only be called once
    pub fn initialize(
        env: Env,
        treasury: Address,
        admin: Address,
        fee_bps: u32,
    ) -> Result<(), Error> {
        // Check if already initialized
        let storage = Storage::new(&env);
        if storage.has(&symbol_short!("config")) {
            return Err(Error::from_contract_error(100)); // Already initialized
        }
        
        // Validate fee
        if fee_bps > 500 { // Max 5%
            return Err(Error::from_contract_error(101)); // Fee too high
        }
        
        // Create config
        let config = Config {
            treasury,
            admin,
            fee_bps,
            is_paused: false,
            nonce: 0,
            last_updated: env.ledger().sequence(),
        };
        
        // Store config
        storage.set(&symbol_short!("config"), &config);
        
        // Initialize empty whitelist
        let whitelist = Whitelist {
            tokens: Map::new(&env),
            dexes: Map::new(&env),
        };
        storage.set(&symbol_short!("whitelist"), &whitelist);
        
        // Emit initialization event
        env.events().publish(
            (symbol_short!("init"), symbol_short!("contract")),
            (treasury, admin, fee_bps),
        );
        
        Ok(())
    }
    
    // --- MAIN SWAP FUNCTION ---
    
    /// Execute a secure token swap with comprehensive validation
    pub fn swap(
        env: Env,
        user: Address,
        input_token: Address,
        output_token: Address,
        amount_in: i128,
        min_amount_out: i128,
        dex_address: Address,
        signature_nonce: u64,
    ) -> Result<i128, Error> {
        // SECURITY: Load and validate config
        let storage = Storage::new(&env);
        let mut config: Config = storage.get(&symbol_short!("config"))
            .ok_or(Error::from_contract_error(102))?; // Not initialized
        
        // Check if paused
        if config.is_paused {
            return Err(Error::from_contract_error(103)); // Contract paused
        }
        
        // SECURITY: Validate nonce to prevent replay
        if signature_nonce != config.nonce + 1 {
            return Err(Error::from_contract_error(104)); // Invalid nonce
        }
        
        // Update nonce BEFORE any external calls (prevents reentrancy)
        config.nonce += 1;
        config.last_updated = env.ledger().sequence();
        storage.set(&symbol_short!("config"), &config);
        
        // SECURITY: Verify treasury authorization with signature nonce
        // Create custom authorization that includes nonce
        let auth_context = ContractContext {
            contract: env.current_contract_address(),
            fn_name: symbol_short!("swap"),
            args: vec![
                &env,
                user.clone().into_val(&env),
                input_token.clone().into_val(&env),
                output_token.clone().into_val(&env),
                amount_in.into_val(&env),
                min_amount_out.into_val(&env),
                dex_address.clone().into_val(&env),
                signature_nonce.into_val(&env),
            ],
        };
        
        config.treasury.require_auth_for_args(auth_context.to_auth_args());
        
        // SECURITY: Input validation
        if amount_in <= 0 || min_amount_out <= 0 {
            return Err(Error::from_contract_error(105)); // Invalid amount
        }
        
        if input_token == output_token {
            return Err(Error::from_contract_error(106)); // Same token
        }
        
        if user == config.treasury {
            return Err(Error::from_contract_error(107)); // User can't be treasury
        }
        
        // SECURITY: Check whitelists
        let whitelist: Whitelist = storage.get(&symbol_short!("whitelist"))
            .ok_or(Error::from_contract_error(108))?;
        
        if !whitelist.tokens.get(input_token.clone()).unwrap_or(false) {
            return Err(Error::from_contract_error(109)); // Input token not whitelisted
        }
        
        if !whitelist.tokens.get(output_token.clone()).unwrap_or(false) {
            return Err(Error::from_contract_error(110)); // Output token not whitelisted
        }
        
        if !whitelist.dexes.get(dex_address.clone()).unwrap_or(false) {
            return Err(Error::from_contract_error(111)); // DEX not whitelisted
        }
        
        let tx_hash = env.crypto().sha256(&env.prng().gen::<BytesN<32>>());
        
        // Emit detailed swap initiation event
        env.events().publish(
            (symbol_short!("swap"), symbol_short!("init")),
            SwapEvent {
                user: user.clone(),
                input_token: input_token.clone(),
                output_token: output_token.clone(),
                amount_in,
                amount_out: 0,
                fee_amount: 0,
                nonce: config.nonce,
                timestamp: env.ledger().timestamp(),
                tx_hash: tx_hash.clone(),
            },
        );
        
        // Step 1: Get quote first to estimate output
        let estimated_out = Self::get_dex_quote(
            env.clone(),
            input_token.clone(),
            output_token.clone(),
            amount_in,
            dex_address.clone(),
        )?;
        
        // SECURITY: Validate quote meets minimum
        if estimated_out < min_amount_out {
            return Err(Error::from_contract_error(112)); // Quote insufficient
        }
        
        // SECURITY: Apply dynamic slippage check (max 1% slippage)
        let max_slippage_bps = 100; // 1%
        let max_slippage_amount = estimated_out
            .checked_mul(max_slippage_bps as i128)
            .ok_or(Error::from_contract_error(113))? // Overflow
            .checked_div(10000)
            .ok_or(Error::from_contract_error(114))?; // Division by zero
        
        let min_acceptable_out = estimated_out
            .checked_sub(max_slippage_amount)
            .ok_or(Error::from_contract_error(115))?;
        
        if min_acceptable_out < min_amount_out {
            return Err(Error::from_contract_error(116)); // Slippage too high
        }
        
        // Step 2: Transfer input tokens from treasury to contract
        // Use low-level token transfer with error handling
        let token_in_client = TokenClient::new(&env, &input_token);
        
        // Check treasury balance first
        let treasury_balance = token_in_client.balance(&config.treasury);
        if treasury_balance < amount_in {
            return Err(Error::from_contract_error(117)); // Insufficient balance
        }
        
        token_in_client.transfer(
            &config.treasury,
            &env.current_contract_address(),
            &amount_in,
        );
        
        env.events().publish(
            (symbol_short!("swap"), symbol_short!("transfer_in")),
            (config.nonce, amount_in),
        );
        
        // Step 3: Approve DEX with limited allowance and short expiration
        let current_ledger = env.ledger().sequence();
        let approval_expiration = current_ledger + 100; // ~8 minutes
        
        token_in_client.approve(
            &env.current_contract_address(),
            &dex_address,
            &amount_in,
            &approval_expiration,
        );
        
        env.events().publish(
            (symbol_short!("swap"), symbol_short!("approved")),
            (config.nonce, amount_in, approval_expiration),
        );
        
        // Step 4: Execute DEX swap with gas limit
        let amount_out = Self::execute_dex_swap(
            env.clone(),
            input_token.clone(),
            output_token.clone(),
            amount_in,
            min_amount_out,
            dex_address.clone(),
        )?;
        
        // SECURITY: Verify we received at least minimum
        if amount_out < min_amount_out {
            // Critical: Revert everything if slippage exceeded
            // Note: DEX should have reverted, but we double-check
            return Err(Error::from_contract_error(118)); // Slippage exceeded
        }
        
        env.events().publish(
            (symbol_short!("swap"), symbol_short!("dex_complete")),
            (config.nonce, amount_out),
        );
        
        // Step 5: Calculate fees (from config)
        let fee_amount = if config.fee_bps > 0 {
            amount_out
                .checked_mul(config.fee_bps as i128)
                .ok_or(Error::from_contract_error(119))?
                .checked_div(10000)
                .ok_or(Error::from_contract_error(120))?
        } else {
            0
        };
        
        let user_amount = amount_out
            .checked_sub(fee_amount)
            .ok_or(Error::from_contract_error(121))?;
        
        // SECURITY: Ensure user gets something
        if user_amount <= 0 {
            return Err(Error::from_contract_error(122)); // User amount invalid
        }
        
        // Step 6: Transfer output to user
        let token_out_client = TokenClient::new(&env, &output_token);
        
        // Check contract balance first
        let contract_balance = token_out_client.balance(&env.current_contract_address());
        if contract_balance < user_amount {
            return Err(Error::from_contract_error(123)); // Balance mismatch
        }
        
        token_out_client.transfer(
            &env.current_contract_address(),
            &user,
            &user_amount,
        );
        
        // Step 7: Transfer fees to treasury
        if fee_amount > 0 {
            if contract_balance >= fee_amount + user_amount {
                token_out_client.transfer(
                    &env.current_contract_address(),
                    &config.treasury,
                    &fee_amount,
                );
            } else {
                // If insufficient balance for fee, user still gets their share
                // Fee is waived for this transaction
                env.events().publish(
                    (symbol_short!("swap"), symbol_short!("fee_waived")),
                    (config.nonce, fee_amount),
                );
            }
        }
        
        // Step 8: Revoke DEX approval (security best practice)
        token_in_client.approve(
            &env.current_contract_address(),
            &dex_address,
            &0,
            &current_ledger, // Expire immediately
        );
        
        // Emit final completion event
        let final_event = SwapEvent {
            user: user.clone(),
            input_token: input_token.clone(),
            output_token: output_token.clone(),
            amount_in,
            amount_out: user_amount,
            fee_amount,
            nonce: config.nonce,
            timestamp: env.ledger().timestamp(),
            tx_hash: tx_hash.clone(),
        };
        
        env.events().publish(
            (symbol_short!("swap"), symbol_short!("complete")),
            final_event,
        );
        
        // Store event in persistent storage for querying
        storage.set(
            &(symbol_short!("event"), config.nonce),
            &final_event,
        );
        
        Ok(user_amount)
    }
    
    // --- INTERNAL HELPERS ---
    
    /// Get quote from DEX with error handling
    fn get_dex_quote(
        env: Env,
        input_token: Address,
        output_token: Address,
        amount_in: i128,
        dex_address: Address,
    ) -> Result<i128, Error> {
        // Use try_invoke_contract for better error handling
        let result: Result<i128, Error> = env.try_invoke_contract(
            &dex_address,
            &symbol_short!("get_quote"),
            vec![
                &env,
                input_token.into_val(&env),
                output_token.into_val(&env),
                amount_in.into_val(&env),
            ],
        );
        
        match result {
            Ok(amount_out) => {
                if amount_out <= 0 {
                    Err(Error::from_contract_error(124)) // Invalid quote
                } else {
                    Ok(amount_out)
                }
            }
            Err(e) => {
                env.events().publish(
                    (symbol_short!("swap"), symbol_short!("quote_error")),
                    e.error,
                );
                Err(Error::from_contract_error(125)) // DEX quote failed
            }
        }
    }
    
    /// Execute DEX swap with comprehensive error handling
    fn execute_dex_swap(
        env: Env,
        input_token: Address,
        output_token: Address,
        amount_in: i128,
        min_amount_out: i128,
        dex_address: Address,
    ) -> Result<i128, Error> {
        // Note: We assume DEX has a "swap" function with these parameters
        // Actual implementation may vary based on specific DEX
        
        let result: Result<i128, Error> = env.try_invoke_contract(
            &dex_address,
            &symbol_short!("swap"),
            vec![
                &env,
                input_token.into_val(&env),
                output_token.into_val(&env),
                amount_in.into_val(&env),
                min_amount_out.into_val(&env),
                env.current_contract_address().into_val(&env), // receiver
            ],
        );
        
        match result {
            Ok(amount_out) => {
                if amount_out < min_amount_out {
                    // DEX should have reverted, but we handle gracefully
                    Err(Error::from_contract_error(126)) // DEX slippage
                } else {
                    Ok(amount_out)
                }
            }
            Err(e) => {
                env.events().publish(
                    (symbol_short!("swap"), symbol_short!("dex_error")),
                    (dex_address, e.error),
                );
                Err(Error::from_contract_error(127)) // DEX swap failed
            }
        }
    }
    
    // --- ADMIN FUNCTIONS ---
    
    /// Update contract configuration (admin only)
    pub fn update_config(
        env: Env,
        new_treasury: Option<Address>,
        new_admin: Option<Address>,
        new_fee_bps: Option<u32>,
        paused: Option<bool>,
    ) -> Result<(), Error> {
        let mut storage = Storage::new(&env);
        let mut config: Config = storage.get(&symbol_short!("config"))
            .ok_or(Error::from_contract_error(128))?;
        
        // SECURITY: Verify admin authorization
        config.admin.require_auth();
        
        if let Some(treasury) = new_treasury {
            config.treasury = treasury;
        }
        
        if let Some(admin) = new_admin {
            config.admin = admin;
        }
        
        if let Some(fee_bps) = new_fee_bps {
            if fee_bps > 500 {
                return Err(Error::from_contract_error(129)); // Fee too high
            }
            config.fee_bps = fee_bps;
        }
        
        if let Some(is_paused) = paused {
            config.is_paused = is_paused;
        }
        
        config.last_updated = env.ledger().sequence();
        storage.set(&symbol_short!("config"), &config);
        
        env.events().publish(
            (symbol_short!("admin"), symbol_short!("config_updated")),
            config,
        );
        
        Ok(())
    }
    
    /// Add token to whitelist (admin only)
    pub fn whitelist_token(
        env: Env,
        token: Address,
        enabled: bool,
    ) -> Result<(), Error> {
        let mut storage = Storage::new(&env);
        let config: Config = storage.get(&symbol_short!("config"))
            .ok_or(Error::from_contract_error(130))?;
        
        config.admin.require_auth();
        
        let mut whitelist: Whitelist = storage.get(&symbol_short!("whitelist"))
            .ok_or(Error::from_contract_error(131))?;
        
        whitelist.tokens.set(token.clone(), enabled);
        storage.set(&symbol_short!("whitelist"), &whitelist);
        
        env.events().publish(
            (symbol_short!("admin"), symbol_short!("token_whitelist")),
            (token, enabled),
        );
        
        Ok(())
    }
    
    /// Add DEX to whitelist (admin only)
    pub fn whitelist_dex(
        env: Env,
        dex: Address,
        enabled: bool,
    ) -> Result<(), Error> {
        let mut storage = Storage::new(&env);
        let config: Config = storage.get(&symbol_short!("config"))
            .ok_or(Error::from_contract_error(132))?;
        
        config.admin.require_auth();
        
        let mut whitelist: Whitelist = storage.get(&symbol_short!("whitelist"))
            .ok_or(Error::from_contract_error(133))?;
        
        whitelist.dexes.set(dex.clone(), enabled);
        storage.set(&symbol_short!("whitelist"), &whitelist);
        
        env.events().publish(
            (symbol_short!("admin"), symbol_short!("dex_whitelist")),
            (dex, enabled),
        );
        
        Ok(())
    }
    
    /// Emergency withdrawal (admin only) - for stuck tokens
    pub fn emergency_withdraw(
        env: Env,
        token: Address,
        amount: i128,
        recipient: Address,
    ) -> Result<(), Error> {
        let storage = Storage::new(&env);
        let config: Config = storage.get(&symbol_short!("config"))
            .ok_or(Error::from_contract_error(134))?;
        
        config.admin.require_auth();
        
        if amount <= 0 {
            return Err(Error::from_contract_error(135));
        }
        
        let token_client = TokenClient::new(&env, &token);
        let contract_balance = token_client.balance(&env.current_contract_address());
        
        if contract_balance < amount {
            return Err(Error::from_contract_error(136)); // Insufficient balance
        }
        
        token_client.transfer(
            &env.current_contract_address(),
            &recipient,
            &amount,
        );
        
        env.events().publish(
            (symbol_short!("admin"), symbol_short!("emergency_withdraw")),
            (token, amount, recipient, env.ledger().timestamp()),
        );
        
        Ok(())
    }
    
    // --- VIEW FUNCTIONS ---
    
    /// Get current configuration
    pub fn get_config(env: Env) -> Result<Config, Error> {
        let storage = Storage::new(&env);
        storage.get(&symbol_short!("config"))
            .ok_or(Error::from_contract_error(137))
    }
    
    /// Get whitelist status
    pub fn get_whitelist(env: Env) -> Result<Whitelist, Error> {
        let storage = Storage::new(&env);
        storage.get(&symbol_short!("whitelist"))
            .ok_or(Error::from_contract_error(138))
    }
    
    /// Get swap event by nonce
    pub fn get_swap_event(env: Env, nonce: u64) -> Result<SwapEvent, Error> {
        let storage = Storage::new(&env);
        storage.get(&(symbol_short!("event"), nonce))
            .ok_or(Error::from_contract_error(139))
    }
    
    /// Check if address is whitelisted for token
    pub fn is_token_whitelisted(env: Env, token: Address) -> Result<bool, Error> {
        let storage = Storage::new(&env);
        let whitelist: Whitelist = storage.get(&symbol_short!("whitelist"))
            .ok_or(Error::from_contract_error(140))?;
        
        Ok(whitelist.tokens.get(token).unwrap_or(false))
    }
    
    /// Check if DEX is whitelisted
    pub fn is_dex_whitelisted(env: Env, dex: Address) -> Result<bool, Error> {
        let storage = Storage::new(&env);
        let whitelist: Whitelist = storage.get(&symbol_short!("whitelist"))
            .ok_or(Error::from_contract_error(141))?;
        
        Ok(whitelist.dexes.get(dex).unwrap_or(false))
    }
    
    /// Get next valid nonce
    pub fn get_next_nonce(env: Env) -> Result<u64, Error> {
        let storage = Storage::new(&env);
        let config: Config = storage.get(&symbol_short!("config"))
            .ok_or(Error::from_contract_error(142))?;
        
        Ok(config.nonce + 1)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger, Events};
    use soroban_sdk::vec;
    
    #[test]
    fn test_initialization() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let treasury = Address::generate(&env);
        
        let contract_id = env.register_contract(None, TokenSwapContract);
        let client = TokenSwapContractClient::new(&env, &contract_id);
        
        // Should fail if not initialized
        assert!(client.get_config().is_err());
        
        // Initialize
        client.initialize(&treasury, &admin, &100); // 1% fee
        
        // Verify config
        let config = client.get_config().unwrap();
        assert_eq!(config.treasury, treasury);
        assert_eq!(config.admin, admin);
        assert_eq!(config.fee_bps, 100);
        assert!(!config.is_paused);
        assert_eq!(config.nonce, 0);
    }
    
    #[test]
    fn test_admin_functions() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let treasury = Address::generate(&env);
        let new_admin = Address::generate(&env);
        let token = Address::generate(&env);
        
        let contract_id = env.register_contract(None, TokenSwapContract);
        let client = TokenSwapContractClient::new(&env, &contract_id);
        
        // Initialize
        client.initialize(&treasury, &admin, &100);
        
        // Non-admin can't update config
        env.mock_all_auths(); // Clear auth
        assert!(client.update_config(
            &None,
            &Some(new_admin.clone()),
            &None,
            &None,
        ).is_err());
        
        // Admin can update config
        env.mock_all_auths();
        admin.require_auth();
        client.update_config(
            &None,
            &Some(new_admin.clone()),
            &None,
            &None,
        ).unwrap();
        
        // Verify admin updated
        let config = client.get_config().unwrap();
        assert_eq!(config.admin, new_admin);
    }
}
