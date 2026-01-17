/// Token Approval API endpoints
/// Implements the complete flow:
/// 1. Create approval (generate message for user to sign)
/// 2. Submit approval (verify signature and execute transfer)
/// 3. Check approval status

use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    execution::{
        SolanaSignatureVerifier, StellarSignatureVerifier, NearSignatureVerifier,
        SignatureVerifier,
    },
    ledger::models::*,
    api::handler::AppState,
    execution::router::Executor,
};


/// POST /approval/create
/// Creates a new token approval request
/// 
/// Returns a message that the user must sign with their wallet
pub async fn create_token_approval(
    State(app_state): State<AppState>,
    Json(req): Json<CreateTokenApprovalRequest>,
) -> AppResult<(StatusCode, Json<CreateTokenApprovalResponse>)> {
    // 1. Validate quote exists and is still valid
    let quote = app_state
        .ledger
        .get_quote(req.quote_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Quote not found: {}", req.quote_id)))?;

    if quote.status != QuoteStatus::Pending {
        return Err(AppError::Internal(
            "Quote must be in Pending status".to_string(),
        ));
    }

    if Utc::now() > quote.expires_at {
        return Err(AppError::Internal("Quote has expired".to_string()));
    }

    // 2. Parse funding chain
    let funding_chain = match req.funding_chain.to_lowercase().as_str() {
        "solana" => Chain::Solana,
        "stellar" => Chain::Stellar,
        "near" => Chain::Near,
        _ => return Err(AppError::UnsupportedChain(req.funding_chain)),
    };

    // 3. Validate user has verified wallet on funding chain
    
    // TODO:For now, assume it's verified; in production add wallet verification check

    // 4. Generate unique nonce
    let nonce = Uuid::new_v4().to_string();

    // 5. Check nonce hasn't been used (replay protection)
    if app_state.ledger.is_nonce_used(&nonce).await? {
        return Err(AppError::Internal("Nonce collision - retry".to_string()));
    }

    // 6. Create message for user to sign
    let expires_at = Utc::now() + chrono::Duration::minutes(15);
    let message = format!(
        "APPROVE_TRANSFER\n\
         Token: {}\n\
         Amount: {}\n\
         Recipient: {}\n\
         Quote ID: {}\n\
         Nonce: {}\n\
         Expires: {}\n\
         \n\
         Only sign this message if you trust the recipient and the amount.",
        req.token.to_uppercase(),
        req.amount,
        req.recipient,
        req.quote_id,
        nonce,
        expires_at.to_rfc3339()
    );

    // 7. Create approval record in database
    let approval = TokenApproval {
        id: Uuid::new_v4(),
        quote_id: req.quote_id,
        user_id: req.user_id,
        funding_chain,
        token: req.token,
        amount: req.amount.parse().map_err(|_| {
            AppError::InvalidInput("Invalid amount format".to_string())
        })?,
        recipient: req.recipient,
        message: message.clone(),
        nonce: nonce.clone(),
        signature: None,
        user_wallet: String::new(), // Will be filled on submission
        status: "pending".to_string(),
        transaction_hash: None,
        block_height: None,
        confirmation_status: None,
        created_at: Utc::now(),
        expires_at,
        submitted_at: None,
        confirmed_at: None,
        executed_at: None,
        failed_at: None,
        error_message: None,
        error_code: None,
        retry_count: 0,
        last_retry_at: None,
    };

    app_state.ledger.create_token_approval(&approval).await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateTokenApprovalResponse {
            approval_id: approval.id,
            message_to_sign: message,
            nonce,
            expires_at,
        }),
    ))
}

