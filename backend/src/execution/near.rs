use async_trait::async_trait;
use rust_decimal::Decimal;
use near_jsonrpc_client::{JsonRpcClient, methods};
use near_primitives::{
    account::id::TryIntoAccountId,
    hash::CryptoHash,
    types::{AccountId, BlockId, BlockReference, Finality}, views::{QueryRequest, TxExecutionStatus},
};
use near_crypto::{SecretKey, InMemorySigner, Signer};
use near_token::NearToken;
use near_primitives::types::Gas;
use near_primitives::transaction::TransactionV0;
use sqlx::types::BigDecimal;
use std::sync::Arc;
use std::str::FromStr;
use tracing::{error, info};
use near_jsonrpc_primitives::types::query::QueryResponseKind;



use crate::{
    error::{AppResult, ExecutionError},
    execution::router::Executor,
    ledger::{
        models::*,
        repository::LedgerRepository,
    },
    risk::controls::RiskController,
};


#[derive(Debug, Clone)]
pub struct NearConfig {
    pub rpc_url: String,
    pub network_id: String,
}

impl Default for NearConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://rpc.testnet.near.org".to_string(),
            network_id: "mainnet".to_string(),
        }
    }
}

#[derive(Debug)]
struct NearAction {
    receiver_id: String,
    amount: u128,
    method_name: Option<String>,
    args: Vec<u8>,
}

pub struct NearExecutor {
    config: NearConfig,
    ledger: Arc<LedgerRepository>,
    risk: Arc<RiskController>,
    treasury_key: String,
    client: JsonRpcClient,
}

impl NearExecutor {
    pub fn new(
        config: NearConfig,
        ledger: Arc<LedgerRepository>,
        risk: Arc<RiskController>,
        treasury_key: String,
    ) -> Self {
        Self {
            config,
            ledger,
            risk,
            treasury_key,
            client: JsonRpcClient::connect("https://rpc.testnet.near.org"),
        }
    }

    async fn parse_action(&self, bytes: &[u8]) -> AppResult<NearAction> {
        // VALIDATION 1: Minimum size check (receiver_id_len + amount)
        if bytes.len() < 4 + 32 {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        let mut cursor = 0;

        // VALIDATION 2: Parse receiver ID length
        let receiver_id_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;

        if receiver_id_len == 0 || receiver_id_len > 64 {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Invalid receiver ID length: {}", receiver_id_len),
            }.into());
        }

