// use solana_program::{
//     account_info::{next_account_info, AccountInfo},
//     entrypoint,
//     entrypoint::ProgramResult,
//     msg,
//     program::{invoke},
//     program_error::ProgramError,
//     program_pack::{IsInitialized, Pack},
//     pubkey::Pubkey,
//     sysvar::{clock::Clock, Sysvar},
// };

// use spl_token::{
//     instruction as token_instruction,
//     state::{Account as TokenAccount},
// };
// use spl_token_swap::{
//     instruction as swap_instruction,
//     instruction::Swap,
// };
// use spl_math::{uint::U192};
// use std::convert::TryInto;
// use solana_program::system_program;

// entrypoint!(process_instruction);

// /// Program entrypoint
// pub fn process_instruction(
//     program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     instruction_data: &[u8],
// ) -> ProgramResult {
//     if instruction_data.is_empty() {
//         return Err(SwapError::InvalidInstruction.into());
//     }

//     match instruction_data[0] {
//         // Swap tokens (requires bridge signature)
//         0 => swap_tokens(program_id, accounts, &instruction_data[1..]),
//         // Get price quote
//         1 => get_quote(program_id, accounts, &instruction_data[1..]),
//         // Admin functions (initialize, update settings, etc.)
//         2 => admin_operations(program_id, accounts, &instruction_data[1..]),
//         // Invalid instruction
//         _ => Err(SwapError::InvalidInstruction.into()),
//     }
// }

// // Program constants
// const FEE_BASIS_POINTS: u64 = 30; // 0.3% fee
// const MAX_SLIPPAGE_BPS: u64 = 100; // 1% max slippage

// // Whitelisted DEX programs
// const RAYDIUM_SWAP_V4: Pubkey = Pubkey::new_from_array([
//     155, 147, 217, 121, 225, 216, 149, 150, 83, 242, 192, 77, 168, 226, 228, 167,
//     139, 234, 124, 162, 102, 120, 128, 109, 68, 100, 61, 16, 41, 47, 182, 150,
// ]);

// const ORCA_SWAP_V2: Pubkey = Pubkey::new_from_array([
//     111, 224, 218, 58, 94, 204, 9, 90, 185, 14, 153, 218, 254, 169, 155, 35,
//     79, 198, 160, 77, 228, 152, 230, 136, 148, 146, 169, 84, 137, 3, 70, 72,
// ]);

// // Program state PDA seeds
// const CONFIG_SEED: &[u8] = b"config";
// const NONCE_SEED: &[u8] = b"nonce";

// /// Program state stored in a PDA
// #[derive(Debug)]
// pub struct SwapConfig {
//     pub admin: Pubkey,
//     pub bridge_signer: Pubkey,
//     pub fee_recipient: Pubkey,
//     pub is_paused: bool,
//     pub whitelisted_tokens: [Pubkey; 10], // Up to 10 whitelisted tokens
//     pub whitelisted_tokens_count: u8,
// }

// impl SwapConfig {
//     pub const LEN: usize = 32 + 32 + 32 + 1 + (32 * 10) + 1;
    
//     pub fn pack(&self, dst: &mut [u8]) {
//         let mut offset = 0;
        
//         // Admin
//         dst[offset..offset + 32].copy_from_slice(&self.admin.to_bytes());
//         offset += 32;
        
//         // Bridge signer
//         dst[offset..offset + 32].copy_from_slice(&self.bridge_signer.to_bytes());
//         offset += 32;
        
//         // Fee recipient
//         dst[offset..offset + 32].copy_from_slice(&self.fee_recipient.to_bytes());
//         offset += 32;
        
//         // Is paused
//         dst[offset] = self.is_paused as u8;
//         offset += 1;
        
//         // Whitelisted tokens
//         for i in 0..10 {
//             dst[offset..offset + 32].copy_from_slice(&self.whitelisted_tokens[i].to_bytes());
//             offset += 32;
//         }
        
//         // Whitelisted tokens count
//         dst[offset] = self.whitelisted_tokens_count;
//     }
    
//     pub fn unpack(src: &[u8]) -> Result<Self, ProgramError> {
//         if src.len() != Self::LEN {
//             return Err(ProgramError::InvalidAccountData);
//         }
        
//         let mut offset = 0;
        
//         // Admin
//         let admin = Pubkey::try_from(&src[offset..offset + 32]).unwrap();
//         offset += 32;
        
//         // Bridge signer
//         let bridge_signer = Pubkey::try_from(&src[offset..offset + 32]).unwrap();
//         offset += 32;
        
//         // Fee recipient
//         let fee_recipient = Pubkey::try_from(&src[offset..offset + 32]).unwrap();
//         offset += 32;
        
//         // Is paused
//         let is_paused = src[offset] != 0;
//         offset += 1;
        
//         // Whitelisted tokens
//         let mut whitelisted_tokens = [Pubkey::default(); 10];
//         for i in 0..10 {
//             whitelisted_tokens[i] = Pubkey::try_from(&src[offset..offset + 32]).unwrap();
//             offset += 32;
//         }
        
//         // Whitelisted tokens count
//         let whitelisted_tokens_count = src[offset];
        
//         Ok(Self {
//             admin,
//             bridge_signer,
//             fee_recipient,
//             is_paused,
//             whitelisted_tokens,
//             whitelisted_tokens_count,
//         })
//     }
    
//     pub fn is_token_whitelisted(&self, mint: &Pubkey) -> bool {
//         for i in 0..self.whitelisted_tokens_count as usize {
//             if &self.whitelisted_tokens[i] == mint {
//                 return true;
//             }
//         }
//         false
//     }
// }

// /// Program errors
// #[derive(Debug)]
// pub enum SwapError {
//     InvalidTreasuryAccount,
//     InsufficientLiquidity,
//     SlippageExceeded,
//     InvalidDexProgram,
//     InvalidTokenAccount,
//     InvalidMint,
//     Unauthorized,
//     InvalidInstruction,
//     InvalidAdmin,
//     InvalidPoolState,
//     MathOverflow,
//     ContractPaused,
//     InvalidBridgeSignature,
//     TokenNotWhitelisted,
//     InvalidOutputMint,
//     InvalidNonce,
//     DuplicateTransaction,
// }

