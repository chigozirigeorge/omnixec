use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use solana_client::{rpc_client::RpcClient, rpc_config::{CommitmentConfig, UiTransactionEncoding}};
use solana_sdk::{
    instruction::Instruction,
    message::{AccountMeta, Message},
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};
use sqlx::types::BigDecimal;
use std::{
    str::FromStr, sync::Arc, time::Duration
};
use tracing::{error, info};

use crate::{
    error::{AppResult, ExecutionError},
    execution::router::Executor,
    ledger::{
        models::*,
        repository::LedgerRepository,
    },
    risk::controls::RiskController
};


#[derive(Debug, Clone)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub commitment: CommitmentConfig,
    pub max_retries: u32,
    pub confirmation_timeout: Duration,
    pub max_compute_units: i32,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self { 
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(), 
            commitment: CommitmentConfig::confirmed(), 
            max_retries: 3, 
            confirmation_timeout: Duration::from_secs(60),
            max_compute_units: 1_400_000,
        }
    }
}


pub struct SolanaExecutor {
    config: SolanaConfig,
    client: RpcClient,
    ledger: Arc<LedgerRepository>,
    risk: Arc<RiskController>,
    treasury_keypair: Arc<Keypair>,
}

impl SolanaExecutor {
    pub fn new(
        config: SolanaConfig,
        ledger: Arc<LedgerRepository>,
        risk: Arc<RiskController>,
        treasury_keypair: Keypair,
    ) -> Self {
        let client = RpcClient::new_with_commitment(config.rpc_url.clone(), config.commitment);

        Self { 
            config, 
            client, 
            ledger, 
            risk, 
            treasury_keypair: Arc::new(treasury_keypair) 
        }
    }

    fn deserialize_instructions(&self, bytes: &[u8]) -> AppResult<Vec<Instruction>> {
        //Parse serializes instructions
        //Format: [num_instructions: u32][instruction_len][instruction_bytes] ...

        if bytes.len() < 4 {
            return Err(ExecutionError::ChainExecutionFailed { 
                chain: Chain::Solana, 
                message: "Invalid instruction data".to_string(), 
            }.into());
        }

        let mut cursor: usize = 0;

        fn read_u32(bytes: &[u8], cursor: &mut usize) -> AppResult<u32> {
            if *cursor + 4 > bytes.len() {
                return Err(ExecutionError::InvalidInstructionData.into());
            }
            let val = u32::from_le_bytes(bytes[*cursor..*cursor + 4].try_into().unwrap());
            *cursor += 4;
            Ok(val)
        }

        fn read_bytes<'a>(bytes: &'a[u8], cursor: &mut usize, len: usize) -> AppResult<&'a [u8]> {
            if *cursor + len > bytes.len() {
                return  Err(ExecutionError::InvalidInstructionData.into());
            }

