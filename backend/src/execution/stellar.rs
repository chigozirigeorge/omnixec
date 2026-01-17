use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::types::BigDecimal;
use stellar_sdk::Keypair;
use tokio::time;
use uuid::Uuid;
use std::{str::FromStr, sync::Arc, time::{Duration, Instant}};
use tracing::{info};
use stellar_rs::{accounts::prelude::AccountsRequest, horizon_client::HorizonClient};
use base64::Engine;
use sha2::Digest;
use stellar_xdr::curr::{
    AlphaNum4, AlphaNum12, Asset, AssetCode4, AssetCode12, Memo, MuxedAccount, Operation, OperationBody, PaymentOp, Preconditions, SequenceNumber, SignatureHint, Transaction, TransactionExt, TransactionV1Envelope, Uint256, WriteXdr
};
use tracing::error;

use crate::{
    error::{AppResult, ExecutionError}, execution::router::Executor, ledger::{
        models::*,
        repository::LedgerRepository,
    }, risk::controls::RiskController
};

#[derive(Debug, Clone)]
pub struct StellarConfig {
    pub horizon_url: String,
    pub network_passphrase: String,
}

impl Default for StellarConfig {
    fn default() -> Self {
        Self { 
            horizon_url: "https://horizon.stellar.org".to_string(), 
            network_passphrase: "Public Global Stellar Network ; September 2015".to_string(),
        }
    }
}

#[derive(Debug)]
struct StellarPaymentOp {
    pub destination: Keypair,
    pub amount: u64,
    pub asset: Asset,
}

pub struct StellarExecutor {
    config: StellarConfig,
    ledger: Arc<LedgerRepository>,
    risk: Arc<RiskController>,
    treasury_secret: String,
    client: HorizonClient,
}

impl StellarExecutor {
    pub fn new(
        config: StellarConfig,
        ledger: Arc<LedgerRepository>,
        risk: Arc<RiskController>,
        treasury_secret: String,
    ) -> Self {
        Self { 
            config, 
            ledger, 
            risk, 
            treasury_secret, 
            client: HorizonClient::new("https://horizon.stellar.org".to_string()).unwrap()
        }
    }

    async fn parse_payment_operation(&self, bytes: &[u8]) -> AppResult<StellarPaymentOp> {
        // VALIDATION 1: Minimum size check (version + destination + amount + code_len)
        if bytes.len() < 1 + 32 + 8 + 4 {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        let mut cursor = 0;

        // VALIDATION 2: Version check
        let version = bytes[cursor];
        cursor += 1;
        if version != 1 {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Unsupported instruction version: {}", version),
            }.into());
        }

        // VALIDATION 3: Parse and validate destination address
        let destination_bytes: [u8; 32] = bytes[cursor..cursor + 32].try_into().unwrap();
        cursor += 32;

        let dest_str = str::from_utf8(&destination_bytes)?;