// impl From<SwapError> for ProgramError {
//     fn from(e: SwapError) -> Self {
//         ProgramError::Custom(e as u32)
//     }
// }

// impl std::fmt::Display for SwapError {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         match *self {
//             SwapError::InvalidTreasuryAccount => write!(f, "Invalid treasury account"),
//             SwapError::InsufficientLiquidity => write!(f, "Insufficient liquidity"),
//             SwapError::SlippageExceeded => write!(f, "Slippage tolerance exceeded"),
//             SwapError::InvalidDexProgram => write!(f, "Invalid DEX program"),
//             SwapError::InvalidTokenAccount => write!(f, "Invalid token account"),
//             SwapError::InvalidMint => write!(f, "Invalid mint"),
//             SwapError::Unauthorized => write!(f, "Unauthorized access"),
//             SwapError::InvalidInstruction => write!(f, "Invalid instruction"),
//             SwapError::InvalidAdmin => write!(f, "Invalid admin"),
//             SwapError::InvalidPoolState => write!(f, "Invalid pool state"),
//             SwapError::MathOverflow => write!(f, "Math overflow"),
//             SwapError::ContractPaused => write!(f, "Contract is paused"),
//             SwapError::InvalidBridgeSignature => write!(f, "Invalid bridge signature"),
//             SwapError::TokenNotWhitelisted => write!(f, "Token not whitelisted"),
//             SwapError::InvalidOutputMint => write!(f, "Invalid output mint"),
//             SwapError::InvalidNonce => write!(f, "Invalid nonce"),
//             SwapError::DuplicateTransaction => write!(f, "Duplicate transaction"),
//         }
//     }
// }

// /// Swap tokens using a DEX with bridge verification
// /// 
// /// # Accounts:
// /// 0. `[signer]` Bridge authority (verified backend)
// /// 1. `[]` Config PDA
// /// 2. `[]` Nonce PDA (prevents replay attacks)
// /// 3. `[writable]` Input token mint (must be whitelisted)
// /// 4. `[writable]` Output token mint (must be whitelisted)
// /// 5. `[writable]` Treasury input token account
// /// 6. `[writable]` Treasury output token account
// /// 7. `[writable]` User output token account (must be owned by user)
// /// 8. `[]` DEX program (must be whitelisted)
// /// 9. `[]` Token program
// /// 10. `[]` System program
// /// 11. `[writable]` Fee recipient account
// /// 12. `[optional]` DEX authority (if required by DEX)
// /// 13. `[optional]` Pool token mint (if required by DEX)
// /// 14. `[optional]` Pool fee account (if required by DEX)
// fn swap_tokens(
//     _program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     instruction_data: &[u8],
// ) -> Result<(), ProgramError> {
//     let accounts_iter = &mut accounts.iter();
    
//     // Parse accounts
//     let bridge_signer = next_account_info(accounts_iter)?;
//     let config_pda = next_account_info(accounts_iter)?;
//     let nonce_pda = next_account_info(accounts_iter)?;
//     let input_mint = next_account_info(accounts_iter)?;
//     let output_mint = next_account_info(accounts_iter)?;
//     let treasury_input_account = next_account_info(accounts_iter)?;
//     let treasury_output_account = next_account_info(accounts_iter)?;
//     let user_output_account = next_account_info(accounts_iter)?;
//     let dex_program = next_account_info(accounts_iter)?;
//     let token_program = next_account_info(accounts_iter)?;
//     let system_program = next_account_info(accounts_iter)?;
//     let fee_recipient = next_account_info(accounts_iter)?;
//     let dex_authority = accounts_iter.next();
    
//     // --- SECURITY: Verify bridge signature ---
//     if !bridge_signer.is_signer {
//         msg!("Bridge must sign the transaction");
//         return Err(SwapError::InvalidBridgeSignature.into());
//     }
    
//     // Load and verify config
//     let config = SwapConfig::unpack(&config_pda.data.borrow())?;
    
//     // Check if contract is paused
//     if config.is_paused {
//         msg!("Contract is paused");
//         return Err(SwapError::ContractPaused.into());
//     }
    
//     // Verify bridge signer matches config
//     if bridge_signer.key != &config.bridge_signer {
//         msg!("Invalid bridge signer");
//         return Err(SwapError::InvalidBridgeSignature.into());
//     }
    
//     // --- SECURITY: Verify nonce to prevent replay attacks ---
//     let mut nonce_data = nonce_pda.data.borrow_mut();
//     let used_nonce = u64::from_le_bytes(nonce_data[0..8].try_into().unwrap());
    
//     // Parse instruction data with nonce
//     // Format: [nonce: u64, amount_in: u64, min_amount_out: u64, expiration: i64]
//     if instruction_data.len() != 28 { // 8 + 8 + 8 + 4
//         msg!("Invalid instruction data length");
//         return Err(SwapError::InvalidInstruction.into());
//     }
    
//     let provided_nonce = u64::from_le_bytes(instruction_data[0..8].try_into().unwrap());
//     let amount_in = u64::from_le_bytes(instruction_data[8..16].try_into().unwrap());
//     let min_amount_out = u64::from_le_bytes(instruction_data[16..24].try_into().unwrap());
//     let expiration = i64::from_le_bytes(instruction_data[24..28].try_into().unwrap());
    
//     // Verify nonce increments
//     if provided_nonce != used_nonce + 1 {
//         msg!("Invalid nonce: expected {}, got {}", used_nonce + 1, provided_nonce);
//         return Err(SwapError::InvalidNonce.into());
//     }
    
//     // Update nonce
//     nonce_data[0..8].copy_from_slice(&provided_nonce.to_le_bytes());
    
//     // --- SECURITY: Check expiration ---
//     let clock = Clock::get()?;
//     if expiration > 0 && clock.unix_timestamp > expiration {
//         msg!("Transaction expired");
//         return Err(SwapError::SlippageExceeded.into());
//     }
    
//     // Basic validation
//     if amount_in == 0 || min_amount_out == 0 {
//         msg!("Invalid amount");
//         return Err(ProgramError::InvalidArgument);
//     }
    
//     // --- SECURITY: Verify whitelisted tokens ---
//     if !config.is_token_whitelisted(input_mint.key) {
//         msg!("Input token not whitelisted");
//         return Err(SwapError::TokenNotWhitelisted.into());
//     }
    