            let slice = &bytes[*cursor..*cursor + len];
            *cursor += len;
            Ok(slice)
        }

        let num_instructions = read_u32(bytes, &mut cursor)? as usize;

        if num_instructions == 0 || num_instructions > 16 {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        let mut instructions = Vec::with_capacity(num_instructions);

        for _ in 0..num_instructions {
            //program id
            let pid_len = read_u32(bytes, &mut cursor)? as usize;
            let pid_bytes = read_bytes(bytes, &mut cursor, pid_len)?;
            let program_id = Pubkey::try_from(pid_bytes)
                .map_err(|_| ExecutionError::InvalidInstructionData)?;

            //Accounts
            let num_accounts = read_u32(bytes, &mut cursor)? as usize;
            if num_accounts > 32 {
                return Err(ExecutionError::InvalidInstructionData.into());
            }

            let mut accounts = Vec::with_capacity(num_accounts);

            for _ in 0..num_accounts {
                let key_bytes = read_bytes(bytes, &mut cursor, 32)?;
                let pubkey = Pubkey::new_from_array(key_bytes.try_into().unwrap());
                let is_signer = read_bytes(bytes, &mut cursor, 1)?[0] != 0;
                let is_writable = read_bytes(bytes, &mut cursor, 1)?[0] != 0;

                accounts.push(AccountMeta {
                        pubkey,
                        is_signer,
                        is_writable
                    });
            }

            //Instruction data
            let data_len = read_u32(bytes, &mut cursor)? as usize;
            let data = read_bytes(bytes, &mut cursor, data_len)?.to_vec();

            instructions.push(Instruction {
                program_id,
                accounts,
                data
            });
        }

        if cursor != bytes.len() {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        Ok(instructions)
    }

    fn build_transaction(&self, instructions: Vec<Instruction>) -> AppResult<Transaction> {
        let recent_blockhash = self.client
            .get_latest_blockhash()
            .map_err(|e| {
                ExecutionError::ChainExecutionFailed { 
                    chain: Chain::Solana, 
                    message: format!("Failed to get blockhash: {}", e) 
                }
            })?;

        let message = Message::new(&instructions, Some(&self.treasury_keypair.pubkey()));
        let transaction = Transaction::new(
            &[&*self.treasury_keypair], 
            message, 
            recent_blockhash
        );

        Ok(transaction)
    }

    fn simulate_transaction(&self, transaction: &Transaction) -> AppResult<()> {
        let result = self.client
            .simulate_transaction(transaction)
            .map_err(|e| {
                ExecutionError::SimulationFailed(format!("SImulation error: {}", e))
            })?;

        if let Some(err) = result.value.err {
            return  Err(ExecutionError::SimulationFailed(format!(
                "Transaction would fail: {:?}",
                err
            ))
            .into());
        }

        Ok(())
    }

    fn send_transaction(&self, transaction: &Transaction) -> AppResult<Signature> {
        let signature = self
            .client
            .send_and_confirm_transaction(transaction)
            .map_err(|e| ExecutionError::ChainExecutionFailed { 
                chain: Chain::Solana, 
                message: format!("Send failed: {}", e), 
            })?;

        Ok(signature)
    }


    fn confirm_transaction(&self, signature: &Signature) -> AppResult<Decimal> {
        //Get transaction details
        match self.client.get_transaction(signature, UiTransactionEncoding::Json) {
            Ok(confirmed_tx) => {
                if let Some(meta) = confirmed_tx.transaction.meta {
                    let fee = meta.fee;
                    Ok(Decimal::from(fee))
                } else {
                    Ok(Decimal::from(5000))  // Default fee
                }
            }
            Err(_) => Ok(Decimal::from(5000))
        }
    }
}


#[async_trait]
impl Executor for SolanaExecutor {
    async fn execute(&self, quote: &Quote) -> AppResult<Execution> {
        info!("Starting solana execution for quote: {}", quote.id);

        // VALIDATION 1: Verify this is the correct chain
        if quote.execution_chain != Chain::Solana {
            return Err(ExecutionError::ExecutorChainMismatch { 
                expected: quote.execution_chain, 
                actual: Chain::Solana
            }.into());
        }

        // VALIDATION 2: Validate execution instructions are not empty
        if quote.execution_instructions.is_empty() {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        // VALIDATION 3: Validate estimated compute units if provided
        if let Some(compute_units) = quote.estimated_compute_units {
            if compute_units <= 0 || compute_units > self.config.max_compute_units {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Solana,
                    message: format!(
                        "Invalid compute units: {}. Must be between 1 and {}",
                        compute_units, self.config.max_compute_units
                    ),
                }.into());
            }
        }