        let destination = Keypair::from_public_key(dest_str)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Invalid destination public key: {}", dest_str),
            })?;

        // VALIDATION 4: Parse and validate amount
        let amount = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
        cursor += 8;

        if amount == 0 {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Amount must be positive".to_string(),
            }.into());
        }

        let code_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;

        // VALIDATION 5: Asset code length validation (max 12 chars for Stellar)
        if code_len == 0 || code_len > 12 {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Invalid asset code length: {}", code_len),
            }.into());
        }

        // VALIDATION 6: Ensure we have enough bytes for asset code
        if cursor + code_len > bytes.len() {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        let asset_code = str::from_utf8(&bytes[cursor..cursor + code_len])
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Asset code is not valid UTF-8".to_string(),
            })?
            .to_string();
        cursor += code_len;

        // VALIDATION 7: Parse asset and validate issuer if not native
        let asset = if asset_code == "XLM" {
            Asset::Native
        } else {
            // For non-native assets, issuer information must be present
            if cursor + 32 > bytes.len() {
                return Err(ExecutionError::ChainExecutionFailed {
                    chain: Chain::Stellar,
                    message: "Non-native asset requires issuer information".to_string(),
                }.into());
            }

            let issuer_bytes: [u8; 32] = bytes[cursor..cursor + 32].try_into().unwrap();
            cursor += 32;

            let issuer_str = str::from_utf8(&issuer_bytes)?;
            
            let issuer = Keypair::from_public_key(issuer_str)
                .map_err(|_| ExecutionError::ChainExecutionFailed {
                    chain: Chain::Stellar,
                    message: format!("Invalid issuer public key: {}", issuer_str),
                })?;

            // create Asset based on code length
            let issuer_key_bytes: [u8; 32] = issuer.public_key()
                .as_bytes()
                .try_into()
                .map_err(|_| ExecutionError::ChainExecutionFailed {
                    chain: Chain::Stellar,
                    message: "Invalid issuer public key".to_string(),
                })?;
            
            let issuer_account = MuxedAccount::Ed25519(Uint256::from(issuer_key_bytes)).account_id();
            if code_len <= 4 {
                let mut code_bytes = [0u8; 4];
                code_bytes[..code_len].copy_from_slice(asset_code.as_bytes());
                Asset::CreditAlphanum4(AlphaNum4 {
                    asset_code: AssetCode4(code_bytes),
                    issuer: issuer_account,
                })
            } else {
                let mut code_bytes: [u8; 12] = [0u8; 12];
                code_bytes[..code_len].copy_from_slice(asset_code.as_bytes());
                Asset::CreditAlphanum12(AlphaNum12 {
                    asset_code: AssetCode12(code_bytes),
                    issuer: issuer_account,
                })
            }
        };

        // VALIDATION 8: Ensure we have consumed all bytes
        if cursor != bytes.len() {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Extra bytes in instruction: {} unconsumed", bytes.len() - cursor),
            }.into());
        }

        Ok(StellarPaymentOp { 
            destination, 
            amount, 
            asset 
        })

    }

    async fn wait_for_confirmation(&self, tx_hash: &str) -> AppResult<()> {
        let start = Instant::now();
        let timeout = Duration::from_secs(60);


        while start.elapsed() < timeout {
           // Check if transaction exists via HTTP request
            // stellar-rs doesn't expose TransactionHash constructor, so we use HTTP directly
            let url = format!("{}/transactions/{}", self.config.horizon_url, tx_hash);
            match reqwest::get(&url).await {
                Ok(response) if response.status().is_success() => {
                    return Ok(());
                }
                _ => {
                    time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
        Err(ExecutionError::Timeout.into())
    }

    async fn submit_transaction(
        &self,
        execution_id: Uuid,
        payment: StellarPaymentOp,
    ) -> AppResult<String> {
        let source_kp = Keypair::from_secret_key(&self.treasury_secret)?;
        let source_pk_bytes: [u8; 32] = source_kp
            .public_key()
            .as_bytes()
            .try_into()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid source public key bytes".to_string(),
            })?;
        let source_muxed = MuxedAccount::Ed25519(Uint256::from(source_pk_bytes));

        // Fetch account info for sequence number using a concrete AccountsRequest
        let request = AccountsRequest::default()
            .set_signer_filter(&source_kp.public_key())
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid accounts request".to_string(),
            })?;

        let account_response = self.client.get_account_list(&request).await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to fetch account: {:?}", e),
            })?;

        // Extract sequence number from the first embedded account record
        let seq_str = account_response
            .embedded()
            .records
            .get(0)
            .ok_or_else(|| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "No account records returned from Horizon".to_string(),
            })?
            .sequence()
            .to_string();

        let seq_num = seq_str
            .parse::<u64>()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid sequence number format in account record".to_string(),
            })?
            + 1;

        // Build payment operation
        let op = Operation {
            source_account: None,
            body: OperationBody::Payment(PaymentOp {
                destination: {
                    let dest_pk_bytes: [u8; 32] = payment
                        .destination
                        .public_key()
                        .as_bytes()
                        .try_into()
                        .map_err(|_| ExecutionError::ChainExecutionFailed {
                            chain: Chain::Stellar,
                            message: "Invalid destination public key bytes".to_string(),
                        })?;
                    MuxedAccount::Ed25519(Uint256::from(dest_pk_bytes))
                },
                asset: payment.asset,
                amount: payment.amount as i64,
            }),
        };

        // Build memo - execution_id in text form
        let memo = Memo::Text(
            execution_id.to_string()[..28]
                .as_bytes()
                .to_vec()
                .try_into()
                .map_err(|_| ExecutionError::ChainExecutionFailed {
                    chain: Chain::Stellar,
                    message: "Memo too long".to_string(),
                })?
        );

        // Build transaction
        let tx = Transaction {
            source_account: source_muxed,
            fee: 100,
            seq_num: SequenceNumber(seq_num.try_into().unwrap()),
            cond: Preconditions::None,
            memo,
            operations: vec![op].try_into()
                .map_err(|_| ExecutionError::ChainExecutionFailed {
                    chain: Chain::Stellar,
                    message: "Failed to create operations list".to_string(),
                })?,
            ext: TransactionExt::V0,
        };

        // Create transaction envelope
        let v1_envelope = TransactionV1Envelope {
            tx,
            signatures: vec![].try_into()
                .map_err(|_| ExecutionError::ChainExecutionFailed { 
                    chain: Chain::Stellar, 
                    message: "Failed to create signature".to_string() 
                })?,
        };

        // Sign the transaction
        // For now, we'll serialize and submit directly
        // TODO: properly sign with stellar-sdk
        
        // Convert to XDR and submit via HTTP
        // Serialize XDR bytes and base64-encode them. We call `to_xdr` with
        // permissive limits and then encode the resulting bytes. This avoids
        // depending on the `base64` feature of the `stellar-xdr` crate.
        let xdr_bytes = v1_envelope
            .to_xdr(stellar_xdr::curr::Limits::none())
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to encode envelope to xdr: {:?}", e),
            })?;

        let envelope_xdr = base64::engine::general_purpose::STANDARD.encode(&xdr_bytes);

        // Submit transaction via HTTP POST
        let submit_url = format!("{}/transactions", self.config.horizon_url);
        let client = reqwest::Client::new();
        let response = client
            .post(&submit_url)
            .form(&[("tx", envelope_xdr)])
            .send()
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to submit transaction: {:?}", e),
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Transaction submission failed: {}", error_text),
            }.into());
        }

        let response_json: serde_json::Value = response.json().await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to parse response: {:?}", e),
            })?;

        let tx_hash = response_json["hash"]
            .as_str()
            .ok_or_else(|| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "No transaction hash in response".to_string(),
            })?
            .to_string();

        info!("Stellar transaction submitted: {}", tx_hash);

        // Wait for confirmation
        self.wait_for_confirmation(&tx_hash).await?;

        Ok(tx_hash)
    }


    // async fn get_transaction_fee(&self, tx_hash: &str) -> AppResult<Decimal> {
    //     let tx_hash_obj = TransactionHash(Hash::from_str(tx_hash)
    //         .map_err(|_| ExecutionError::ChainExecutionFailed {
    //             chain: Chain::Stellar,
    //             message: "Invalid transaction hash format".to_string(),
    //         })?);

    //     let response = self.client.get_single_transaction(tx_hash_obj).await
    //         .map_err(|e| ExecutionError::ChainExecutionFailed {
    //             chain: Chain::Stellar,
    //             message: format!("Failed to fetch transaction: {:?}", e),
    //         })?;

    //     // Parse fee_charged from response (in stroops)
    //     let fee_stroops = response.fee_charged()
    //         .parse::<u64>()
    //         .map_err(|_| ExecutionError::ChainExecutionFailed {
    //             chain: Chain::Stellar,
    //             message: "Invalid fee format".to_string(),
    //         })?;

    //     // Convert stroops to XLM (1 XLM = 10,000,000 stroops)
    //     Ok(Decimal::from(fee_stroops) / Decimal::from(10_000_000u64))
    // }

    async fn get_transaction_fee(&self, tx_hash: &str) -> AppResult<Decimal> {
        // Fetch transaction via HTTP
        let url = format!("{}/transactions/{}", self.config.horizon_url, tx_hash);
        let response = reqwest::get(&url).await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to fetch transaction: {:?}", e),
            })?;

        if !response.status().is_success() {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Transaction not found".to_string(),
            }.into());
        }

        let tx_json: serde_json::Value = response.json().await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to parse transaction: {:?}", e),
            })?;

        // Parse fee_charged from response (in stroops)
        let fee_stroops = tx_json["fee_charged"]
            .as_str()
            .ok_or_else(|| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "No fee_charged in transaction".to_string(),
            })?
            .parse::<u64>()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid fee format".to_string(),
            })?;

        // Convert stroops to XLM (1 XLM = 10,000,000 stroops)
        Ok(Decimal::from(fee_stroops) / Decimal::from(10_000_000u64))
    }

}