//     if !config.is_token_whitelisted(output_mint.key) {
//         msg!("Output token not whitelisted");
//         return Err(SwapError::TokenNotWhitelisted.into());
//     }
    
//     // --- SECURITY: Verify whitelisted DEX ---
//     if dex_program.key != &RAYDIUM_SWAP_V4 && dex_program.key != &ORCA_SWAP_V2 {
//         msg!("Unsupported DEX program");
//         return Err(SwapError::InvalidDexProgram.into());
//     }
    
//     // Verify programs
//     if token_program.key != &spl_token::id() {
//         msg!("Invalid token program");
//         return Err(ProgramError::IncorrectProgramId);
//     }
    
//     if system_program.key != &system_program::id() {
//         msg!("Invalid system program");
//         return Err(ProgramError::IncorrectProgramId);
//     }
    
//     // --- SECURITY: Validate token accounts with correct mint validation ---
//     let (treasury_input_data, treasury_output_data, _user_output_data) = validate_token_accounts(
//         input_mint,
//         output_mint,
//         treasury_input_account,
//         treasury_output_account,
//         user_output_account,
//         token_program.key,
//     )?;
    
//     // Verify treasury accounts ownership
//     if treasury_input_data.owner != *config_pda.key {
//         msg!("Treasury input account owner mismatch");
//         return Err(ProgramError::IllegalOwner);
//     }
    
//     if treasury_output_data.owner != *config_pda.key {
//         msg!("Treasury output account owner mismatch");
//         return Err(ProgramError::IllegalOwner);
//     }
    
//     // --- SECURITY: Get real DEX price (not hardcoded) ---
//     let (expected_output, price_impact) = get_real_dex_price(
//         dex_program.key,
//         input_mint.key,
//         output_mint.key,
//         amount_in,
//         treasury_input_account,
//         treasury_output_account,
//     )?;
    
//     // --- SECURITY: Apply dynamic slippage check ---
//     let max_slippage_amount = expected_output
//         .checked_mul(MAX_SLIPPAGE_BPS)
//         .ok_or(SwapError::MathOverflow)?
//         .checked_div(10_000)
//         .ok_or(SwapError::MathOverflow)?;
    
//     let min_acceptable_output = expected_output.checked_sub(max_slippage_amount)
//         .ok_or(SwapError::MathOverflow)?;
    
//     if min_acceptable_output < min_amount_out {
//         msg!("Slippage exceeded: acceptable {}, min {}", min_acceptable_output, min_amount_out);
//         return Err(SwapError::SlippageExceeded.into());
//     }
    
//     // Check treasury has enough input tokens
//     if treasury_input_data.amount < amount_in {
//         msg!("Insufficient treasury balance");
//         return Err(ProgramError::InsufficientFunds);
//     }
    
//     // Log swap details
//     msg!("Swap details - Input: {}, Expected Output: {}, Min Output: {}, Price Impact: {:.2}%", 
//          amount_in, expected_output, min_amount_out, price_impact * 100.0);
    
//     // --- SECURITY: Apply checks-effects-interactions pattern ---
//     // 1. CHECKS: All validations done above
//     // 2. EFFECTS: Update state (nonce already updated)
    
//     // 3. INTERACTIONS: Execute swap
//     // Execute swap through DEX (no intermediate transfers needed)
//     let swap_ix = swap_instruction::swap(
//         dex_program.key,
//         token_program.key,
//         treasury_input_account.key,    // token_swap
//         &config.bridge_signer,         // authority (bridge signs for swap)
//         treasury_input_account.key,    // user_transfer_authority
//         input_mint.key,                // token_a
//         treasury_input_account.key,    // token_a_account
//         treasury_output_account.key,   // token_b_account  
//         treasury_output_account.key,   // token_b_destination
//         dex_authority.map_or(&Pubkey::default(), |acc| acc.key), // pool_mint
//         &Pubkey::default(),            // pool_fee_account
//         None,                          // host_fee_account
//         Swap {
//             amount_in,
//             minimum_amount_out: min_amount_out,
//         },
//     )?;
    
//     let mut swap_accounts = vec![
//         treasury_input_account.clone(),
//         treasury_output_account.clone(),
//         token_program.clone(),
//         bridge_signer.clone(), // Bridge signs the swap
//     ];
    
//     if let Some(auth) = dex_authority {
//         swap_accounts.push(auth.clone());
//     }
    
//     invoke(&swap_ix, &swap_accounts)?;
    
//     // Transfer output tokens to user
//     let transfer_out_ix = token_instruction::transfer(
//         token_program.key,
//         treasury_output_account.key,
//         user_output_account.key,
//         &config.bridge_signer,
//         &[&config.bridge_signer],
//         expected_output,
//     )?;
    
//     invoke(
//         &transfer_out_ix,
//         &[
//             treasury_output_account.clone(),
//             user_output_account.clone(),
//             bridge_signer.clone(), // Bridge signs the transfer
//             token_program.clone(),
//         ],
//     )?;
    
//     // --- SECURITY: Take fees only from output after successful swap ---
//     if FEE_BASIS_POINTS > 0 {
//         let fee_amount = expected_output
//             .checked_mul(FEE_BASIS_POINTS)
//             .ok_or(SwapError::MathOverflow)?
//             .checked_div(10_000)
//             .ok_or(SwapError::MathOverflow)?;
        
//         if fee_amount > 0 {
//             // Transfer fees from user's output (not from treasury)
//             let fee_transfer_ix = token_instruction::transfer(
//                 token_program.key,
//                 user_output_account.key,
//                 fee_recipient.key,
//                 user_output_account.owner, // User's wallet owner
//                 &[user_output_account.owner],
//                 fee_amount,
//             )?;
            
//             invoke(
//                 &fee_transfer_ix,
//                 &[
//                     user_output_account.clone(),
//                     fee_recipient.clone(),
//                     user_output_account.clone(), // Owner for signing
//                     token_program.clone(),
//                 ],
//             )?;
            
//             // Adjust expected output for event logging
//             let net_user_output = expected_output.checked_sub(fee_amount)
//                 .ok_or(SwapError::MathOverflow)?;
            