        // VALIDATION 4: Validate execution cost is reasonable
        if quote.execution_cost.is_sign_negative() || quote.execution_cost.is_zero() {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: "Execution cost must be positive".to_string(),
            }.into());
        }

        // Begin atomic transaction 
        let mut tx = self.ledger.begin_tx().await?;

        //create execution record (idempotency via UNIQUE constraint)
        let execution = match self
            .ledger
            .create_execution(&mut tx, quote.id, Chain::Solana)
            .await
            {
                Ok(exec) => {
                    tx.commit().await?;
                    exec
                }
                Err(_) => {
                    tx.rollback().await?;
                    return Err(ExecutionError::DuplicateExecution.into());
                }
            };

        //Risk control check
        self.risk
        .check_execution_allowed(Chain::Solana, quote.execution_cost)
        .await?;

        let instructions = self.deserialize_instructions(&quote.execution_instructions)?;

        //Build transaction 
        let transaction = self.build_transaction(instructions)?;

        //simulate first
        self.simulate_transaction(&transaction)?;

        info!("Simulation successful, sending transaction");

        // Send transaction
        let signature = match self.send_transaction(&transaction) {
            Ok(sig) => sig,
            Err(e) => {
                error!("Failed to send transaction: {:?}", e);

                let mut tx = self.ledger.begin_tx().await?;
                self.ledger
                    .complete_execution(
                        &mut tx, 
                        execution.id, 
                        ExecutionStatus::Failed, 
                        None, 
                        None, 
                        Some(e.to_string()),
                    )
                    .await?;

                tx.commit().await?;

                return Err(e);
            }
        };
        
        info!("Transaction sent: {}", signature);

        //Get gas used
        let gas_used = self.confirm_transaction(&signature)?;

        //Record successful execution
        let mut tx = self.ledger.begin_tx().await?;

        self.ledger
            .complete_execution(
                &mut tx, 
                execution.id, 
                ExecutionStatus::Success, 
                Some(signature.to_string()), 
                Some(BigDecimal::from_str(&gas_used.to_string()).unwrap()), 
                None,
            )
            .await?;

        self.ledger
            .update_quote_status(
                &mut tx, 
                quote.id, 
                QuoteStatus::Committed, 
                QuoteStatus::Executed
            ).await?;

        self.risk
            .record_spending(&mut tx, Chain::Solana, quote.execution_cost)
            .await?;

        self.ledger
            .log_audit_event(
                AuditEventType::ExecutionCompleted, 
                Some(Chain::Solana), 
                Some(execution.id), 
                Some(quote.user_id), 
                serde_json::json!({
                    "signature": signature.to_string(),
                    "gas_used": gas_used.to_string()
                })).await?;
        
        tx.commit().await?;

        info!("Solana execution completed successfully");

        Ok(Execution { 
            id: execution.id, 
            quote_id: quote.id, 
            execution_chain: Chain::Solana, 
            transaction_hash: Some(signature.to_string()), 
            status: ExecutionStatus::Success, 
            gas_used: Some(gas_used), 
            error_message: None, 
            retry_count: 0, 
            executed_at: Utc::now(), 
            completed_at: Some(Utc::now()) 
        })
    }

    fn chain(&self) -> Chain {
        Chain::Solana
    }

    async fn check_treasury_balance(&self, required: Decimal) -> AppResult<()> {
        let balance = self.get_treasury_balance().await?;

        if balance < required {
            return Err(ExecutionError::InsufficientTreasury(Chain::Solana).into());
        }

        Ok(())
    }

    async fn get_treasury_balance(&self) -> AppResult<Decimal> {
        let balance = self
            .client
            .get_balance(&self.treasury_keypair.pubkey())
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain:  Chain::Solana, 
                message: format!("Failed to get balance: {}", e),
            })?;

        //convert lamports to SOL
        Ok(Decimal::from(balance) / Decimal::from(1_000_000_000))
    }

    async fn transfer_to_treasury(&self, token_or_asset: &str, amount: &str) -> AppResult<String> {
    info!("🔄 Solana settlement transfer initiated: {} {}", amount, token_or_asset);
    
    // STEP 1: Parse and validate amount
    let amount_lamports: u64 = amount.parse()
        .map_err(|_| ExecutionError::ChainExecutionFailed {
            chain: Chain::Solana,
            message: format!("Invalid amount format: {}", amount),
        })?;
    
    if amount_lamports == 0 {
        return Err(ExecutionError::ChainExecutionFailed {
            chain: Chain::Solana,
            message: "Transfer amount must be greater than zero".to_string(),
        }.into());
    }
    
    let treasury_pubkey = self.treasury_keypair.pubkey();
    info!(" Amount: {} lamports | Treasury: {}", amount_lamports, treasury_pubkey);
    
    // STEP 2: Get recent blockhash (required for all transactions)
    let recent_blockhash = self.client
        .get_latest_blockhash()
        .map_err(|e| ExecutionError::ChainExecutionFailed {
            chain: Chain::Solana,
            message: format!("Failed to get blockhash: {}", e),
        })?;
    
    // STEP 3: Execute based on asset type
    let tx_hash = if token_or_asset.to_uppercase() == "SOL" {
        // ===== NATIVE SOL TRANSFER =====
        info!("Native SOL transfer: {} lamports", amount_lamports);
        
        // build and submit the transaction
        
        // 1. Create the system instruction for transfer
        //    (In real settlement: settlement_account -> treasury_account)
        
        // For this implementation: treasury transfers to settlement account
        // TODO (In production: would be from settlement account to treasury)
        let settlement_account = Keypair::new();  // In production: get from settlement data
        
        // Build transfer instruction - directly create system instruction
        // System program: Transfer = instruction_data[0] = 2, followed by 8-byte amount in little-endian
        let mut instruction_data = vec![2u8];  // Transfer instruction discriminator
        instruction_data.extend_from_slice(&amount_lamports.to_le_bytes());
        
        // System program ID (well-known constant)
        let system_program_id = Pubkey::from([0; 32]);  // System program is all zeros
        
        let transfer_instruction = Instruction {
            program_id: system_program_id,
            accounts: vec![
                AccountMeta::new(treasury_pubkey, true),           // from (must be signer)
                AccountMeta::new(settlement_account.pubkey(), false), // to
            ],
            data: instruction_data,
        };
        
        // 2. Create the transaction message
        let message = Message::new(
            &[transfer_instruction],
            Some(&treasury_pubkey),
        );
        
        // 3. Build and sign the transaction
        let mut transaction = Transaction::new_unsigned(message);
        transaction.sign(&[&*self.treasury_keypair], recent_blockhash);
        
        // 4. Simulate first (safety check)
        match self.client.simulate_transaction(&transaction) {
            Ok(sim_result) => {
                if let Some(err) = sim_result.value.err {
                    return Err(ExecutionError::ChainExecutionFailed {
                        chain: Chain::Solana,
                        message: format!("Transaction simulation failed: {:?}", err),
                    }.into());
                }
            }
            Err(e) => {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Solana,
                    message: format!("Simulation error: {}", e),
                }.into());
            }
        }
        
        // 5. Send transaction to network
        let signature = self.client
            .send_transaction(&transaction)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Failed to send transaction: {}", e),
            })?;
        
        info!("✅ Native SOL transaction sent: {}", signature);
        
        // 6. Wait for confirmation (finality)
        match self.client.confirm_transaction_with_spinner(
            &signature,
            &recent_blockhash,
            CommitmentConfig::finalized(),
        ) {
            Ok(_) => {
                info!("✅ Transaction confirmed on-chain: {}", signature);
                signature.to_string()
            }
            Err(e) => {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Solana,
                    message: format!("Transaction confirmation failed: {}", e),
                }.into());
            }
        }
    } else {
        // ===== SPL TOKEN TRANSFER =====
        info!("SPL Token transfer: {} of {} to treasury", amount_lamports, token_or_asset);
        
        // Build and submit SPL token transfer
        
        // 1. Parse mint address
        let mint_pubkey = Pubkey::from_str(token_or_asset)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Invalid mint address: {}", token_or_asset),
            })?;
        
        // 2. Get treasury's Associated Token Account for this mint
        let treasury_ata = spl_associated_token_account::get_associated_token_address(
            &treasury_pubkey,
            &mint_pubkey,
        );
        
        // 3. Verify treasury's ATA exists (query account data)
        match self.client.get_account(&treasury_ata) {
            Ok(account) => {
                if account.owner != spl_token::ID {
                    return Err(ExecutionError::ChainExecutionFailed {
                        chain: Chain::Solana,
                        message: "Treasury ATA owned by wrong program".to_string(),
                    }.into());
                }
            }
            Err(_) => {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Solana,
                    message: "Treasury ATA not found for this token".to_string(),
                }.into());
            }
        }
        
        // 4. Create SPL token transfer instruction
        //  TODO:  (In settlement: source_account -> treasury_ata)
        let settlement_token_account = Keypair::new();  // In production: from settlement data
        
        let transfer_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            &settlement_token_account.pubkey(),
            &treasury_ata,
            &treasury_pubkey,
            &[&treasury_pubkey],  // signers
            amount_lamports,
        ).map_err(|e| ExecutionError::ChainExecutionFailed {
            chain: Chain::Solana,
            message: format!("Failed to build SPL transfer instruction: {:?}", e),
        })?;
        
        // 5. Create and sign transaction
        let message = Message::new(
            &[transfer_instruction],
            Some(&treasury_pubkey),
        );
        
        let mut transaction = Transaction::new_unsigned(message);
        transaction.sign(&[&*self.treasury_keypair], recent_blockhash);
        
        // 6. Simulate
        match self.client.simulate_transaction(&transaction) {
            Ok(sim_result) => {
                if let Some(err) = sim_result.value.err {
                    return Err(ExecutionError::ChainExecutionFailed {
                        chain: Chain::Solana,
                        message: format!("Token transfer simulation failed: {:?}", err),
                    }.into());
                }
            }
            Err(e) => {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Solana,
                    message: format!("Simulation error: {}", e),
                }.into());
            }
        }
        
        // 7. Send transaction
        let signature = self.client
            .send_transaction(&transaction)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Failed to send token transfer: {}", e),
            })?;
        
        info!("✅ SPL token transaction sent: {}", signature);
        
        // 8. Wait for confirmation
        match self.client.confirm_transaction_with_spinner(
            &signature,
            &recent_blockhash,
            CommitmentConfig::finalized(),
        ) {
            Ok(_) => {
                info!("✅ Token transfer confirmed: {}", signature);
                signature.to_string()
            }
            Err(e) => {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Solana,
                    message: format!("Token transfer confirmation failed: {}", e),
                }.into());
            }
        }
    };
    
    info!(" Settlement recorded: {}", tx_hash);
    Ok(tx_hash)
}
}