/// POST /approval/submit
/// Submits a signed approval and executes the transfer
/// 
/// Verifies the signature, executes the transfer, and returns the transaction hash
pub async fn submit_token_approval(
    State(app_state): State<AppState>,
    Json(req): Json<SubmitTokenApprovalRequest>,
) -> AppResult<(StatusCode, Json<SubmitTokenApprovalResponse>)> {
    // 1. Get approval record
    let approval = app_state.ledger.get_token_approval(&req.approval_id).await?;

    // 2. Validate approval is still pending
    if approval.status != "pending" {
        return Err(AppError::Internal(format!(
            "Approval is in {} status, expected pending",
            approval.status
        )));
    }

    // 3. Validate not expired
    if Utc::now() > approval.expires_at {
        app_state
            .ledger
            .update_token_approval_failed(
                &req.approval_id,
                "Approval expired",
                "APPROVAL_EXPIRED",
            )
            .await?;
        return Err(AppError::Internal("Approval has expired".to_string()));
    }

    // 4. Validate nonce hasn't been reused (replay protection)
    if app_state.ledger.is_nonce_used(&req.nonce).await? {
        return Err(AppError::Internal("Nonce already used - replay attack prevented".to_string()));
    }

    // 5. Verify signature based on funding chain
    let signature_valid = match approval.funding_chain {
        Chain::Solana => {
            let verifier = SolanaSignatureVerifier;
            verifier
                .verify_signature(&req.signature, &req.message, &req.user_wallet)
                .await?
        }
        Chain::Stellar => {
            let verifier = StellarSignatureVerifier;
            verifier
                .verify_signature(&req.signature, &req.message, &req.user_wallet)
                .await?
        }
        Chain::Near => {
            let verifier = NearSignatureVerifier;
            verifier
                .verify_signature(&req.signature, &req.message, &req.user_wallet)
                .await?
        }
    };

    if !signature_valid {
        app_state
            .ledger
            .update_token_approval_failed(
                &req.approval_id,
                "Signature verification failed",
                "SIGNATURE_INVALID",
            )
            .await?;
        return Err(AppError::InvalidSignature(
            "Signature verification failed".to_string(),
        ));
    }

    // 6. Verify message matches (prevent tampering)
    if req.message != approval.message {
        app_state
            .ledger
            .update_token_approval_failed(
                &req.approval_id,
                "Message tampering detected",
                "MESSAGE_TAMPERED",
            )
            .await?;
        return Err(AppError::Internal(
            "Message tampering detected".to_string(),
        ));
    }

    // 7. Execute transfer based on chain
    let tx_hash = match approval.funding_chain {
        Chain::Solana => {
            app_state
                .solana_executor
                .transfer_to_treasury(&approval.token, &approval.amount.to_string())
                .await?
        }
        Chain::Stellar => {
            app_state
                .stellar_executor
                .transfer_to_treasury(&approval.token, &approval.amount.to_string())
                .await?
        }
        Chain::Near => {
            app_state
                .near_executor
                .transfer_to_treasury(&approval.token, &approval.amount.to_string())
                .await?
        }
    };

    let tx_clone = tx_hash.clone();

    // 8. Update approval with transaction hash
    app_state
        .ledger
        .update_token_approval_submitted(&req.approval_id, &req.signature, &tx_hash)
        .await?;

    // 9. Spawn background task to wait for confirmation
    let approval_id = req.approval_id;
    let ledger = app_state.ledger.clone();
    let solana_executor = app_state.solana_executor.clone();
    let stellar_executor = app_state.stellar_executor.clone();
    let near_executor = app_state.near_executor.clone();
    let chain = approval.funding_chain;

    tokio::spawn(async move {
        // Wait for confirmation on the appropriate chain
        let confirmation_timeout = 120u64; // seconds
        let result = match chain {
            Chain::Solana => {
                solana_executor.wait_for_confirmation(&tx_hash, confirmation_timeout).await
            }
            Chain::Stellar => {
                stellar_executor.wait_for_confirmation_f(&tx_hash, confirmation_timeout).await
            }
            Chain::Near => {
                near_executor.wait_for_confirmation(&tx_hash, confirmation_timeout).await
            }
        };

        if let Ok(true) = result {
            // Get block height and mark as confirmed
            let block_height = match chain {
                Chain::Solana => solana_executor.get_block_height().await.unwrap_or(0),
                Chain::Stellar => stellar_executor.get_block_height().await.unwrap_or(0),
                Chain::Near => near_executor.get_block_height().await.unwrap_or(0),
            };

            let _ = ledger
                .update_token_approval_confirmed(&approval_id, block_height)
                .await;
            
            // Mark as executed
            let _ = ledger.update_token_approval_executed(&approval_id).await;
        } else {
            let _ = ledger
                .update_token_approval_failed(
                    &approval_id,
                    "Transaction confirmation timeout",
                    "CONFIRMATION_TIMEOUT",
                )
                .await;
        }
    });

    Ok((
        StatusCode::OK,
        Json(SubmitTokenApprovalResponse {
            approval_id: req.approval_id,
            status: "executed".to_string(),
            transaction_hash: tx_clone,
            confirmation_status: "Processed".to_string(),
            estimated_confirmation_time: 10,
        }),
    ))
}

/// GET /approval/status/:approval_id
/// Check the status of a token approval
pub async fn get_token_approval_status(
    State(app_state): State<AppState>,
    Path(approval_id): Path<Uuid>,
) -> AppResult<Json<TokenApprovalStatusResponse>> {
    let approval = app_state.ledger.get_token_approval(&approval_id).await?;

    // Check if expired and update status if needed
    if Utc::now() > approval.expires_at && approval.status == "pending" {
        app_state
            .ledger
            .update_token_approval_failed(
                &approval_id,
                "Approval expired",
                "APPROVAL_EXPIRED",
            )
            .await?;

        return Ok(Json(TokenApprovalStatusResponse {
            approval_id: approval.id,
            status: "expired".to_string(),
            transaction_hash: None,
            confirmation_status: None,
            block_height: None,
            confirmed_at: None,
            error_message: Some("Approval has expired".to_string()),
        }));
    }

    Ok(Json(TokenApprovalStatusResponse {
        approval_id: approval.id,
        status: approval.status,
        transaction_hash: approval.transaction_hash,
        confirmation_status: approval.confirmation_status,
        block_height: approval.block_height,
        confirmed_at: approval.confirmed_at,
        error_message: approval.error_message,
    }))
}