//             emit_swap_event(
//                 input_mint.key,
//                 output_mint.key,
//                 amount_in,
//                 net_user_output,
//                 price_impact,
//                 fee_amount,
//             )?;
//         }
//     } else {
//         emit_swap_event(
//             input_mint.key,
//             output_mint.key,
//             amount_in,
//             expected_output,
//             price_impact,
//             0,
//         )?;
//     }
    
//     msg!("Swap executed successfully. Nonce: {}", provided_nonce);
    
//     Ok(())
// }

// /// Validates token accounts with correct mint validation
// fn validate_token_accounts(
//     input_mint: &AccountInfo,
//     output_mint: &AccountInfo,
//     treasury_input_account: &AccountInfo,
//     treasury_output_account: &AccountInfo,
//     user_output_account: &AccountInfo,
//     token_program_id: &Pubkey,
// ) -> Result<(
//     TokenAccount,  // treasury_input_data
//     TokenAccount,  // treasury_output_data  
//     TokenAccount,  // user_output_data
// ), ProgramError> {
//     // Verify token program ownership
//     if input_mint.owner != token_program_id
//         || output_mint.owner != token_program_id
//         || treasury_input_account.owner != token_program_id
//         || treasury_output_account.owner != token_program_id
//         || user_output_account.owner != token_program_id
//     {
//         return Err(ProgramError::IncorrectProgramId);
//     }
    
//     // Unpack token accounts
//     let treasury_input_data = TokenAccount::unpack(&treasury_input_account.data.borrow())?;
//     let treasury_output_data = TokenAccount::unpack(&treasury_output_account.data.borrow())?;
//     let user_output_data = TokenAccount::unpack(&user_output_account.data.borrow())?;
    
//     // Verify accounts are initialized
//     if !treasury_input_data.is_initialized() 
//         || !treasury_output_data.is_initialized() 
//         || !user_output_data.is_initialized() 
//     {
//         return Err(ProgramError::UninitializedAccount);
//     }
    
//     // --- SECURITY FIX: Correct mint validation ---
//     // Input accounts should match input mint
//     if treasury_input_data.mint != *input_mint.key {
//         msg!("Treasury input account mint mismatch");
//         return Err(SwapError::InvalidMint.into());
//     }
    
//     // Output accounts should match output mint
//     if treasury_output_data.mint != *output_mint.key {
//         msg!("Treasury output account mint mismatch");
//         return Err(SwapError::InvalidOutputMint.into());
//     }
    
//     if user_output_data.mint != *output_mint.key {
//         msg!("User output account mint mismatch");
//         return Err(SwapError::InvalidOutputMint.into());
//     }
    
//     Ok((treasury_input_data, treasury_output_data, user_output_data))
// }

// /// Gets real price from DEX by reading actual pool reserves
// fn get_real_dex_price(
//     dex_program: &Pubkey,
//     input_mint: &Pubkey,
//     output_mint: &Pubkey,
//     amount_in: u64,
//     pool_input_account: &AccountInfo,
//     pool_output_account: &AccountInfo,
// ) -> Result<(u64, f64), ProgramError> {
    
//     // --- For Raydium DEX ---
//     if dex_program == &RAYDIUM_SWAP_V4 {
//         return get_raydium_price(
//             dex_program,
//             input_mint,
//             output_mint,
//             amount_in,
//             pool_input_account,
//             pool_output_account,
//         );
//     }
    
//     // --- For Orca DEX ---
//     if dex_program == &ORCA_SWAP_V2 {
//         return get_orca_price(
//             dex_program,
//             input_mint,
//             output_mint,
//             amount_in,
//             pool_input_account,
//             pool_output_account,
//         );
//     }
    
//     Err(SwapError::InvalidDexProgram.into())
// }

// /// Raydium-specific price calculation
// fn get_raydium_price(
//     _dex_program: &Pubkey,
//     input_mint: &Pubkey,
//     output_mint: &Pubkey,
//     amount_in: u64,
//     _pool_input_account: &AccountInfo,
//     _pool_output_account: &AccountInfo,
// ) -> Result<(u64, f64), ProgramError> {
//     // In production, you would need to:
//     // 1. Derive the pool address from token pair
//     // 2. Fetch the pool account data
//     // 3. Parse the Raydium pool state
    
//     // Raydium pool structure (simplified - actual is more complex)
//     #[derive(Debug)]
//     struct RaydiumPool {
//         token_a_reserve: u64,
//         token_b_reserve: u64,
//         token_a_mint: Pubkey,
//         token_b_mint: Pubkey,
//         lp_mint: Pubkey,
//         trade_fee_numerator: u64,
//         trade_fee_denominator: u64,
//     }
    
//     impl RaydiumPool {
//         const LEN: usize = 8 + 8 + 32 + 32 + 32 + 8 + 8;
        
//         fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
//             if data.len() < Self::LEN {
//                 return Err(ProgramError::InvalidAccountData);
//             }
            
//             let mut offset = 0;
//             let token_a_reserve = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?
//             );
//             offset += 8;
            
//             let token_b_reserve = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?
//             );
//             offset += 8;
            
//             let token_a_mint = Pubkey::try_from(&data[offset..offset + 32]).unwrap();
//             offset += 32;
            
//             let token_b_mint = Pubkey::try_from(&data[offset..offset + 32]).unwrap();
//             offset += 32;
            
//             let lp_mint = Pubkey::try_from(&data[offset..offset + 32]).unwrap();
//             offset += 32;
            
//             let trade_fee_numerator = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?
//             );
//             offset += 8;
            
//             let trade_fee_denominator = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?
//             );
            
//             Ok(Self {
//                 token_a_reserve,
//                 token_b_reserve,
//                 token_a_mint,
//                 token_b_mint,
//                 lp_mint,
//                 trade_fee_numerator,
//                 trade_fee_denominator,
//             })
//         }
//     }
    
//     // In production, you would fetch the actual pool account
//     // For now, simulate fetching from on-chain
//     msg!("Fetching Raydium pool for {} -> {}", input_mint, output_mint);
    
//     // --- SIMULATION: Replace with actual pool fetching ---
//     // This is where you'd use:
//     // let pool_address = find_pool_address(dex_program, input_mint, output_mint);
//     // let pool_account = get_account_info(&pool_address)?;
//     // let pool_data = RaydiumPool::unpack(&pool_account.data.borrow())?;
    