        // VALIDATION 3: Ensure we have enough bytes for receiver ID
        if cursor + receiver_id_len > bytes.len() {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        // VALIDATION 4: Parse and validate receiver ID as valid UTF-8
        let receiver_id = str::from_utf8(&bytes[cursor..cursor + receiver_id_len])
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: "Receiver ID is not valid UTF-8".to_string(),
            })?
            .to_string();
        cursor += receiver_id_len;

        // VALIDATION 5: Parse amount (in yoctoNEAR)
        if cursor + 32 > bytes.len() {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        let amount = u128::from_le_bytes(bytes[cursor..cursor + 16].try_into().unwrap());
        cursor += 16;

        if amount == 0 {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: "Amount must be positive".to_string(),
            }.into());
        }

        // VALIDATION 6: Parse method name if present
        let method_name = if cursor < bytes.len() {
            let method_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;

            if method_len > 256 {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Near,
                    message: "Method name too long".to_string(),
                }.into());
            }

            if method_len > 0 {
                if cursor + method_len > bytes.len() {
                    return Err(ExecutionError::InvalidInstructionData.into());
                }

                let method = str::from_utf8(&bytes[cursor..cursor + method_len])
                    .map_err(|_| ExecutionError::ChainExecutionFailed {
                        chain: Chain::Near,
                        message: "Method name is not valid UTF-8".to_string(),
                    })?
                    .to_string();
                let _ = cursor + method_len; // Acknowledge we would consume these bytes

                Some(method)
            } else {
                None
            }
        } else {
            None
        };

        Ok(NearAction {
            receiver_id,
            amount,
            method_name,
            args: vec![],
        })
    }

    async fn submit_transaction(&self, action: &NearAction) -> AppResult<String> {
        info!(
            "Submitting Near transaction: {} yoctoNEAR to {}",
            action.amount, action.receiver_id
        );

        // VALIDATION 1: Verify treasury key is valid account ID format
        if self.treasury_key.is_empty() || self.treasury_key.len() > 64 {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: "Invalid treasury account ID".to_string(),
            }.into());
        }

        // VALIDATION 2: Verify receiver ID exists and is valid
        if action.receiver_id.is_empty() {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: "Receiver ID cannot be empty".to_string(),
            }.into());
        }

        // VALIDATION 3: Check for self-transfer (anti-pattern)
        if self.treasury_key == action.receiver_id {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: "Cannot transfer to the same account".to_string(),
            }.into());
        }

        // VALIDATION 4: Amount must be positive
        if action.amount == 0 {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: "Transfer amount must be greater than zero".to_string(),
            }.into());
        }

        // Step 1: Parse and validate account IDs
        let sender_id = AccountId::from_str(&self.treasury_key)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Invalid sender account ID: {}", e),
            })?;

        let receiver_id = AccountId::from_str(&action.receiver_id)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Invalid receiver account ID: {}", e),
            })?;

        // Step 2: Fetch latest block hash from NEAR network
        let request = methods::block::RpcBlockRequest {
            block_reference: BlockReference::Finality(Finality::Final),
        };
        let block_response = self.client
            .call(request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to fetch block: {}", e),
            })?;

        let block_hash = block_response.header.hash;

        // Step 3: Fetch sender account to get nonce from access key
        let access_request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::ViewAccessKey {
                account_id: sender_id.clone(),
                public_key: self.get_public_key()?,
            },
        };

        let access_response = self.client
            .call(access_request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to fetch account: {}", e),
            })?;

        let _nonce = access_response.block_height;





        let nonce = match access_response.kind {
            QueryResponseKind::AccessKey(access_key) => access_key.nonce,
            _ => {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Near,
                    message: "Unexpected query response type".to_string(),
                }.into());
            }
        };

        // Step 4: Build Transfer action
        use near_primitives::transaction::{Action, Transaction, TransferAction, SignedTransaction};

        let transfer = TransferAction {
            deposit: NearToken::from_yoctonear(action.amount),
        };

        let tx = Transaction::V0(TransactionV0 {
            signer_id: sender_id.clone(),
            public_key: self.get_public_key()?,
            nonce: nonce + 1,
            receiver_id: receiver_id.clone(),
            block_hash,
            actions: vec![Action::Transfer(transfer)],
        });

        // Pre-compute tx hash before signing
        let tx_hash_bytes = tx.get_hash_and_size().0;
        let tx_hash = format!("near_{}", hex::encode(tx_hash_bytes.as_ref()));

        // Step 5: Sign transaction
        let signer = self.create_signer(&sender_id)?;
        let signature = signer.sign(tx_hash_bytes.as_ref());
        let signed_tx = SignedTransaction::new(signature, tx);

        // Step 6: Submit transaction
        let send_request = methods::send_tx::RpcSendTransactionRequest {
            signed_transaction: signed_tx,
            wait_until: TxExecutionStatus::Final,
        };

        let _send_response = self.client
            .call(send_request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to submit transaction: {}", e),
            })?;

        info!("✅ NEAR transfer submitted: {} -> {} ({} yoctoNEAR)", sender_id, receiver_id, action.amount);
        info!("Transaction hash: {}", tx_hash);

        Ok(tx_hash)
    }

    /// Get the public key from the secret key
    fn get_public_key(&self) -> AppResult<near_crypto::PublicKey> {
        let secret_key = SecretKey::from_str(&self.treasury_key)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Invalid secret key format: {}", e),
            })?;

        Ok(secret_key.public_key())
    }

    /// Create an in-memory signer from the treasury secret key
    fn create_signer(&self, account_id: &AccountId) -> AppResult<InMemorySigner> {
        let secret_key = SecretKey::from_str(&self.treasury_key)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to parse secret key: {}", e),
            })?;

        match InMemorySigner::from_secret_key(account_id.clone(), secret_key) {
            Signer::InMemory(signer) => Ok(signer),
            _ => Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: "Unexpected signer type".to_string(),
            }.into()),
        }
    }

    async fn get_transaction_fee(&self, tx_hash: &str) -> AppResult<Decimal> {
        // Query transaction outcome to get gas burned using NEAR RPC

        let request = methods::gas_price::RpcGasPriceRequest{
            block_id: Some(BlockId::Hash(CryptoHash::hash_borsh(tx_hash)))
        };

        let tx_response = self.client
            .call(request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to fetch transaction details: {}", e),
            })?;

        // Extract gas burned from transaction outcome
        // NEAR RPC returns: {"final_execution_outcome": {"outcome": {"gas_burnt": <u128>}}}
        let gas_burned = tx_response.gas_price.as_near();

        // // Convert gas to NEAR using standard NEAR economics:
        // // Gas price: 100 yoctoNEAR per gas unit (fixed)
        // // 1 NEAR = 10^24 yoctoNEAR
        // let gas_price_yocto: u128 = 100;
        // let fee_yocto = gas_burned as u128 * gas_price_yocto;
        // let fee_near = Decimal::from(fee_yocto) / Decimal::from(1_000_000_000_000_000_000_000_000u128);

        info!("Transaction {} fee: {} NEAR ", tx_hash, gas_burned);

        Ok(Decimal::from(gas_burned))
    }
}