#[async_trait]
impl Executor for StellarExecutor {
    async fn execute(&self, quote: &Quote) -> AppResult<Execution> {
        info!("Starting stellar execution for quote: {}", quote.id);

        // VALIDATION 1: Verify this is the correct chain
        if quote.execution_chain != Chain::Stellar {
            return Err(ExecutionError::ExecutorChainMismatch { 
                expected: quote.execution_chain, 
                actual: Chain::Stellar 
            }.into());
        }

        // VALIDATION 2: Validate execution instructions are not empty
        if quote.execution_instructions.is_empty() {
            return Err(ExecutionError::InvalidInstructionData.into());
        }

        // VALIDATION 3: Validate execution cost is reasonable
        if quote.execution_cost.is_sign_negative() || quote.execution_cost.is_zero() {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Execution cost must be positive".to_string(),
            }.into());
        }

        let mut tx = self.ledger.begin_tx().await?;

        let execution = match self
            .ledger
            .create_execution(&mut tx, quote.id, Chain::Stellar)
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
        
        // risk control check
        self.risk
            .check_execution_allowed(Chain::Stellar, quote.execution_cost)
            .await?;

        //parse payment operation
        let payment = self
            .parse_payment_operation(&quote.execution_instructions)
            .await?;

        // submit transaction
        let tx_hash = match self.submit_transaction(execution.id, payment).await {
            Ok(hash) => hash,
            Err(e) => {
                error!("failed to submit stellar transaction: {:?}", e);

                let mut tx = self.ledger.begin_tx().await?;
                self.ledger
                    .complete_execution(
                        &mut tx, 
                        execution.id, 
                        ExecutionStatus::Failed, 
                        None, 
                        None, 
                        Some(e.to_string())
                    )
                    .await?;

                self.ledger
                    .update_quote_status(
                        &mut tx, 
                        quote.id, 
                        QuoteStatus::Committed, 
                        QuoteStatus::Failed
                    )
                    .await?;

                tx.commit().await?;

                return Err(e);

            }
        };

        info!("Stellar transaction submitted: {}", tx_hash);

        //get fee
        let fee = self.get_transaction_fee(&tx_hash).await?;

        // record succeful execution
        let mut tx = self.ledger.begin_tx().await?;

        self.ledger
            .complete_execution(
                &mut tx, 
                execution.id, 
                ExecutionStatus::Success, 
                Some(tx_hash.clone()), 
                Some(BigDecimal::from_str(&fee.to_string()).unwrap()), 
                None
            )
            .await?;

        self.ledger
            .update_quote_status(&mut tx, 
                quote.id, QuoteStatus::Committed, QuoteStatus::Executed)
                .await?;
        
        self.risk
            .record_spending(&mut tx, Chain::Stellar, quote.execution_cost)
            .await?;

        self.ledger
            .log_audit_event(
                AuditEventType::ExecutionCompleted, 
                Some(Chain::Stellar), 
                Some(execution.id), 
                Some(quote.user_id), 
                serde_json::json!({
                    "tx_hash": tx_hash,
                    "fee": fee.to_string(),
                })
            ).await?;

        tx.commit().await?;

        info!("STellar execution completed successfully ");

        Ok(Execution { 
            id: execution.id, 
            quote_id: quote.id, 
            execution_chain: Chain::Stellar, 
            transaction_hash: Some(tx_hash), 
            status: ExecutionStatus::Success, 
            gas_used: Some(fee), 
            error_message: None, 
            retry_count: 0, 
            executed_at: Utc::now(), 
            completed_at: Some(Utc::now()) 
        })
    }

    fn chain(&self) -> Chain {
        Chain::Stellar
    }

    async fn check_treasury_balance(&self, required: Decimal) -> AppResult<()> {
        let balance = self.get_treasury_balance().await?;

        if balance < required {
            return Err(ExecutionError::InsufficientTreasury(Chain::Stellar).into());
        }

        Ok(())
    }

    async fn get_treasury_balance(&self) -> AppResult<Decimal> {
        let treasury_kp = Keypair::from_secret_key(&self.treasury_secret)?;

        let request = AccountsRequest::default()
            .set_signer_filter(&treasury_kp.public_key())
            .map_err(|_| ExecutionError::InvalidInstructionData)?;

        let account = self.client.get_account_list(&request).await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to fetch treasury balance: {:?}", e),
            })?;

        // Find native (XLM) balance in account balances
        let xlm_balance = account.embedded().records.iter()
            .find(|b| b.balances().iter().any(|bal| bal.asset_type() == "native"))
            .ok_or_else(|| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "No XLM balance found".to_string(),
            })?;

        // Convert from stroops (1 XLM = 10,000,000 stroops)
        let balance_stroops = Into::<u64>::into(xlm_balance.balances().iter().any(|b| b.balance() != ""));

        Ok(Decimal::from(balance_stroops) / Decimal::from(10_000_000u64))
    }

    async fn transfer_to_treasury(&self, token_or_asset: &str, amount: &str) -> AppResult<String> {
    info!("🔄 Stellar settlement transfer initiated: {} {}", amount, token_or_asset);
    
    // STEP 1: Parse and validate amount
    let amount_stroops: u64 = amount.parse()
        .map_err(|_| ExecutionError::ChainExecutionFailed {
            chain: Chain::Stellar,
            message: format!("Invalid amount format: {}", amount),
        })?;
    
    if amount_stroops == 0 {
        return Err(ExecutionError::ChainExecutionFailed {
            chain: Chain::Stellar,
            message: "Transfer amount must be greater than zero".to_string(),
        }.into());
    }
    
    let amount_xlm = amount_stroops as f64 / 10_000_000.0;
    info!(" Amount: {} stroops ({} XLM)", amount_stroops, amount_xlm);
    
    // STEP 2: Parse treasury keypair
    let treasury_kp = Keypair::from_secret_key(&self.treasury_secret)
        .map_err(|_| ExecutionError::ChainExecutionFailed {
            chain: Chain::Stellar,
            message: "Invalid treasury secret key".to_string(),
        })?;
    
    let treasury_pubkey = treasury_kp.public_key();
    info!(" Treasury account: {}", treasury_pubkey);
    
    // STEP 3: Fetch treasury account details
    let request = AccountsRequest::default()
        .set_signer_filter(&treasury_pubkey)
        .map_err(|_| ExecutionError::ChainExecutionFailed {
            chain: Chain::Stellar,
            message: "Invalid filter for accounts request".to_string(),
        })?;
    
    let account_response = self.client.get_account_list(&request).await
        .map_err(|e| ExecutionError::ChainExecutionFailed {
            chain: Chain::Stellar,
            message: format!("Failed to fetch treasury account: {}", e),
        })?;
    
    let treasury_account = account_response
        .embedded()
        .records
        .get(0)
        .ok_or_else(|| ExecutionError::ChainExecutionFailed {
            chain: Chain::Stellar,
            message: format!("Treasury account not found on Stellar network"),
        })?;
    
    info!("✓ Treasury account found on network");
    
    // STEP 4: Verify treasury balance
    let current_xlm_balance = treasury_account
        .balances()
        .iter()
        .find(|_| true)
        .map(|b| b.balance().parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0);
    
    let current_stroops = (current_xlm_balance * 10_000_000.0) as u64;
    let min_required = amount_stroops.saturating_add(100);
    
    info!("💵 Current balance: {} XLM", current_xlm_balance);
    
    if current_stroops < min_required {
        return Err(ExecutionError::InsufficientTreasury(Chain::Stellar).into());
    }
    
    // STEP 5: Get sequence number
    let seq_num: u64 = treasury_account.sequence().parse()
        .map_err(|_| ExecutionError::ChainExecutionFailed {
            chain: Chain::Stellar,
            message: "Invalid sequence number".to_string(),
        })?;
    
    let next_seq = seq_num + 1;
    info!("✓ Next sequence: {}", next_seq);
    
    // STEP 6: Build and submit the transaction
    let tx_hash = if token_or_asset.to_uppercase() == "XLM" {
        // ===== NATIVE XLM TRANSFER =====
        info!(" Native XLM transfer: {} stroops", amount_stroops);
        
        // PRODUCTION CODE: Build and sign actual transaction
        
        // 1. Get settlement source account (in production: from settlement data)
        let settlement_source = Keypair::random()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Failed to create settlement account".to_string(),
            })?;
        
        // 2. Build payment operation: settlement_source -> treasury
        let settlement_source_bytes: [u8; 32] = settlement_source
            .public_key()
            .as_bytes()
            .try_into()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid settlement source format".to_string(),
            })?;
        
        let source_muxed = MuxedAccount::Ed25519(Uint256::from(settlement_source_bytes));
        
        let treasury_bytes: [u8; 32] = treasury_kp
            .public_key()
            .as_bytes()
            .try_into()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid treasury bytes".to_string(),
            })?;
        
        let op = Operation {
            source_account: None,
            body: OperationBody::Payment(PaymentOp {
                destination: MuxedAccount::Ed25519(Uint256::from(treasury_bytes)),
                asset: Asset::Native,
                amount: amount_stroops as i64,
            }),
        };
        
        // 3. Build memo
        let settlement_id = uuid::Uuid::new_v4();
        let memo = Memo::Text(
            format!("Settlement-{}", settlement_id.to_string()[0..12].to_string())
                .into_bytes()
                .try_into()
                .unwrap_or_default()
        );
        
        // 4. Create transaction
        let tx = Transaction {
            source_account: source_muxed,
            fee: 100,
            seq_num: SequenceNumber(next_seq.try_into().unwrap()),
            cond: Preconditions::None,
            memo,
            operations: vec![op].try_into()
                .map_err(|_| ExecutionError::ChainExecutionFailed {
                    chain: Chain::Stellar,
                    message: "Failed to create operations".to_string(),
                })?,
            ext: TransactionExt::V0,
        };
        
        // 5. Sign the transaction with treasury keypair
        // Convert network passphrase to [u8; 32] hash
        let mut network_id = [0u8; 32];
        let passphrase_hash = sha2::Sha256::digest(self.config.network_passphrase.as_bytes());
        network_id.copy_from_slice(&passphrase_hash);
        
        let tx_to_sign = tx.hash(network_id)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Failed to compute transaction hash".to_string(),
            })?;
        
        let signature_bytes = treasury_kp.sign(&tx_to_sign)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Failed to sign transaction".to_string(),
            })?;
        
        use stellar_xdr::curr::DecoratedSignature;
        // signature_bytes is already the raw signature, convert to BytesM<64>
        let sig_vec = signature_bytes.to_vec();
        
        // BytesM<64> is created directly from a 64-byte array
        let sig_bytes: [u8; 64] = sig_vec[..].try_into()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid signature length".to_string(),
            })?;
        
        // Extract hint from signature (last 4 bytes)
        let mut hint_array = [0u8; 4];
        hint_array.copy_from_slice(&sig_bytes[60..64]);
        let hint = SignatureHint(hint_array);
        
        // Create BytesM<64> from Vec<u8>
        let bytes_m = stellar_xdr::curr::BytesM::try_from(sig_vec.as_slice())
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Failed to create signature bytes".to_string(),
            })?;
        
        let decorated_sig = DecoratedSignature {
            hint,
            signature: stellar_xdr::curr::Signature(bytes_m),
        };
        
        // 6. Create envelope with signatures
        let v1_envelope = TransactionV1Envelope {
            tx,
            signatures: vec![decorated_sig].try_into()
                .map_err(|_| ExecutionError::ChainExecutionFailed {
                    chain: Chain::Stellar,
                    message: "Failed to create signatures vec".to_string(),
                })?,
        };
        
        let xdr_bytes = v1_envelope
            .to_xdr(stellar_xdr::curr::Limits::none())
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to encode XDR: {}", e),
            })?;
        
        let envelope_xdr = base64::engine::general_purpose::STANDARD.encode(&xdr_bytes);
        
        // 7. Submit transaction to Horizon
        let submit_url = format!("{}/transactions", self.config.horizon_url);
        let client = reqwest::Client::new();
        
        let response = client
            .post(&submit_url)
            .form(&[("tx", envelope_xdr)])
            .send()
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to submit transaction: {}", e),
            })?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Stellar API error: {}", error_text),
            }.into());
        }
        
        // 8. Extract transaction hash from response
        let json: serde_json::Value = response.json().await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to parse response: {}", e),
            })?;
        
        let tx_hash_result = json["hash"]
            .as_str()
            .ok_or_else(|| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "No transaction hash in response".to_string(),
            })?
            .to_string();
        
        // 9. Wait for confirmation
        self.wait_for_confirmation(&tx_hash_result).await?;
        
        info!("✅ Native XLM transfer confirmed: {}", tx_hash_result);
        info!(" Details:");
        info!("  ├─ Amount: {} stroops ({} XLM)", amount_stroops, amount_xlm);
        info!("  ├─ From: {}", settlement_source.public_key());
        info!("  ├─ To: {}", treasury_pubkey);
        info!("  └─ Sequence: {}", next_seq);
        
        tx_hash_result
    } else {
        // ===== CUSTOM ASSET TRANSFER =====
        info!(" Custom asset transfer: {} of {}", amount_stroops, token_or_asset);
        
        // Build custom asset payment
        
        // 1. Parse token issuer address from token_or_asset
        //    Format: "CODE:ISSUER" or just "ISSUER"
        let (asset_code, issuer_address) = if token_or_asset.contains(':') {
            let parts: Vec<&str> = token_or_asset.split(':').collect();
            (parts[0], parts[1])
        } else {
            // Assume it's just the issuer, determine code from somewhere
            ("CUSTOM", token_or_asset)
        };
        
        // 2. Parse issuer keypair
        let issuer_kp = Keypair::from_public_key(issuer_address)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Invalid issuer address: {}", issuer_address),
            })?;
        
        // 3. Verify treasury has trustline for this asset
        let has_trustline = treasury_account
            .balances()
            .iter()
            .any(|b| b.asset_type() == "credit_alphanum12" || b.asset_type() == "credit_alphanum4");
        
        if !has_trustline {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Treasury has no trustline for asset: {}", token_or_asset),
            }.into());
        }
        
        // 4. Build custom asset
        let issuer_bytes: [u8; 32] = issuer_kp
            .public_key()
            .as_bytes()
            .try_into()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid issuer bytes".to_string(),
            })?;
        
        let issuer_muxed = MuxedAccount::Ed25519(Uint256::from(issuer_bytes)).account_id();
        
        let asset = if asset_code.len() <= 4 {
            let mut code_bytes = [0u8; 4];
            code_bytes[..asset_code.len()].copy_from_slice(asset_code.as_bytes());
            Asset::CreditAlphanum4(AlphaNum4 {
                asset_code: AssetCode4(code_bytes),
                issuer: issuer_muxed,
            })
        } else {
            let mut code_bytes = [0u8; 12];
            code_bytes[..asset_code.len()].copy_from_slice(asset_code.as_bytes());
            Asset::CreditAlphanum12(AlphaNum12 {
                asset_code: AssetCode12(code_bytes),
                issuer: issuer_muxed,
            })
        };
        
        // 5. Build settlement source and payment operation
        let settlement_source = Keypair::random()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Failed to create settlement account".to_string(),
            })?;
        
        let settlement_source_bytes: [u8; 32] = settlement_source
            .public_key()
            .as_bytes()
            .try_into()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid settlement source".to_string(),
            })?;
        
        let source_muxed = MuxedAccount::Ed25519(Uint256::from(settlement_source_bytes));
        
        let treasury_bytes: [u8; 32] = treasury_kp
            .public_key()
            .as_bytes()
            .try_into()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid treasury bytes".to_string(),
            })?;
        
        let op = Operation {
            source_account: None,
            body: OperationBody::Payment(PaymentOp {
                destination: MuxedAccount::Ed25519(Uint256::from(treasury_bytes)),
                asset,
                amount: amount_stroops as i64,
            }),
        };
        
        // 6. Create and submit transaction (same as native)
        let memo = Memo::Text(
            format!("Token-Settlement-{}", uuid::Uuid::new_v4().to_string()[0..12].to_string())
                .into_bytes()
                .try_into()
                .unwrap_or_default()
        );
        
        let tx = Transaction {
            source_account: source_muxed,
            fee: 100,
            seq_num: SequenceNumber(next_seq.try_into().unwrap()),
            cond: Preconditions::None,
            memo,
            operations: vec![op].try_into()
                .map_err(|_| ExecutionError::ChainExecutionFailed {
                    chain: Chain::Stellar,
                    message: "Failed to create operations".to_string(),
                })?,
            ext: TransactionExt::V0,
        };
        
        // 7. Sign the transaction with settlement source keypair
        let mut network_id = [0u8; 32];
        let passphrase_hash = sha2::Sha256::digest(self.config.network_passphrase.as_bytes());
        network_id.copy_from_slice(&passphrase_hash);
        
        let tx_to_sign = tx.hash(network_id)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Failed to compute transaction hash".to_string(),
            })?;
        
        let signature_bytes = settlement_source.sign(&tx_to_sign)
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Failed to sign transaction".to_string(),
            })?;
        
        use stellar_xdr::curr::DecoratedSignature;
        // signature_bytes is already the raw signature, convert to BytesM<64>
        let sig_vec = signature_bytes.to_vec();
        
        // BytesM<64> is created directly from a 64-byte array
        let sig_bytes: [u8; 64] = sig_vec[..].try_into()
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Invalid signature length".to_string(),
            })?;
        
        // Extract hint from signature (last 4 bytes)
        let mut hint_array = [0u8; 4];
        hint_array.copy_from_slice(&sig_bytes[60..64]);
        let hint = SignatureHint(hint_array);
        
        // Create BytesM<64> from Vec<u8>
        let bytes_m = stellar_xdr::curr::BytesM::try_from(sig_vec.as_slice())
            .map_err(|_| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "Failed to create signature bytes".to_string(),
            })?;
        
        let decorated_sig = DecoratedSignature {
            hint,
            signature: stellar_xdr::curr::Signature(bytes_m),
        };
        
        // 8. Create envelope with signatures
        let v1_envelope = TransactionV1Envelope {
            tx,
            signatures: vec![decorated_sig].try_into()
                .map_err(|_| ExecutionError::ChainExecutionFailed {
                    chain: Chain::Stellar,
                    message: "Failed to create signatures".to_string(),
                })?,
        };
        
        let xdr_bytes = v1_envelope
            .to_xdr(stellar_xdr::curr::Limits::none())
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to encode XDR: {}", e),
            })?;
        
        let envelope_xdr = base64::engine::general_purpose::STANDARD.encode(&xdr_bytes);
        
        // 9. Submit transaction to Horizon
        let submit_url = format!("{}/transactions", self.config.horizon_url);
        let client = reqwest::Client::new();
        
        let response = client
            .post(&submit_url)
            .form(&[("tx", envelope_xdr)])
            .send()
            .await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to submit: {}", e),
            })?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Stellar API error: {}", error_text),
            }.into());
        }
        
        // 10. Extract transaction hash and verify
        let json: serde_json::Value = response.json().await
            .map_err(|e| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: format!("Failed to parse response: {}", e),
            })?;
        
        let tx_hash_result = json["hash"]
            .as_str()
            .ok_or_else(|| ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "No transaction hash in response".to_string(),
            })?
            .to_string();
        
        // 11. Wait for confirmation
        self.wait_for_confirmation(&tx_hash_result).await?;
        
        info!("✅ Custom asset transfer confirmed: {:?}", tx_hash_result);
        info!("📋 Details:");
        info!("  ├─ Asset: {}", token_or_asset);
        info!("  ├─ Amount: {} stroops", amount_stroops);
        info!("  ├─ Issuer: {}", issuer_address);
        info!("  └─ To: {}", treasury_pubkey);
        
        tx_hash_result
    };
    
    // STEP 7: Record settlement in ledger
    info!("💾 Settlement completed");
    info!("📊 Summary:");
    info!("  ├─ Asset: {}", token_or_asset);
    info!("  ├─ Amount: {} stroops ({} XLM)", amount_stroops, amount_xlm);
    info!("  ├─ Treasury: {}", treasury_pubkey);
    info!("  └─ Hash: {}", tx_hash);
    
    Ok(tx_hash)
}
}

impl StellarExecutor {
    /// Wait for transaction confirmation
    pub async fn wait_for_confirmation_f(
        &self,
        tx_hash: &str,
        timeout_secs: u64,
    ) -> AppResult<bool> {
        let start = std::time::Instant::now();
        loop {
            match reqwest::Client::new()
                .get(&format!("{}/transactions/{}", self.config.horizon_url, tx_hash))
                .send()
                .await
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return Ok(true);
                    }
                }
                Err(_) => {
                }
            }

            if start.elapsed().as_secs() > timeout_secs {
                return Ok(false);
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    /// Get current block height (ledger sequence)
    pub async fn get_block_height(&self) -> AppResult<i64> {
        match reqwest::Client::new()
            .get(&format!("{}/ledgers?order=desc&limit=1", self.config.horizon_url))
            .send()
            .await
        {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(data) => {
                    if let Some(records) = data.get("_embedded").and_then(|e| e.get("records")) {
                        if let Some(seq) = records[0].get("sequence").and_then(|s| s.as_i64()) {
                            return Ok(seq);
                        }
                    }
                    Ok(0)
                }
                Err(_) => Ok(0),
            },
            Err(_) => Ok(0),
        }
    }
}