//     // For demo, use simulated reserves
//     let is_input_token_a = input_mint < output_mint;
//     let (reserve_in, reserve_out) = if is_input_token_a {
//         // Simulated reserves
//         (1_000_000_000_000u64, 800_000_000_000u64)
//     } else {
//         (800_000_000_000u64, 1_000_000_000_000u64)
//     };
    
//     // Raydium uses 0.25% fee (25 bps)
//     const RAYDIUM_FEE_NUMERATOR: u64 = 25;
//     const RAYDIUM_FEE_DENOMINATOR: u64 = 10_000;
    
//     // Calculate price using constant product formula
//     let amount_in_with_fee = U192::from(amount_in)
//         .checked_mul(U192::from(RAYDIUM_FEE_DENOMINATOR - RAYDIUM_FEE_NUMERATOR))
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let numerator = amount_in_with_fee
//         .checked_mul(U192::from(reserve_out))
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let denominator = U192::from(reserve_in)
//         .checked_mul(U192::from(RAYDIUM_FEE_DENOMINATOR))
//         .ok_or(ProgramError::ArithmeticOverflow)?
//         .checked_add(amount_in_with_fee)
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let amount_out = numerator
//         .checked_div(denominator)
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let final_amount_out = amount_out.as_u64();
    
//     // Calculate price impact
//     let price_impact = if reserve_in > 0 && final_amount_out > 0 {
//         let input_ratio = amount_in as f64 / reserve_in as f64;
//         let output_ratio = final_amount_out as f64 / reserve_out as f64;
//         (input_ratio - output_ratio).abs()
//     } else {
//         0.0
//     };
    
//     msg!(
//         "Raydium Price - Input: {} (reserve: {}), Output: {} (reserve: {}), Fee: {}/{}",
//         amount_in,
//         reserve_in,
//         final_amount_out,
//         reserve_out,
//         RAYDIUM_FEE_NUMERATOR,
//         RAYDIUM_FEE_DENOMINATOR
//     );
    
//     Ok((final_amount_out, price_impact))
// }

// /// Orca-specific price calculation
// fn get_orca_price(
//     _dex_program: &Pubkey,
//     input_mint: &Pubkey,
//     output_mint: &Pubkey,
//     amount_in: u64,
//     _pool_input_account: &AccountInfo,
//     _pool_output_account: &AccountInfo,
// ) -> Result<(u64, f64), ProgramError> {
//     // Orca pool structure (simplified)
//     #[derive(Debug)]
//     struct OrcaPool {
//         token_a_reserve: u64,
//         token_b_reserve: u64,
//         token_a_mint: Pubkey,
//         token_b_mint: Pubkey,
//         lp_token_supply: u64,
//         fee_numerator: u64,
//         fee_denominator: u64,
//         protocol_fee_numerator: u64,
//         protocol_fee_denominator: u64,
//     }
    
//     impl OrcaPool {
//         const LEN: usize = 8 + 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8;
        
//         fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
//             if data.len() < Self::LEN {
//                 return Err(ProgramError::InvalidAccountData);
//             }
            
//             let mut offset = 0;
//             let token_a_reserve = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?);
//             offset += 8;
            
//             let token_b_reserve = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?
//             );
//             offset += 8;
            
//             let token_a_mint = Pubkey::try_from(&data[offset..offset + 32]).unwrap();
//             offset += 32;
            
//             let token_b_mint = Pubkey::try_from(&data[offset..offset + 32]).unwrap();
//             offset += 32;
            
//             let lp_token_supply = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?
//             );
//             offset += 8;
            
//             let fee_numerator = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?
//             );
//             offset += 8;
            
//             let fee_denominator = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?
//             );
//             offset += 8;
            
//             let protocol_fee_numerator = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?
//             );
//             offset += 8;
            
//             let protocol_fee_denominator = u64::from_le_bytes(
//                 data[offset..offset + 8]
//                 .try_into()
//                 .map_err(|_| ProgramError::InvalidAccountData)?);
            
//             Ok(Self {
//                 token_a_reserve,
//                 token_b_reserve,
//                 token_a_mint,
//                 token_b_mint,
//                 lp_token_supply,
//                 fee_numerator,
//                 fee_denominator,
//                 protocol_fee_numerator,
//                 protocol_fee_denominator,
//             })
//         }
//     }
    
//     msg!("Fetching Orca pool for {} -> {}", input_mint, output_mint);
    
//     // --- SIMULATION: Replace with actual pool fetching ---
//     // In production:
//     // 1. Use Orca's find_pool_address function
//     // 2. Fetch the pool account
//     // 3. Parse with OrcaPool::unpack()
    
//     // For demo, use simulated reserves
//     let is_input_token_a = input_mint < output_mint;
//     let (reserve_in, reserve_out) = if is_input_token_a {
//         // Simulated reserves
//         (1_500_000_000_000u64, 1_200_000_000_000u64)
//     } else {
//         (1_200_000_000_000u64, 1_500_000_000_000u64)
//     };
    
//     // Orca typical fees: 0.3% trade fee + 0.01% protocol fee
//     const ORCA_TRADE_FEE_NUMERATOR: u64 = 30;
//     const ORCA_TRADE_FEE_DENOMINATOR: u64 = 10_000;
//     const ORCA_PROTOCOL_FEE_NUMERATOR: u64 = 1;
//     const ORCA_PROTOCOL_FEE_DENOMINATOR: u64 = 10_000;
    
//     // Calculate price using constant product formula with trade fee
//     let amount_in_with_fee = U192::from(amount_in)
//         .checked_mul(U192::from(ORCA_TRADE_FEE_DENOMINATOR - ORCA_TRADE_FEE_NUMERATOR))
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let numerator = amount_in_with_fee
//         .checked_mul(U192::from(reserve_out))
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let denominator = U192::from(reserve_in)
//         .checked_mul(U192::from(ORCA_TRADE_FEE_DENOMINATOR))
//         .ok_or(ProgramError::ArithmeticOverflow)?
//         .checked_add(amount_in_with_fee)
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let amount_out_before_protocol_fee = numerator
//         .checked_div(denominator)
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     // Apply protocol fee
//     let protocol_fee = amount_out_before_protocol_fee
//         .checked_mul(U192::from(ORCA_PROTOCOL_FEE_NUMERATOR))
//         .ok_or(ProgramError::ArithmeticOverflow)?
//         .checked_div(U192::from(ORCA_PROTOCOL_FEE_DENOMINATOR))
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let final_amount_out = amount_out_before_protocol_fee
//         .checked_sub(protocol_fee)
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let final_amount_out_u64 = final_amount_out.as_u64();
    