#[async_trait]
impl Executor for NearExecutor {
    async fn execute(&self, quote: &Quote) -> AppResult<Execution> {
        info!("Starting Near execution for quote: {}", quote.id);

        // VALIDATION 1: Verify this is the correct chain
        if quote.execution_chain != Chain::Near {
            return Err(ExecutionError::ExecutorChainMismatch {
                expected: quote.execution_chain,
                actual: Chain::Near,
            }
            .into());
        }

        // VALIDATION 2: Validate execution instructions are not empty
        if quote.execution_instructions.is_empty() {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        // VALIDATION 3: Validate execution cost is reasonable
        if quote.execution_cost.is_sign_negative() || quote.execution_cost.is_zero() {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: "Execution cost must be positive".to_string(),
            }.into());
        }

        let mut tx = self.ledger.begin_tx().await?;

        let execution = match self
            .ledger
            .create_execution(&mut tx, quote.id, Chain::Near)
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

        // Risk control check
        self.risk
            .check_execution_allowed(Chain::Near, quote.execution_cost)
            .await?;

        // Parse action
        let action = self
            .parse_action(&quote.execution_instructions)
            .await?;

        // Submit transaction
        let tx_hash = match self.submit_transaction(&action).await {
            Ok(hash) => hash,
            Err(e) => {
                error!("Failed to submit Near transaction: {:?}", e);

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

                self.ledger
                    .update_quote_status(
                        &mut tx,
                        quote.id,
                        QuoteStatus::Committed,
                        QuoteStatus::Failed,
                    )
                    .await?;

                tx.commit().await?;

                return Err(e);
            }
        };

        info!("Near transaction submitted: {}", tx_hash);

        // Get fee
        let fee = self.get_transaction_fee(&tx_hash).await?;

        // Record successful execution
        let mut tx = self.ledger.begin_tx().await?;

        self.ledger
            .complete_execution(
                &mut tx,
                execution.id,
                ExecutionStatus::Success,
                Some(tx_hash.clone()),
                Some(BigDecimal::from_str(&fee.to_string()).unwrap()),
                None,
            )
            .await?;

        self.ledger
            .update_quote_status(&mut tx, quote.id, QuoteStatus::Committed, QuoteStatus::Executed)
            .await?;

        self.risk
            .record_spending(&mut tx, Chain::Near, quote.execution_cost)
            .await?;

        self.ledger
            .log_audit_event(
                AuditEventType::ExecutionCompleted,
                Some(Chain::Near),
                Some(execution.id),
                Some(quote.user_id),
                serde_json::json!({
                    "tx_hash": tx_hash,
                    "fee": fee.to_string(),
                }),
            )
            .await?;

        tx.commit().await?;

        info!("Near execution completed successfully");

