#![deny(missing_docs)]

//! Cross-chain Treasury Swap Program (Solana 2.x)

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    
};
use spl_token::state::Account as TokenAccount;
// use solana_program::program_pack::Pack;

entrypoint!(process_instruction);

/// =======================
/// ===== Constants =======
/// =======================

const CONFIG_SEED: &[u8] = b"config";
const TREASURY_SEED: &[u8] = b"treasury";
const NONCE_SEED: &[u8] = b"nonce";
const WHITELIST_SEED: &[u8] = b"whitelist";

const MAX_WHITELIST: usize = 16;
const MAX_TOKEN_WHITELIST: usize = 30;
const BPS_DENOMINATOR: u64 = 10_000;

/// =======================
/// ===== Program State ===
/// =======================

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Config {
    pub admin: Pubkey,
    pub fee_bps: u16,
    pub paused: bool,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct NonceState {
    pub last_nonce: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Whitelist {
    pub dexes: Vec<Pubkey>,
    pub tokens: Vec<Pubkey>,
}

/// =======================
/// ===== Instructions ===
/// =======================

#[derive(BorshSerialize, BorshDeserialize)]
pub enum ProgramInstruction {
    /// Admin-only
    Initialize {
        admin: Pubkey,
        fee_bps: u16,
    },

    /// Admin-only
    AddDex {
        dex: Pubkey,
    },

    /// Admin-only
    AddToken {
        mint: Pubkey,
    },

    /// Execute treasury-funded swap
    ExecuteSwap {
        nonce: u64,
        min_output: u64,
    },
}

/// =======================
/// ===== Entrypoint ======
/// =======================

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let ix = ProgramInstruction::try_from_slice(data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    match ix {
        ProgramInstruction::Initialize { admin, fee_bps } => {
            initialize(program_id, accounts, admin, fee_bps)
        }
        ProgramInstruction::AddDex { dex } => add_dex(program_id, accounts, dex),
        ProgramInstruction::AddToken { mint } => add_token(program_id, accounts, mint),
        ProgramInstruction::ExecuteSwap { nonce, min_output } => {
            execute_swap(program_id, accounts, nonce, min_output)
        }
    }
}

/// =======================
/// ===== Admin Logic =====
/// =======================

fn initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    admin: Pubkey,
    fee_bps: u16,
) -> ProgramResult {
    let ai = &mut accounts.iter();
    let payer = next_account_info(ai)?;
    let config_ai = next_account_info(ai)?;
    let whitelist_ai = next_account_info(ai)?;
    let nonce_ai = next_account_info(ai)?;

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let config = Config {
        admin,
        fee_bps,
        paused: false,
    };
    config.serialize(&mut *config_ai.data.borrow_mut())?;

    Whitelist {
        dexes: Vec::new(),
        tokens: Vec::new(),
    }
    .serialize(&mut *whitelist_ai.data.borrow_mut())?;

    NonceState { last_nonce: 0 }
        .serialize(&mut *nonce_ai.data.borrow_mut())?;

    msg!("initialized admin={}", admin);
    Ok(())
}

fn add_dex(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    dex: Pubkey,
) -> ProgramResult {
    admin_guard(accounts)?;
    let whitelist_ai = &accounts[2];

    let mut wl = Whitelist::try_from_slice(&whitelist_ai.data.borrow())?;
    if wl.dexes.len() >= MAX_WHITELIST {
        return Err(ProgramError::AccountDataTooSmall);
    }
    wl.dexes.push(dex);
    wl.serialize(&mut *whitelist_ai.data.borrow_mut())?;

    msg!("dex_whitelisted={}", dex);
    Ok(())
}

fn add_token(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    mint: Pubkey,
) -> ProgramResult {
    admin_guard(accounts)?;
    let whitelist_ai = &accounts[2];

    let mut wl = Whitelist::try_from_slice(&whitelist_ai.data.borrow())?;
    if wl.tokens.len() >= MAX_TOKEN_WHITELIST {
        return Err(ProgramError::AccountDataTooSmall);
    }
    wl.tokens.push(mint);
    wl.serialize(&mut *whitelist_ai.data.borrow_mut())?;

    msg!("token_whitelisted={}", mint);
    Ok(())
}

fn admin_guard(accounts: &[AccountInfo]) -> ProgramResult {
    let admin = &accounts[0];
    let config_ai = &accounts[1];
    let config = Config::try_from_slice(&config_ai.data.borrow())?;

    if !admin.is_signer || admin.key != &config.admin {
        return Err(ProgramError::IllegalOwner);
    }
    Ok(())
}

/// =======================
/// ===== Swap Logic ======
/// =======================

fn execute_swap(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    nonce: u64,
    min_output: u64,
) -> ProgramResult {
    let ai = &mut accounts.iter();

    let backend = next_account_info(ai)?;
    let config_ai = next_account_info(ai)?;
    let nonce_ai = next_account_info(ai)?;
    let whitelist_ai = next_account_info(ai)?;
    let treasury_authority = next_account_info(ai)?;
    let treasury_output = next_account_info(ai)?;
    let user_output = next_account_info(ai)?;
    let dex_program = next_account_info(ai)?;
    let token_program = next_account_info(ai)?;

    if !backend.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let config = Config::try_from_slice(&config_ai.data.borrow())?;
    let wl = Whitelist::try_from_slice(&whitelist_ai.data.borrow())?;

    if config.paused {
        return Err(ProgramError::InvalidAccountData);
    }
    if !wl.dexes.contains(dex_program.key) {
        return Err(ProgramError::IncorrectProgramId);
    }

    /// ---- Replay protection ----
    let mut nonce_state = NonceState::try_from_slice(&nonce_ai.data.borrow())?;
    if nonce <= nonce_state.last_nonce {
        return Err(ProgramError::InvalidInstructionData);
    }
    nonce_state.last_nonce = nonce;
    nonce_state.serialize(&mut *nonce_ai.data.borrow_mut())?;

    /// ---- Snapshot balance before swap ----
    let before = TokenAccount::unpack(&treasury_output.data.borrow())?.amount;

    /// ---- CPI into DEX ----
    let remaining: Vec<AccountInfo> = ai.cloned().collect();
    invoke(
        &Instruction {
            program_id: *dex_program.key,
            accounts: remaining
                .iter()
                .map(|a| AccountMeta {
                    pubkey: *a.key,
                    is_signer: a.is_signer,
                    is_writable: a.is_writable,
                })
                .collect(),
            data: vec![],
        },
        &remaining,
    )?;

    /// ---- Snapshot after swap ----
    let after_amt = TokenAccount::unpack(&treasury_output.data.borrow())?.amount;
    let delta = after_amt
        .checked_sub(before)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    if delta < min_output {
        return Err(ProgramError::InvalidArgument);
    }

    /// ---- Fee extraction ----
    let fee = (delta * config.fee_bps as u64) / BPS_DENOMINATOR;
    let user_amount = delta - fee;

    /// ---- Transfer to user ----
    let (treasury_pda, bump) =
        Pubkey::find_program_address(&[TREASURY_SEED], program_id);

    let transfer_ix = spl_token::instruction::transfer(
        token_program.key,
        treasury_output.key,
        user_output.key,
        &treasury_pda,
        &[],
        user_amount,
    )?;

    invoke_signed(
        &transfer_ix,
        &[
            treasury_output.clone(),
            user_output.clone(),
            token_program.clone(),
        ],
        &[&[TREASURY_SEED, &[bump]]],
    )?;

    msg!(
        "swap nonce={} gross={} fee={} net={}",
        nonce,
        delta,
        fee,
        user_amount
    );

    Ok(())
}