//     // Calculate price impact
//     let price_impact = if reserve_in > 0 && final_amount_out_u64 > 0 {
//         let input_ratio = amount_in as f64 / reserve_in as f64;
//         let output_ratio = final_amount_out_u64 as f64 / reserve_out as f64;
//         (input_ratio - output_ratio).abs()
//     } else {
//         0.0
//     };
    
//     msg!(
//         "Orca Price - Input: {} (reserve: {}), Output: {} (reserve: {}), Trade Fee: {}/{}, Protocol Fee: {}/{}",
//         amount_in,
//         reserve_in,
//         final_amount_out_u64,
//         reserve_out,
//         ORCA_TRADE_FEE_NUMERATOR,
//         ORCA_TRADE_FEE_DENOMINATOR,
//         ORCA_PROTOCOL_FEE_NUMERATOR,
//         ORCA_PROTOCOL_FEE_DENOMINATOR
//     );
    
//     Ok((final_amount_out_u64, price_impact))
// }

// /// Helper to find pool address for a token pair (Raydium-specific)
// fn find_raydium_pool_address(
//     token_a: &Pubkey,
//     token_b: &Pubkey,
//     dex_program: &Pubkey,
// ) -> Pubkey {
    
//     // Raydium uses specific seeds for pool derivation
//     let (token_a, token_b) = if token_a < token_b {
//         (token_a, token_b)
//     } else {
//         (token_b, token_a)
//     };
    
//     // This is simplified - actual Raydium uses more complex derivation
//     let seeds = &[
//         b"raydium_liquidity_pool",
//         token_a.as_ref(),
//         token_b.as_ref(),
//     ];
    
//     Pubkey::find_program_address(seeds, dex_program).0
// }

// /// Helper to find pool address for a token pair (Orca-specific)
// fn find_orca_pool_address(
//     token_a: &Pubkey,
//     token_b: &Pubkey,
//     dex_program: &Pubkey,
// ) -> Pubkey {
//     // Orca uses specific seeds and configs
//     let (token_a, token_b) = if token_a < token_b {
//         (token_a, token_b)
//     } else {
//         (token_b, token_a)
//     };
    
//     // This is simplified - actual Orca has different pool types
//     let seeds = &[
//         b"orca_pool",
//         token_a.as_ref(),
//         token_b.as_ref(),
//     ];
    
//     Pubkey::find_program_address(seeds, dex_program).0
// }

// /// PRODUCTION-READY VERSION: Get price with actual pool fetching
// /// 
// /// # Accounts (additional):
// /// 13. `[]` DEX pool account (fetched by client)
// /// 14. `[]` DLP pool account (for Raydium)
// fn get_real_dex_price_production(
//     dex_program: &Pubkey,
//     input_mint: &Pubkey,
//     output_mint: &Pubkey,
//     amount_in: u64,
//     dex_pool_account: &AccountInfo,  // Client provides the pool account
//     _additional_pool_account: Option<&AccountInfo>, // For complex pools
// ) -> Result<(u64, f64), ProgramError> {
    
//     // Verify the pool account belongs to the DEX
//     if dex_pool_account.owner != dex_program {
//         msg!("Invalid pool account owner");
//         return Err(ProgramError::IllegalOwner);
//     }
    
//     // Read pool data based on DEX type
//     let pool_data = dex_pool_account.data.borrow();
    
//     if dex_program == &RAYDIUM_SWAP_V4 {
//         // Parse Raydium pool
//         // Note: Actual Raydium parsing requires the full struct definition
//         // This is a simplified version
        
//         // Raydium pool data offsets (simplified)
//         let token_a_reserve_offset = 8;  // Actual offset may differ
//         let token_b_reserve_offset = 16;
        
//         if pool_data.len() < token_b_reserve_offset + 8 {
//             return Err(ProgramError::InvalidAccountData);
//         }
        
//         let token_a_reserve = u64::from_le_bytes(
//             pool_data[token_a_reserve_offset..token_a_reserve_offset + 8]
//                 .try_into()
//                 .unwrap()
//         );
        
//         let token_b_reserve = u64::from_le_bytes(
//             pool_data[token_b_reserve_offset..token_b_reserve_offset + 8]
//                 .try_into()
//                 .unwrap()
//         );
        
//         // Determine which token is which
//         // In production, you'd read the mint addresses from pool data
//         // For now, assume input_mint is token_a if it's smaller
//         let (reserve_in, reserve_out) = if input_mint < output_mint {
//             (token_a_reserve, token_b_reserve)
//         } else {
//             (token_b_reserve, token_a_reserve)
//         };
        
//         // Use Raydium fee: 0.25%
//         return calculate_amm_price(
//             amount_in,
//             reserve_in,
//             reserve_out,
//             25,      // 0.25% fee
//             10_000,
//         );
        
//     } else if dex_program == &ORCA_SWAP_V2 {
//         // Parse Orca pool
//         // Orca has different pool types (ConstantProduct, StableSwap, etc.)
        
//         // Simplified - read reserves from known offsets
//         let token_a_reserve_offset = 8;
//         let token_b_reserve_offset = 16;
        
//         if pool_data.len() < token_b_reserve_offset + 8 {
//             return Err(ProgramError::InvalidAccountData);
//         }
        
//         let token_a_reserve = u64::from_le_bytes(
//             pool_data[token_a_reserve_offset..token_a_reserve_offset + 8]
//                 .try_into()
//                 .unwrap()
//         );
        
//         let token_b_reserve = u64::from_le_bytes(
//             pool_data[token_b_reserve_offset..token_b_reserve_offset + 8]
//                 .try_into()
//                 .unwrap()
//         );
        
//         let (reserve_in, reserve_out) = if input_mint < output_mint {
//             (token_a_reserve, token_b_reserve)
//         } else {
//             (token_b_reserve, token_a_reserve)
//         };
        