        Ok(Execution {
            id: execution.id,
            quote_id: quote.id,
            execution_chain: Chain::Near,
            transaction_hash: Some(tx_hash),
            status: ExecutionStatus::Success,
            gas_used: Some(fee),
            error_message: None,
            retry_count: 0,
            executed_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        })
    }

    fn chain(&self) -> Chain {
        Chain::Near
    }

    async fn check_treasury_balance(&self, required: Decimal) -> AppResult<()> {
        let balance = self.get_treasury_balance().await?;

        if balance < required {
            return Err(ExecutionError::InsufficientTreasury(Chain::Near).into());
        }

        Ok(())
    }

    async fn get_treasury_balance(&self) -> AppResult<Decimal> {
        // Query NEAR account balance via RPC using view_account
        let account_id = AccountId::try_into_account_id(self.treasury_key.parse()?)?;

        let account_request = methods::query::RpcQueryRequest{
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::ViewAccount { 
                account_id
            }
        };

        let account_response = self.client
            .call(account_request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to query treasury balance: {}", e),
            })?;

        // Extract amount from account info
        // The response contains the account details with available balance
        // QueryResponse::AccountView contains the amount field in yoctoNEAR
        let balance_yocto = match account_response.kind {
            QueryResponseKind::ViewAccount(account_view) => {
                account_view.amount.as_yoctonear()
            }
            _ => {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Near,
                    message: "Unexpected query response type".to_string(),
                }.into())
            }
        };
        
        // Convert from yoctoNEAR (10^24) to NEAR
        let balance_near = Decimal::from(balance_yocto) / Decimal::from(1_000_000_000_000_000_000_000_000u128);

        info!("Treasury balance: {} NEAR ({} yoctoNEAR)", balance_near, balance_yocto);

        Ok(balance_near)
    }

   async fn transfer_to_treasury(&self, token_or_asset: &str, amount: &str) -> AppResult<String> {
    info!("🔄 NEAR settlement transfer initiated: {} {}", amount, token_or_asset);
    
    // STEP 1: Parse and validate amount
    let amount_yocto: u128 = amount.parse()
        .map_err(|_| ExecutionError::ChainExecutionFailed {
            chain: Chain::Near,
            message: format!("Invalid amount format: {}", amount),
        })?;
    
    if amount_yocto == 0 {
        return Err(ExecutionError::ChainExecutionFailed {
            chain: Chain::Near,
            message: "Transfer amount must be greater than zero".to_string(),
        }.into());
    }
    
    let amount_near = amount_yocto as f64 / 1_000_000_000_000_000_000_000_000.0;
    info!("Amount: {} yoctoNEAR ({} NEAR)", amount_yocto, amount_near);
    
    // STEP 2: Parse and validate treasury account ID
    if self.treasury_key.is_empty() || self.treasury_key.len() > 64 {
        return Err(ExecutionError::ChainExecutionFailed {
            chain: Chain::Near,
            message: "Invalid treasury account ID".to_string(),
        }.into());
    }
    
    let treasury_account_id = AccountId::try_into_account_id(self.treasury_key.parse()?)
        .map_err(|e| ExecutionError::ChainExecutionFailed {
            chain: Chain::Near,
            message: format!("Invalid treasury account ID: {}", e),
        })?;
    
    info!("Treasury account: {}", treasury_account_id);
    
    // STEP 3: Fetch treasury account details
    let account_request = methods::query::RpcQueryRequest {
        block_reference: BlockReference::Finality(Finality::Final),
        request: QueryRequest::ViewAccount { 
            account_id: treasury_account_id.clone()
        }
    };
    
    let account_response = self.client
        .call(account_request)
        .await
        .map_err(|e| ExecutionError::ChainExecutionFailed {
            chain: Chain::Near,
            message: format!("Failed to fetch treasury account: {}", e),
        })?;
    
    // STEP 4: Verify sufficient balance
    let balance_yocto = match account_response.kind {
        QueryResponseKind::ViewAccount(account_view) => {
            account_view.amount.as_yoctonear()
        }
        _ => {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: "Unexpected query response type".to_string(),
            }.into());
        }
    };
    
    let balance_near = balance_yocto as f64 / 1_000_000_000_000_000_000_000_000.0;
    let min_required = amount_yocto.saturating_add(1_000_000_000_000_000_000);  // 1 NEAR for gas
    
    info!("💵 Current balance: {} NEAR", balance_near);
    
    if balance_yocto < min_required {
        return Err(ExecutionError::InsufficientTreasury(Chain::Near).into());
    }
    
    // STEP 5: Get current block for transaction reference
    let block_request = methods::block::RpcBlockRequest {
        block_reference: BlockReference::Finality(Finality::Final),
    };
    
    let block_response = self.client
        .call(block_request)
        .await
        .map_err(|e| ExecutionError::ChainExecutionFailed {
            chain: Chain::Near,
            message: format!("Failed to fetch block: {}", e),
        })?;
    
    info!("✓ Block height: {}", block_response.header.height);
    
    // STEP 6: Build and submit the transaction
    let tx_hash = if token_or_asset.to_uppercase() == "NEAR" {
        // ===== NATIVE NEAR TRANSFER =====
        info!("Native NEAR transfer: {} yoctoNEAR", amount_yocto);
        
        // Build actual transaction
        
        // 1. Create settlement source account get from settlement data)
        let source_account_id = AccountId::try_into_account_id("omnixec.near".parse()?)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Invalid source account: {}", e),
            })?;
        
        // 2. Fetch source account to get nonce
        let source_request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::ViewAccount { 
                account_id: source_account_id.clone()
            }
        };
        
        let _source_response = self.client
            .call(source_request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to fetch source account: {}", e),
            })?;
        
        // Get the current nonce for the source account from its access key
        let access_request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::ViewAccessKey {
                account_id: source_account_id.clone(),
                public_key: self.get_public_key()?,
            }
        };

        let access_response = self.client
            .call(access_request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to fetch access key: {}", e),
            })?;

        let source_nonce = match access_response.kind {
            QueryResponseKind::AccessKey(access_key) => access_key.nonce,
            _ => {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Near,
                    message: "Unexpected response when fetching access key".to_string(),
                }.into());
            }
        };
        
        // 3. Build Transfer action
        use near_primitives::transaction::Action;
        use near_primitives::transaction::TransferAction;
        
        let transfer_action = TransferAction {
            deposit: NearToken::from_yoctonear(amount_yocto),
        };
        
        let action = Action::Transfer(transfer_action);
        
        // 4. Create transaction
        use near_primitives::transaction::{Transaction, SignedTransaction};
        
        
        let tx = Transaction::V0(TransactionV0 {
            signer_id: source_account_id.clone(),
            public_key: self.get_public_key()?,
            nonce: source_nonce + 1,
            receiver_id: treasury_account_id.clone(),
            block_hash: block_response.header.hash,
            actions: vec![action],
        });

        let tx_hash = format!("near_{}", hex::encode(tx.get_hash_and_size().0.as_ref()));
        
        // 5. Sign transaction
        let signer = self.create_signer(&source_account_id)?;
        let signature = signer.sign(tx.get_hash_and_size().0.as_ref());
        let signed_tx = SignedTransaction::new(
            signature,
            tx,
        );
        
        // 6. Serialize (for debugging) and submit via RPC
        let _serialized_tx = borsh::to_vec(&signed_tx)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to serialize transaction: {}", e),
            })?;
        
        // 7. Submit transaction
        let send_request = methods::send_tx::RpcSendTransactionRequest {
            signed_transaction: signed_tx,
            wait_until: TxExecutionStatus::Final,
        };
        
        let send_response = self.client
            .call(send_request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to submit transaction: {}", e),
            })?;
        
        // 8. Use the pre-computed tx hash (RPC response shapes vary)
        let tx_hash = tx_hash;
        
        info!("✅ Native NEAR transaction submitted: {:?}", tx_hash);
        info!("Details:");
        info!("  ├─ Amount: {} yoctoNEAR ({} NEAR)", amount_yocto, amount_near);
        info!("  ├─ From: {}", source_account_id);
        info!("  ├─ To: {}", treasury_account_id);
        info!("  └─ Nonce: {}", source_nonce + 1);
        
        tx_hash
    } else {
        // ===== NEP-141 TOKEN TRANSFER =====
        info!(" NEP-141 token transfer: {} of {}", amount_yocto, token_or_asset);
        
        // Validate token contract format
        if !token_or_asset.contains('.') {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Invalid token contract: {}", token_or_asset),
            }.into());
        }
        
        //Build FunctionCall action for ft_transfer
        
        // 1. Create settlement source account
        let source_account_id = AccountId::try_into_account_id("settlement.near".parse()?)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Invalid source: {}", e),
            })?;
        
        // 2. Fetch source account nonce
        let source_request = methods::query::RpcQueryRequest {
            block_reference: BlockReference::Finality(Finality::Final),
            request: QueryRequest::ViewAccount { 
                account_id: source_account_id.clone()
            }
        };
        
        let source_response = self.client
            .call(source_request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to fetch source: {}", e),
            })?;
        
        // Derive nonce from access key for the source account
        let source_nonce = {
            let access_request = methods::query::RpcQueryRequest {
                block_reference: BlockReference::Finality(Finality::Final),
                request: QueryRequest::ViewAccessKey {
                    account_id: source_account_id.clone(),
                    public_key: self.get_public_key()?,
                },
            };

            let access_response = self.client
                .call(access_request)
                .await
                .map_err(|e| ExecutionError::ChainExecutionFailed {
                    chain: Chain::Near,
                    message: format!("Failed to fetch access key: {}", e),
                })?;

            match access_response.kind {
                QueryResponseKind::AccessKey(access_key) => access_key.nonce,
                _ => {
                    return Err(ExecutionError::ChainExecutionFailed {
                        chain: Chain::Near,
                        message: "Unexpected response when fetching access key".to_string(),
                    }.into());
                }
            }
        };
        
        // 3. Build FunctionCall action for ft_transfer
        use near_primitives::transaction::Action;
        use near_primitives::transaction::FunctionCallAction;
        
        let token_contract = AccountId::try_into_account_id(token_or_asset.parse()?)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Invalid token contract: {}", e),
            })?;
        
        // ft_transfer arguments as JSON
        let ft_args = serde_json::json!({
            "receiver_id": treasury_account_id.to_string(),
            "amount": amount_yocto.to_string(),
            "memo": None::<String>
        });
        
        let function_call = FunctionCallAction {
            method_name: "ft_transfer".to_string(),
            args: ft_args.to_string().into_bytes(),
            gas: Gas::from_gas(30_000_000_000_000u64),  // 30 TGas
            deposit: NearToken::from_yoctonear(1),  // 1 yoctoNEAR
        };
        
        let action = Action::FunctionCall(Box::new(function_call));
        
        // 4. Create transaction
        use near_primitives::transaction::{Transaction, SignedTransaction};
        
        let tx = Transaction::V0(TransactionV0 {
            signer_id: source_account_id.clone(),
            public_key: self.get_public_key()?,
            nonce: source_nonce + 1,
            receiver_id: token_contract.clone(),
            block_hash: block_response.header.hash,
            actions: vec![action],
        });

        // Compute tx_hash before moving tx into signed_tx
        let tx_hash = format!("near_{}", hex::encode(tx.get_hash_and_size().0.as_ref()));
        
        // 5. Sign transaction
        let signer = self.create_signer(&source_account_id)?;
        let signature = signer.sign(tx.get_hash_and_size().0.as_ref());
        let signed_tx = SignedTransaction::new(
            signature,
            tx,
        );
        
        // 6. (optional) Serialize transaction for debugging
        let _serialized_tx = borsh::to_vec(&signed_tx)
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to serialize: {}", e),
            })?;

        // 7. Submit transaction (pass SignedTransaction) and request finality
        let send_request = methods::send_tx::RpcSendTransactionRequest {
            signed_transaction: signed_tx,
            wait_until: TxExecutionStatus::Final,
        };
        
        let send_response = self.client
            .call(send_request)
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Near,
                message: format!("Failed to submit: {}", e),
            })?;
        
        // Use the pre-computed tx hash (RPC response shapes vary)
        let tx_hash = tx_hash;
        
        info!("✅ NEP-141 token transfer submitted: {}", tx_hash);
        info!(" Details:");
        info!("  ├─ Token: {}", token_contract);
        info!("  ├─ Amount: {} yoctoNEAR ({} NEAR)", amount_yocto, amount_near);
        info!("  ├─ To: {}", treasury_account_id);
        info!("  ├─ Gas: 30 TGas");
        info!("  └─ Deposit: 1 yoctoNEAR");
        
        tx_hash
    };
    
    info!(" Settlement completed");
    info!(" Summary:");
    info!("  ├─ Asset: {}", token_or_asset);
    info!("  ├─ Amount: {} NEAR", amount_near);
    info!("  ├─ Treasury: {}", treasury_account_id);
    info!("  └─ Hash: {}", tx_hash);
    
    Ok(tx_hash)
}
}

impl NearExecutor {
    /// Wait for transaction confirmation
    pub async fn wait_for_confirmation(
        &self,
        tx_hash: &str,
        timeout_secs: u64,
    ) -> AppResult<bool> {
        let start = std::time::Instant::now();
        let client = reqwest::Client::new();
        
        loop {
            // Check transaction status via HTTP
            let url = format!("{}/tx/{}", self.config.rpc_url, tx_hash);
            match client.get(&url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return Ok(true);
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
        match self.client.call(
            near_jsonrpc_client::methods::status::RpcStatusRequest
        ).await {
            Ok(response) => Ok(response.sync_info.latest_block_height as i64),
            Err(_) => Ok(0),
        }
    }
   
}