impl SolanaExecutor {
    /// Wait for transaction confirmation
    pub async fn wait_for_confirmation(
        &self,
        tx_hash: &str,
        timeout_secs: u64,
    ) -> AppResult<bool> {
        let signature = Signature::from_str(tx_hash)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: "Invalid transaction hash".to_string(),
            })?;

        let start = std::time::Instant::now();
        loop {
            // Use get_signature_statuses which returns the status we need
            match self.client.get_signature_statuses(&[signature]) {
                Ok(response) => {
                    if let Some(Some(status)) = response.value.first() {
                        if status.confirmation_status.is_some() {
                            return Ok(true);
                        }
                    }
                }
                Err(_) => {
                    // Continue waiting
                }
            }

            if start.elapsed().as_secs() > timeout_secs {
                return Ok(false);
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    /// Get current block height
    pub async fn get_block_height(&self) -> AppResult<i64> {
        let slot = self.client.get_slot()
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Failed to get slot: {}", e),
            })?;
        Ok(slot as i64)
    }

    /// Execute a token swap on Solana via the smart contract
    ///
    /// FLOW:
    /// 1. Build swap instruction with input/output tokens and amounts
    /// 2. Execute via the deployed smart contract program
    /// 3. Contract handles: transfer from treasury → DEX swap → transfer to user wallet
    /// 4. Return transaction hash and confirmation
    pub async fn execute_swap(
        &self,
        user_wallet: &str,
        input_token: &str,
        output_token: &str,
        amount_in: u64,
        min_amount_out: u64,
        treasury_input_ata: &str,
        treasury_output_ata: &str,
    ) -> AppResult<String> {
        info!("Executing Solana swap: {} {} -> {}", 
            amount_in, input_token, output_token);

        // Parse all addresses
        let user_wallet_pk = Pubkey::from_str(user_wallet)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Invalid user wallet address: {}", user_wallet),
            })?;

        let input_mint = Pubkey::from_str(input_token)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Invalid input token address: {}", input_token),
            })?;

        let output_mint = Pubkey::from_str(output_token)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Invalid output token address: {}", output_token),
            })?;

        let treasury_input_pk = Pubkey::from_str(treasury_input_ata)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Invalid treasury input ATA: {}", treasury_input_ata),
            })?;

        let treasury_output_pk = Pubkey::from_str(treasury_output_ata)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Invalid treasury output ATA: {}", treasury_output_ata),
            })?;

        // Smart contract program ID (deployed on-chain)
        let swap_program_id = Pubkey::from_str(
            "YOUR_SOLANA_SWAP_PROGRAM_ID"  // This will be replaced with actual deployed address
        ).map_err(|_| ExecutionError::ChainExecutionFailed {
            chain: Chain::Solana,
            message: "Invalid swap program ID configuration".to_string(),
        })?;

        // DEX program (e.g., Raydium)
        let dex_program_id = Pubkey::from_str(
            "9xQeWvG816bUx9EPjHmaT23sSikZWfqDmZ1HjbDMuNA" // Raydium Swap V4
        ).map_err(|_| ExecutionError::ChainExecutionFailed {
            chain: Chain::Solana,
            message: "Invalid DEX program ID".to_string(),
        })?;

        // Build instruction data: [0: u8 = 0 for swap][amount_in: u64][min_amount_out: u64]
        let mut instruction_data = vec![0u8]; // Instruction type: execute_swap
        instruction_data.extend_from_slice(&amount_in.to_le_bytes());
        instruction_data.extend_from_slice(&min_amount_out.to_le_bytes());

        info!("📋 Swap instruction data:");
        info!("   - Treasury: {}", self.treasury_keypair.pubkey());
        info!("   - User wallet: {}", user_wallet_pk);
        info!("   - Input token: {}", input_mint);
        info!("   - Output token: {}", output_mint);
        info!("   - Amount in: {} tokens", amount_in);
        info!("   - Min out: {} tokens", min_amount_out);

        // Construct the swap instruction
        let swap_instruction = Instruction {
            program_id: swap_program_id,
            accounts: vec![
                AccountMeta::new(self.treasury_keypair.pubkey(), true),  // 0. treasury (signer)
                AccountMeta::new_readonly(input_mint, false),           // 1. input_token_mint
                AccountMeta::new_readonly(output_mint, false),          // 2. output_token_mint
                AccountMeta::new(user_wallet_pk, false),                // 3. user_wallet (receives output)
                AccountMeta::new(treasury_input_pk, false),             // 4. treasury_input_ata (holds input tokens)
                AccountMeta::new(treasury_output_pk, false),            // 5. treasury_output_ata (receives output from DEX)
                AccountMeta::new_readonly(dex_program_id, false),       // 6. dex_program (Raydium)
                AccountMeta::new_readonly(spl_token::id(), false),      // 7. token_program
            ],
            data: instruction_data,
        };

        // Get latest blockhash for transaction
        let blockhash = self.client.get_latest_blockhash()
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Failed to get blockhash: {}", e),
            })?;

        // Build and sign transaction
        let message = Message::new(&[swap_instruction], Some(&self.treasury_keypair.pubkey()));
        let mut transaction = Transaction::new_unsigned(message);
        transaction.sign(&[&*self.treasury_keypair], blockhash);

        info!("🔄 Simulating swap transaction before sending...");

        // Simulate first to check for errors
        self.simulate_transaction(&transaction)?;

        info!("✅ Simulation successful, sending transaction...");

        // Send transaction
        let signature = self.send_transaction(&transaction)?;

        info!("📤 Swap transaction sent: {}", signature);

        // Wait for confirmation
        let tx_hash = signature.to_string();
        
        // Poll for confirmation (with timeout)
        if self.wait_for_confirmation(&tx_hash, 60).await? {
            info!("✅ Swap confirmed on-chain");
            Ok(tx_hash)
        } else {
            Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: "Swap transaction confirmation timeout".to_string(),
            }.into())
        }
    }

    /// Call the swap contract to get a price quote
    /// 
    /// This queries the deployed swap contract to get actual swap rates
    /// instead of mock values.
    pub async fn get_swap_quote(
        &self,
        input_token: &str,
        output_token: &str,
        amount_in: u64,
    ) -> AppResult<(u64, Decimal)> {
        // Parse token addresses
        let input_mint = Pubkey::from_str(input_token)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Invalid input token address: {}", input_token),
            })?;

        let output_mint = Pubkey::from_str(output_token)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Solana,
                message: format!("Invalid output token address: {}", output_token),
            })?;

        // Contract program ID (will be deployed)
        let swap_program_id = Pubkey::from_str(
            "YOUR_SOLANA_SWAP_PROGRAM_ID"  //fix with actual
        ).map_err(|_| ExecutionError::ChainExecutionFailed {
            chain: Chain::Solana,
            message: "Invalid swap program ID configuration".to_string(),
        })?;

        // Build instruction data: [instruction_type: u8] (1 = get_quote)
        // followed by: [amount_in: u64]
        let mut instruction_data = vec![1u8]; // Instruction type: get_quote
        instruction_data.extend_from_slice(&amount_in.to_le_bytes());

        // Construct instruction to call get_quote on contract
        let instruction = Instruction {
            program_id: swap_program_id,
            accounts: vec![
                AccountMeta::new_readonly(input_mint, false),
                AccountMeta::new_readonly(output_mint, false),
            ],
            data: instruction_data,
        };

        // Simulate the instruction to get the quote result
        let message = Message::new(
            &[instruction],
            Some(&self.treasury_keypair.pubkey()),
        );

        let transaction = Transaction::new_unsigned(message);

        // Simulate to get output without actually executing
        match self.client.simulate_transaction(&transaction) {
            Ok(response) => {
                if response.value.err.is_some() {
                    return Err(ExecutionError::ChainExecutionFailed {
                        chain: Chain::Solana,
                        message: "Swap quote simulation failed".to_string(),
                    }.into());
                }

                // Parse the logs to extract the quote result
                // Contract logs: "QUOTE_RESULT:amount_out:rate"
                if let Some(logs) = response.value.logs {
                    for log in logs {
                        if log.starts_with("QUOTE_RESULT:") {
                            let parts: Vec<&str> = log.split(':').collect();
                            if parts.len() >= 3 {
                                if let (Ok(amount_out), Ok(rate_str)) = (
                                    parts[1].parse::<u64>(),
                                    parts[2].parse::<Decimal>()
                                ) {
                                    info!("Got swap quote: {} -> {} at rate {}", 
                                        amount_in, amount_out, rate_str);
                                    return Ok((amount_out, rate_str));
                                }
                            }
                        }
                    }
                }

                // Fallback: if no quote in logs, return error
                Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Solana,
                    message: "No quote result in contract response".to_string(),
                }.into())
            }
            Err(e) => {
                error!("Failed to simulate swap quote: {}", e);
                Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Solana,
                    message: format!("Simulation failed: {}", e),
                }.into())
            }
        }
    }
}