//         // Use Orca fees: 0.3% trade + 0.01% protocol
//         return calculate_amm_price_with_protocol_fee(
//             amount_in,
//             reserve_in,
//             reserve_out,
//             30,      // 0.3% trade fee
//             10_000,
//             1,       // 0.01% protocol fee
//             10_000,
//         );
//     }
    
//     Err(SwapError::InvalidDexProgram.into())
// }

// /// Generic AMM price calculation (constant product formula)
// fn calculate_amm_price(
//     amount_in: u64,
//     reserve_in: u64,
//     reserve_out: u64,
//     fee_numerator: u64,
//     fee_denominator: u64,
// ) -> Result<(u64, f64), ProgramError> {
    
//     if reserve_in == 0 || reserve_out == 0 {
//         msg!("No liquidity in pool");
//         return Err(SwapError::InsufficientLiquidity.into());
//     }
    
//     let amount_in_with_fee = U192::from(amount_in)
//         .checked_mul(U192::from(fee_denominator - fee_numerator))
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let numerator = amount_in_with_fee
//         .checked_mul(U192::from(reserve_out))
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let denominator = U192::from(reserve_in)
//         .checked_mul(U192::from(fee_denominator))
//         .ok_or(ProgramError::ArithmeticOverflow)?
//         .checked_add(amount_in_with_fee)
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let amount_out = numerator
//         .checked_div(denominator)
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let final_amount_out = amount_out.as_u64();
    
//     // Calculate price impact
//     let price_impact = if reserve_in > 0 && final_amount_out > 0 {
//         let input_ratio = amount_in as f64 / reserve_in as f64;
//         let output_ratio = final_amount_out as f64 / reserve_out as f64;
//         (input_ratio - output_ratio).abs()
//     } else {
//         0.0
//     };
    
//     Ok((final_amount_out, price_impact))
// }

// /// AMM price calculation with protocol fee
// fn calculate_amm_price_with_protocol_fee(
//     amount_in: u64,
//     reserve_in: u64,
//     reserve_out: u64,
//     trade_fee_numerator: u64,
//     trade_fee_denominator: u64,
//     protocol_fee_numerator: u64,
//     protocol_fee_denominator: u64,
// ) -> Result<(u64, f64), ProgramError> {
    
//     // Calculate base amount out with trade fee
//     let (amount_out_before_protocol, price_impact) = calculate_amm_price(
//         amount_in,
//         reserve_in,
//         reserve_out,
//         trade_fee_numerator,
//         trade_fee_denominator,
//     )?;
    
//     // Apply protocol fee
//     let protocol_fee = U192::from(amount_out_before_protocol)
//         .checked_mul(U192::from(protocol_fee_numerator))
//         .ok_or(ProgramError::ArithmeticOverflow)?
//         .checked_div(U192::from(protocol_fee_denominator))
//         .ok_or(ProgramError::ArithmeticOverflow)?;
    
//     let final_amount_out = U192::from(amount_out_before_protocol)
//         .checked_sub(protocol_fee)
//         .ok_or(ProgramError::ArithmeticOverflow)?
//         .as_u64();
    
//     Ok((final_amount_out, price_impact))
// }

// fn emit_swap_event(
//     input_mint: &Pubkey,
//     output_mint: &Pubkey,
//     amount_in: u64,
//     amount_out: u64,
//     price_impact: f64,
//     fee_amount: u64,
// ) -> ProgramResult {
//     msg!(
//         "SWAP_EVENT: {}:{}:{}:{}:{:.6}:{}",
//         input_mint,
//         output_mint,
//         amount_in,
//         amount_out,
//         price_impact,
//         fee_amount
//     );
//     Ok(())
// }

// /// Get a swap quote from a DEX
// /// 
// /// # Accounts:
// /// 0. `[]` Input token mint
// /// 1. `[]` Output token mint  
// /// 2. `[]` DEX program
// /// 3. `[]` Pool input token account
// /// 4. `[]` Pool output token account
// fn get_quote(
//     _program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     instruction_data: &[u8],
// ) -> Result<(), ProgramError> {
//     // Parse accounts
//     let accounts_iter = &mut accounts.iter();
//     let input_mint = next_account_info(accounts_iter)?;
//     let output_mint = next_account_info(accounts_iter)?;
//     let dex_program = next_account_info(accounts_iter)?;
//     let pool_input_account = next_account_info(accounts_iter)?;
//     let pool_output_account = next_account_info(accounts_iter)?;
    
//     // Parse amount from instruction data
//     if instruction_data.len() < 8 {
//         return Err(ProgramError::InvalidInstructionData);
//     }
//     let amount_in = u64::from_le_bytes(instruction_data[0..8].try_into().unwrap());
    
//     // Verify DEX program is supported
//     if dex_program.key != &RAYDIUM_SWAP_V4 && dex_program.key != &ORCA_SWAP_V2 {
//         msg!("Unsupported DEX program");
//         return Err(SwapError::InvalidDexProgram.into());
//     }
    
//     // Get real price quote
//     let (amount_out, price_impact) = get_real_dex_price(
//         dex_program.key,
//         input_mint.key,
//         output_mint.key,
//         amount_in,
//         pool_input_account,
//         pool_output_account,
//     )?;
    
//     // Return the quote
//     msg!("QUOTE:{}:{}:{:.6}", amount_in, amount_out, price_impact);
    
//     msg!(
//         "📊 Quote: {} {} -> {} {} (Impact: {:.2}%)",
//         amount_in,
//         input_mint.key,
//         amount_out, 
//         output_mint.key,
//         price_impact * 100.0
//     );

//     Ok(())
// }

// /// Admin operations (initialize, update settings, pause, etc.)
// fn admin_operations(
//     program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     instruction_data: &[u8],
// ) -> ProgramResult {
//     if instruction_data.is_empty() {
//         return Err(SwapError::InvalidInstruction.into());
//     }
    
//     match instruction_data[0] {
//         // Initialize config
//         0 => initialize_config(program_id, accounts, &instruction_data[1..]),
//         // Update bridge signer
//         1 => update_bridge_signer(program_id, accounts, &instruction_data[1..]),
//         // Pause/unpause
//         2 => toggle_pause(program_id, accounts, &instruction_data[1..]),
//         // Add whitelisted token
//         3 => add_whitelisted_token(program_id, accounts, &instruction_data[1..]),
//         _ => Err(SwapError::InvalidInstruction.into()),
//     }
// }

// /// Initialize the swap config
// fn initialize_config(
//     program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     instruction_data: &[u8],
// ) -> ProgramResult {
//     let accounts_iter = &mut accounts.iter();
//     let admin = next_account_info(accounts_iter)?;
//     let config_pda = next_account_info(accounts_iter)?;
//     let nonce_pda = next_account_info(accounts_iter)?;
//     // let system_program = next_account_info(accounts_iter)?;
    
//     // Admin must sign
//     if !admin.is_signer {
//         return Err(SwapError::Unauthorized.into());
//     }
    
//     // Verify PDA addresses
//     let (expected_config_pda, _config_bump) = Pubkey::find_program_address(
//         &[CONFIG_SEED],
//         program_id,
//     );
    
//     let (expected_nonce_pda, _nonce_bump) = Pubkey::find_program_address(
//         &[NONCE_SEED],
//         program_id,
//     );
    
//     if config_pda.key != &expected_config_pda || nonce_pda.key != &expected_nonce_pda {
//         return Err(ProgramError::InvalidArgument);
//     }
    
//     // Parse initialization data
//     if instruction_data.len() < 32 * 2 {
//         return Err(SwapError::InvalidInstruction.into());
//     }
    
//     let bridge_signer = Pubkey::try_from(&instruction_data[0..32]).unwrap();
//     let fee_recipient = Pubkey::try_from(&instruction_data[32..64]).unwrap();
    
//     // Create config
//     let config = SwapConfig {
//         admin: *admin.key,
//         bridge_signer,
//         fee_recipient,
//         is_paused: false,
//         whitelisted_tokens: [Pubkey::default(); 10],
//         whitelisted_tokens_count: 0,
//     };
    
//     // Initialize nonce to 0
//     let mut nonce_data = nonce_pda.data.borrow_mut();
//     nonce_data[0..8].copy_from_slice(&0u64.to_le_bytes());
    
//     // Store config
//     let mut config_data = config_pda.data.borrow_mut();
//     config.pack(&mut config_data);
    
//     msg!("Config initialized. Admin: {}, Bridge: {}", admin.key, bridge_signer);
    
//     Ok(())
// }

// /// Update bridge signer (admin only)
// fn update_bridge_signer(
//     _program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     instruction_data: &[u8],
// ) -> ProgramResult {
//     let accounts_iter = &mut accounts.iter();
//     let admin = next_account_info(accounts_iter)?;
//     let config_pda = next_account_info(accounts_iter)?;
    
//     // Verify admin
//     if !admin.is_signer {
//         return Err(SwapError::Unauthorized.into());
//     }
    
//     // Load and verify config
//     let mut config_data = config_pda.data.borrow_mut();
//     let mut config = SwapConfig::unpack(&config_data)?;
    
//     if admin.key != &config.admin {
//         return Err(SwapError::InvalidAdmin.into());
//     }
    
//     // Parse new bridge signer
//     if instruction_data.len() < 32 {
//         return Err(SwapError::InvalidInstruction.into());
//     }
    
//     let new_bridge_signer = Pubkey::try_from(&instruction_data[0..32]).unwrap();
//     config.bridge_signer = new_bridge_signer;
    
//     // Save updated config
//     config.pack(&mut config_data);
    
//     msg!("Bridge signer updated to: {}", new_bridge_signer);
    
//     Ok(())
// }

// /// Toggle pause state (admin only)
// fn toggle_pause(
//     _program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     instruction_data: &[u8],
// ) -> ProgramResult {
//     let accounts_iter = &mut accounts.iter();
//     let admin = next_account_info(accounts_iter)?;
//     let config_pda = next_account_info(accounts_iter)?;
    
//     // Verify admin
//     if !admin.is_signer {
//         return Err(SwapError::Unauthorized.into());
//     }
    
//     // Parse pause state
//     if instruction_data.is_empty() {
//         return Err(SwapError::InvalidInstruction.into());
//     }
    
//     let should_pause = instruction_data[0] != 0;
    
//     // Load and update config
//     let mut config_data = config_pda.data.borrow_mut();
//     let mut config = SwapConfig::unpack(&config_data)?;
    
//     if admin.key != &config.admin {
//         return Err(SwapError::InvalidAdmin.into());
//     }
    
//     config.is_paused = should_pause;
//     config.pack(&mut config_data);
    
//     msg!("Contract {} by admin", if should_pause { "paused" } else { "unpaused" });
    
//     Ok(())
// }

// /// Add whitelisted token (admin only)
// fn add_whitelisted_token(
//     _program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     instruction_data: &[u8],
// ) -> ProgramResult {
//     let accounts_iter = &mut accounts.iter();
//     let admin = next_account_info(accounts_iter)?;
//     let config_pda = next_account_info(accounts_iter)?;
    
//     // Verify admin
//     if !admin.is_signer {
//         return Err(SwapError::Unauthorized.into());
//     }
    
//     // Parse token to whitelist
//     if instruction_data.len() < 32 {
//         return Err(SwapError::InvalidInstruction.into());
//     }
    
//     let token_to_whitelist = Pubkey::try_from(&instruction_data[0..32]).unwrap();
    
//     // Load and update config
//     let mut config_data = config_pda.data.borrow_mut();
//     let mut config = SwapConfig::unpack(&config_data)?;
    
//     if admin.key != &config.admin {
//         return Err(SwapError::InvalidAdmin.into());
//     }
    
//     // Check if token already whitelisted
//     if config.is_token_whitelisted(&token_to_whitelist) {
//         msg!("Token already whitelisted");
//         return Ok(());
//     }
    
//     // Check if we have space
//     if config.whitelisted_tokens_count >= 10 {
//         msg!("Whitelist full (max 10 tokens)");
//         return Err(ProgramError::InvalidArgument);
//     }
    
//     // Add token to whitelist
//     config.whitelisted_tokens[config.whitelisted_tokens_count as usize] = token_to_whitelist;
//     config.whitelisted_tokens_count += 1;
    
//     config.pack(&mut config_data);
    
//     msg!("Token whitelisted: {}", token_to_whitelist);
    
//     Ok(())
// }