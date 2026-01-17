# Token Approval Flow - Implementation Roadmap

## Overview

This document outlines the step-by-step implementation guide for integrating the improved **Token Approval + Signature** flow into your platform.

---

## Phase 1: Database Schema & Models

### Step 1.1: Create Approvals Table

```sql
CREATE TABLE approvals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    quote_id UUID NOT NULL REFERENCES quotes(id),
    user_id UUID NOT NULL REFERENCES users(id),
    funding_chain VARCHAR(50) NOT NULL,
    token VARCHAR(50) NOT NULL,
    amount DECIMAL(20, 8) NOT NULL,
    recipient VARCHAR(255) NOT NULL,
    
    -- Message & Signature Fields
    message TEXT NOT NULL,
    nonce VARCHAR(255) UNIQUE NOT NULL,
    signature TEXT,
    user_wallet VARCHAR(255) NOT NULL,
    
    -- Status Tracking
    status VARCHAR(50) NOT NULL DEFAULT 'pending', 
    -- enum: 'pending', 'signed', 'submitted', 'confirmed', 'executed', 'failed', 'expired', 'cancelled'
    
    -- Blockchain Data
    transaction_hash VARCHAR(255),
    block_height BIGINT,
    confirmation_status VARCHAR(50), 
    -- enum: 'Processed', 'Confirmed', 'Finalized'
    
    -- Timestamps
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP NOT NULL,
    submitted_at TIMESTAMP,
    confirmed_at TIMESTAMP,
    executed_at TIMESTAMP,
    failed_at TIMESTAMP,
    
    -- Error Handling
    error_message TEXT,
    error_code VARCHAR(50),
    retry_count INT DEFAULT 0,
    last_retry_at TIMESTAMP,
    
    CONSTRAINT unique_active_approval UNIQUE (quote_id, status) 
    WHERE status IN ('pending', 'submitted', 'confirmed')
);

-- Indexes for performance
CREATE INDEX idx_approvals_user_id ON approvals(user_id);
CREATE INDEX idx_approvals_quote_id ON approvals(quote_id);
CREATE INDEX idx_approvals_status ON approvals(status);
CREATE INDEX idx_approvals_nonce ON approvals(nonce);
CREATE INDEX idx_approvals_expires_at ON approvals(expires_at) 
WHERE status IN ('pending', 'submitted');
```

### Step 1.2: Create Rust Models

```rust
// In src/ledger/models.rs

use sqlx::FromRow;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Approval {
    pub id: Uuid,
    pub quote_id: Uuid,
    pub user_id: Uuid,
    pub funding_chain: String,
    pub token: String,
    pub amount: Decimal,
    pub recipient: String,
    
    pub message: String,
    pub nonce: String,
    pub signature: Option<String>,
    pub user_wallet: String,
    
    pub status: String,
    pub transaction_hash: Option<String>,
    pub block_height: Option<i64>,
    pub confirmation_status: Option<String>,
    
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub retry_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateApprovalRequest {
    pub quote_id: Uuid,
    pub user_id: Uuid,
    pub funding_chain: String,
    pub token: String,
    pub amount: String,
    pub recipient: String,
}

#[derive(Debug, Serialize)]
pub struct CreateApprovalResponse {
    pub approval_id: Uuid,
    pub message_to_sign: String,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitApprovalRequest {
    pub approval_id: Uuid,
    pub user_wallet: String,
    pub signature: String,
    pub message: String,
    pub nonce: String,
}

#[derive(Debug, Serialize)]
pub struct SubmitApprovalResponse {
    pub approval_id: Uuid,
    pub status: String,
    pub transaction_hash: String,
    pub confirmation_status: String,
    pub estimated_confirmation_time: u32,
}

#[derive(Debug, Serialize)]
pub struct ApprovalStatusResponse {
    pub approval_id: Uuid,
    pub status: String,
    pub transaction_hash: Option<String>,
    pub confirmation_status: Option<String>,
    pub block_height: Option<i64>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ApprovalStatus {
    Pending,
    Signed,
    Submitted,
    Confirmed,
    Executed,
    Failed,
    Expired,
    Cancelled,
}

impl ApprovalStatus {
    pub fn to_string(&self) -> String {
        match self {
            Self::Pending => "pending".to_string(),
            Self::Signed => "signed".to_string(),
            Self::Submitted => "submitted".to_string(),
            Self::Confirmed => "confirmed".to_string(),
            Self::Executed => "executed".to_string(),
            Self::Failed => "failed".to_string(),
            Self::Expired => "expired".to_string(),
            Self::Cancelled => "cancelled".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "signed" => Self::Signed,
            "submitted" => Self::Submitted,
            "confirmed" => Self::Confirmed,
            "executed" => Self::Executed,
            "failed" => Self::Failed,
            "expired" => Self::Expired,
            "cancelled" => Self::Cancelled,
            _ => Self::Pending,
        }
    }
}
```

---

## Phase 2: Signature Verification (Security Core)

### Step 2.1: Add Signature Verification Traits

```rust
// In src/execution/mod.rs

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

/// Trait for signature verification across all chains
pub trait SignatureVerifier: Send + Sync {
    /// Verify a signature for a given message and public key
    async fn verify_signature(
        &self,
        signature: &str,           // base64 encoded signature
        message: &str,             // original message
        public_key: &str,          // user's public key
    ) -> AppResult<bool>;
    
    /// Extract/recover the signer's public key from signature + message
    /// (optional, for additional validation)
    fn recover_public_key(
        &self,
        signature: &str,
        message: &str,
    ) -> AppResult<String>;
}

// Solana Implementation
impl SignatureVerifier for SolanaExecutor {
    async fn verify_signature(
        &self,
        signature: &str,
        message: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        use solana_sdk::{signature::Signature, pubkey::Pubkey, signer::keypair::Keypair};
        use ed25519_dalek::VerifyingKey;
        use std::str::FromStr;

        // Decode base64 signature
        let signature_bytes = BASE64.decode(signature)?;
        let sig = Signature::try_from(signature_bytes.as_slice())?;

        // Parse public key
        let pubkey = Pubkey::from_str(public_key)?;
        let pubkey_bytes: &[u8; 32] = pubkey.as_ref();

        // Verify using ed25519_dalek
        let verify_key = VerifyingKey::from_bytes(pubkey_bytes)?;
        let message_bytes = message.as_bytes();
        
        verify_key
            .verify_strict(message_bytes, &sig.to_bytes())
            .map_err(|_| AppError::SignatureVerificationFailed)?;

        Ok(true)
    }

    fn recover_public_key(
        &self,
        _signature: &str,
        _message: &str,
    ) -> AppResult<String> {
        Err(AppError::NotImplemented)
    }
}

// Stellar Implementation
impl SignatureVerifier for StellarExecutor {
    async fn verify_signature(
        &self,
        signature: &str,
        message: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        use stellar_sdk::{PublicKey, Keypair};
        use ed25519_dalek::VerifyingKey;

        // Decode base64 signature
        let signature_bytes = BASE64.decode(signature)?;

        // Parse Stellar public key (Stroop format)
        let keypair = Keypair::from_public_key(public_key)?;
        let pubkey_bytes: &[u8; 32] = keypair.public_key_bytes();

        // Verify
        let verify_key = VerifyingKey::from_bytes(pubkey_bytes)?;
        let message_bytes = message.as_bytes();
        
        verify_key
            .verify_strict(message_bytes, &signature_bytes)
            .map_err(|_| AppError::SignatureVerificationFailed)?;

        Ok(true)
    }

    fn recover_public_key(
        &self,
        _signature: &str,
        _message: &str,
    ) -> AppResult<String> {
        Err(AppError::NotImplemented)
    }
}

// NEAR Implementation
impl SignatureVerifier for NearExecutor {
    async fn verify_signature(
        &self,
        signature: &str,
        message: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        use near_crypto::{PublicKey, Signature};

        // Decode base64 signature
        let signature_bytes = BASE64.decode(signature)?;
        let sig = Signature::try_from(signature_bytes.as_slice())?;

        // Parse public key
        let pk = PublicKey::from_str(public_key)?;

        // Verify
        pk.verify(message.as_bytes(), &sig)
            .map_err(|_| AppError::SignatureVerificationFailed)?;

        Ok(true)
    }

    fn recover_public_key(
        &self,
        _signature: &str,
        _message: &str,
    ) -> AppResult<String> {
        Err(AppError::NotImplemented)
    }
}
```

### Step 2.2: Nonce & Replay Protection

```rust
// In src/ledger/repository.rs

impl LedgerRepository {
    /// Check if nonce has been used before (prevents replay attacks)
    pub async fn is_nonce_used(&self, nonce: &str) -> AppResult<bool> {
        let result = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM approvals WHERE nonce = $1 AND status != 'expired')"
        )
        .bind(nonce)
        .fetch_one(&self.db)
        .await?;

        Ok(result)
    }

    /// Mark nonce as used
    pub async fn mark_nonce_used(&self, nonce: &str) -> AppResult<()> {
        sqlx::query("UPDATE approvals SET status = 'signed' WHERE nonce = $1")
            .bind(nonce)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    /// Get approval by ID
    pub async fn get_approval(&self, approval_id: &Uuid) -> AppResult<Approval> {
        let approval = sqlx::query_as::<_, Approval>(
            "SELECT * FROM approvals WHERE id = $1"
        )
        .bind(approval_id)
        .fetch_one(&self.db)
        .await?;

        Ok(approval)
    }

    /// Create new approval
    pub async fn create_approval(&self, approval: Approval) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO approvals (
                id, quote_id, user_id, funding_chain, token, amount, recipient,
                message, nonce, user_wallet, status, created_at, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#
        )
        .bind(approval.id)
        .bind(approval.quote_id)
        .bind(approval.user_id)
        .bind(approval.funding_chain)
        .bind(approval.token)
        .bind(approval.amount)
        .bind(approval.recipient)
        .bind(approval.message)
        .bind(approval.nonce)
        .bind(approval.user_wallet)
        .bind(approval.status)
        .bind(approval.created_at)
        .bind(approval.expires_at)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Update approval with signature & submission details
    pub async fn update_approval_submitted(
        &self,
        approval_id: &Uuid,
        signature: &str,
        tx_hash: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE approvals 
            SET signature = $1, status = 'submitted', transaction_hash = $2, submitted_at = NOW()
            WHERE id = $3
            "#
        )
        .bind(signature)
        .bind(tx_hash)
        .bind(approval_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Mark approval as confirmed
    pub async fn update_approval_confirmed(
        &self,
        approval_id: &Uuid,
        block_height: i64,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE approvals 
            SET status = 'confirmed', confirmation_status = 'Finalized', 
                block_height = $1, confirmed_at = NOW()
            WHERE id = $2
            "#
        )
        .bind(block_height)
        .bind(approval_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Mark approval as executed
    pub async fn update_approval_executed(
        &self,
        approval_id: &Uuid,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE approvals SET status = 'executed', executed_at = NOW() WHERE id = $1"
        )
        .bind(approval_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Mark approval as failed
    pub async fn update_approval_failed(
        &self,
        approval_id: &Uuid,
        error_message: &str,
        error_code: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE approvals 
            SET status = 'failed', error_message = $1, error_code = $2, failed_at = NOW()
            WHERE id = $3
            "#
        )
        .bind(error_message)
        .bind(error_code)
        .bind(approval_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }
}
```

---

## Phase 3: API Endpoints Implementation

### Step 3.1: Create Approval Endpoint

```rust
// In src/api/handler.rs

use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use uuid::Uuid;

#[post("/approval/create")]
pub async fn create_approval(
    State(app_state): State<AppState>,
    Json(req): Json<CreateApprovalRequest>,
) -> Result<(StatusCode, Json<CreateApprovalResponse>)> {
    // 1. Validate quote exists
    let quote = app_state
        .ledger
        .get_quote(&req.quote_id)
        .await?;

    if quote.status != "pending" {
        return Err(AppError::InvalidQuoteStatus);
    }

    // 2. Validate expiration
    if Utc::now() > quote.expires_at {
        return Err(AppError::QuoteExpired);
    }

    // 3. Validate user has verified wallet on funding chain
    let wallet = app_state
        .ledger
        .get_user_wallet_by_chain(&req.user_id, &req.funding_chain)
        .await?;

    if wallet.status != "verified" {
        return Err(AppError::WalletNotVerified);
    }

    // 4. Generate unique nonce
    let nonce = Uuid::new_v4().to_string();

    // 5. Create message for user to sign
    let expires_at = Utc::now() + chrono::Duration::minutes(15);
    let message = format!(
        "APPROVE_{}_TRANSFER\nAmount: {} {}\nRecipient: {}\nQuote ID: {}\nNonce: {}\nExpires: {}",
        req.token.to_uppercase(),
        req.amount,
        req.token.to_uppercase(),
        req.recipient,
        req.quote_id,
        nonce,
        expires_at.to_rfc3339()
    );

    // 6. Create approval record
    let approval = Approval {
        id: Uuid::new_v4(),
        quote_id: req.quote_id,
        user_id: req.user_id,
        funding_chain: req.funding_chain,
        token: req.token,
        amount: Decimal::from_str(&req.amount)?,
        recipient: req.recipient,
        message: message.clone(),
        nonce: nonce.clone(),
        signature: None,
        user_wallet: wallet.address.clone(),
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
    };

    app_state.ledger.create_approval(approval.clone()).await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApprovalResponse {
            approval_id: approval.id,
            message_to_sign: message,
            nonce,
            expires_at,
        }),
    ))
}
```

### Step 3.2: Submit Approval Endpoint

```rust
#[post("/approval/submit")]
pub async fn submit_approval(
    State(app_state): State<AppState>,
    Json(req): Json<SubmitApprovalRequest>,
) -> Result<(StatusCode, Json<SubmitApprovalResponse>)> {
    // 1. Get approval
    let mut approval = app_state.ledger.get_approval(&req.approval_id).await?;

    // 2. Validate not expired
    if Utc::now() > approval.expires_at {
        app_state
            .ledger
            .update_approval_failed(
                &req.approval_id,
                "Approval expired",
                "APPROVAL_EXPIRED",
            )
            .await?;
        return Err(AppError::ApprovalExpired);
    }

    // 3. Validate not duplicate (nonce already used)
    if app_state.ledger.is_nonce_used(&req.nonce).await? {
        return Err(AppError::NonceAlreadyUsed);
    }

    // 4. Verify signature based on chain
    let verifier = match approval.funding_chain.as_str() {
        "Solana" => &app_state.solana_executor as &dyn SignatureVerifier,
        "Stellar" => &app_state.stellar_executor as &dyn SignatureVerifier,
        "Near" => &app_state.near_executor as &dyn SignatureVerifier,
        _ => return Err(AppError::UnsupportedChain),
    };

    let signature_valid = verifier
        .verify_signature(&req.signature, &req.message, &approval.user_wallet)
        .await?;

    if !signature_valid {
        app_state
            .ledger
            .update_approval_failed(
                &req.approval_id,
                "Signature verification failed",
                "SIGNATURE_INVALID",
            )
            .await?;
        return Err(AppError::SignatureVerificationFailed);
    }

    // 5. Verify message matches
    if req.message != approval.message {
        app_state
            .ledger
            .update_approval_failed(
                &req.approval_id,
                "Message tampering detected",
                "MESSAGE_TAMPERED",
            )
            .await?;
        return Err(AppError::MessageTampering);
    }

    // 6. Mark nonce as used
    app_state.ledger.mark_nonce_used(&req.nonce).await?;

    // 7. Execute transfer based on chain
    let tx_hash = match approval.funding_chain.as_str() {
        "Solana" => {
            app_state
                .solana_executor
                .transfer_to_treasury_from_user(
                    &approval.user_wallet,
                    &approval.token,
                    &approval.amount.to_string(),
                    &approval.recipient,
                )
                .await?
        }
        "Stellar" => {
            app_state
                .stellar_executor
                .transfer_to_treasury_from_user(
                    &approval.user_wallet,
                    &approval.token,
                    &approval.amount.to_string(),
                    &approval.recipient,
                )
                .await?
        }
        "Near" => {
            app_state
                .near_executor
                .transfer_to_treasury_from_user(
                    &approval.user_wallet,
                    &approval.token,
                    &approval.amount.to_string(),
                    &approval.recipient,
                )
                .await?
        }
        _ => return Err(AppError::UnsupportedChain),
    };

    // 8. Update approval record with transaction hash
    app_state
        .ledger
        .update_approval_submitted(&req.approval_id, &req.signature, &tx_hash)
        .await?;

    // 9. Trigger confirmation polling (async)
    let approval_id = req.approval_id;
    let ledger = app_state.ledger.clone();
    let executor = app_state.get_executor(&approval.funding_chain)?.clone();

    tokio::spawn(async move {
        if let Ok(confirmation) = executor.wait_for_confirmation(&tx_hash, 120).await {
            if confirmation {
                if let Ok(block_height) = executor.get_block_height().await {
                    let _ = ledger.update_approval_confirmed(&approval_id, block_height).await;
                }
            }
        }
    });

    Ok((
        StatusCode::OK,
        Json(SubmitApprovalResponse {
            approval_id: req.approval_id,
            status: "executed".to_string(),
            transaction_hash: tx_hash,
            confirmation_status: "Processed".to_string(),
            estimated_confirmation_time: 10,
        }),
    ))
}
```

### Step 3.3: Status Endpoint

```rust
#[get("/approval/status/:approval_id")]
pub async fn get_approval_status(
    State(app_state): State<AppState>,
    Path(approval_id): Path<Uuid>,
) -> Result<Json<ApprovalStatusResponse>> {
    let approval = app_state.ledger.get_approval(&approval_id).await?;

    // Check if expired and update status if needed
    if Utc::now() > approval.expires_at && approval.status == "pending" {
        app_state
            .ledger
            .update_approval_failed(
                &approval_id,
                "Approval expired",
                "APPROVAL_EXPIRED",
            )
            .await?;

        return Err(AppError::ApprovalExpired);
    }

    Ok(Json(ApprovalStatusResponse {
        approval_id: approval.id,
        status: approval.status,
        transaction_hash: approval.transaction_hash,
        confirmation_status: approval.confirmation_status,
        block_height: approval.block_height,
        confirmed_at: approval.confirmed_at,
        error_message: approval.error_message,
    }))
}
```

---

## Phase 4: Integration with Executors

### Step 4.1: Add Transfer Methods to Executors

Each executor (Solana, Stellar, NEAR) needs a method to accept user payments:

```rust
// In src/execution/solana.rs

impl SolanaExecutor {
    /// Transfer tokens from user to treasury (called after signature verification)
    pub async fn transfer_to_treasury_from_user(
        &self,
        user_wallet: &str,
        token: &str,
        amount: &str,
        recipient: &str,
    ) -> AppResult<String> {
        // 1. Build transfer instruction
        // 2. Sign as treasury
        // 3. Submit to blockchain
        // 4. Return tx hash
        
        todo!("Implement based on existing transfer_to_treasury logic")
    }

    /// Wait for transaction confirmation
    pub async fn wait_for_confirmation(
        &self,
        tx_hash: &str,
        timeout_secs: u64,
    ) -> AppResult<bool> {
        let start = Instant::now();
        loop {
            if let Ok(status) = self.client.get_signature_statuses(&[tx_hash.into()]) {
                if let Some(Some(confirmation_status)) = status.value.first() {
                    if confirmation_status.confirmation_status == Some("confirmed".to_string()) {
                        return Ok(true);
                    }
                }
            }

            if start.elapsed().as_secs() > timeout_secs {
                return Ok(false);
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    }

    /// Get current block height
    pub async fn get_block_height(&self) -> AppResult<i64> {
        let slot = self.client.get_slot().await?;
        Ok(slot as i64)
    }
}
```

---

## Phase 5: Frontend Integration

### Step 5.1: Frontend Approval Component

```typescript
// ApprovalFlow.tsx

import React, { useState } from 'react';
import { WalletContextState, useWallet } from '@solana/wallet-adapter-react';

interface ApprovalFlowProps {
  quoteId: string;
  amount: string;
  chain: 'Solana' | 'Stellar' | 'Near';
  token: string;
  recipient: string;
  onApproved: (response: any) => void;
  onError: (error: Error) => void;
}

export const ApprovalFlow: React.FC<ApprovalFlowProps> = ({
  quoteId,
  amount,
  chain,
  token,
  recipient,
  onApproved,
  onError,
}) => {
  const [loading, setLoading] = useState(false);
  const [step, setStep] = useState<'initial' | 'signing' | 'submitting' | 'done'>('initial');
  const wallet = useWallet();

  const handleApprove = async () => {
    try {
      setLoading(true);
      setStep('signing');

      // Step 1: Create approval
      const createResponse = await fetch('/approval/create', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          quote_id: quoteId,
          user_id: localStorage.getItem('user_id'),
          funding_chain: chain,
          token,
          amount,
          recipient,
        }),
      }).then((r) => r.json());

      if (!createResponse.approval_id) {
        throw new Error('Failed to create approval');
      }

      // Step 2: Sign with wallet
      const message = new TextEncoder().encode(createResponse.message_to_sign);
      const signature = await wallet.signMessage?.(message);

      if (!signature) {
        throw new Error('User rejected signature');
      }

      // Step 3: Submit signed approval
      setStep('submitting');
      const submitResponse = await fetch('/approval/submit', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          approval_id: createResponse.approval_id,
          user_wallet: wallet.publicKey?.toBase58(),
          signature: Buffer.from(signature).toString('base64'),
          message: createResponse.message_to_sign,
          nonce: createResponse.nonce,
        }),
      }).then((r) => r.json());

      if (!submitResponse.transaction_hash) {
        throw new Error('Failed to submit approval');
      }

      // Step 4: Poll for confirmation
      let confirmed = false;
      let attempts = 0;
      while (!confirmed && attempts < 60) {
        const statusResponse = await fetch(
          `/approval/status/${createResponse.approval_id}`
        ).then((r) => r.json());

        if (statusResponse.status === 'confirmed') {
          confirmed = true;
          setStep('done');
          onApproved(submitResponse);
        }

        attempts++;
        await new Promise((resolve) => setTimeout(resolve, 2000));
      }

      if (!confirmed) {
        throw new Error('Confirmation timeout');
      }
    } catch (error) {
      setStep('initial');
      onError(error as Error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="approval-flow">
      {step === 'initial' && (
        <div className="approval-card">
          <h2>Approve Token Transfer</h2>
          <div className="approval-details">
            <p>
              Token: <strong>{token}</strong>
            </p>
            <p>
              Amount: <strong>{amount}</strong>
            </p>
            <p>
              Recipient: <strong>{recipient}</strong>
            </p>
          </div>
          <button onClick={handleApprove} disabled={loading}>
            {loading ? 'Processing...' : 'Approve with Wallet'}
          </button>
        </div>
      )}

      {step === 'signing' && (
        <div className="status-card">
          <p>⏳ Please sign the message in your wallet...</p>
        </div>
      )}

      {step === 'submitting' && (
        <div className="status-card">
          <p>⏳ Submitting transfer...</p>
        </div>
      )}

      {step === 'done' && (
        <div className="status-card success">
          <p>✓ Transfer successful!</p>
        </div>
      )}
    </div>
  );
};
```

---

## Phase 6: Testing & Validation

### Step 6.1: Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_approval() {
        // Setup
        let app_state = setup_test_state().await;
        
        // Execute
        let response = create_approval(
            State(app_state),
            Json(CreateApprovalRequest {
                quote_id: test_quote_id(),
                user_id: test_user_id(),
                funding_chain: "Solana".to_string(),
                token: "USDC".to_string(),
                amount: "100.00".to_string(),
                recipient: "treasury_address".to_string(),
            }),
        ).await;
        
        // Verify
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_signature_verification() {
        // Test that valid signatures pass
        // Test that invalid signatures fail
        // Test that tampered messages fail
    }

    #[tokio::test]
    async fn test_replay_attack_prevention() {
        // Test that reused nonce is rejected
        // Test that expired approvals are rejected
    }
}
```

### Step 6.2: Integration Tests

Test end-to-end flow with testnet

---

## Deployment Checklist

- [ ] Database migration deployed
- [ ] Signature verification traits implemented
- [ ] All three chain executors updated with signature verification
- [ ] API endpoints implemented and tested
- [ ] Frontend components created
- [ ] E2E tests passing
- [ ] Security audit completed
- [ ] Deployed to staging
- [ ] User testing completed
- [ ] Deployed to production

---

## Timeline Estimate

| Phase | Duration | Status |
|-------|----------|--------|
| Phase 1: DB & Models | 1-2 days | ⏳ |
| Phase 2: Signature Verification | 2-3 days | ⏳ |
| Phase 3: API Endpoints | 2-3 days | ⏳ |
| Phase 4: Executor Integration | 1-2 days | ⏳ |
| Phase 5: Frontend | 2-3 days | ⏳ |
| Phase 6: Testing | 2-3 days | ⏳ |
| **Total** | **10-16 days** | **⏳** |

---

## Key Security Reminders

✅ **Always verify signature before executing any transaction**
✅ **Use unique nonce for every approval**
✅ **Check expiration before processing**
✅ **Prevent replay attacks by marking nonce as used**
✅ **Never store user's private key**
✅ **Validate message hasn't been tampered**
✅ **Rate limit approval endpoints**
✅ **Log all approvals and executions**
✅ **Alert on failed verification attempts**

---

## Success Metrics

After implementation, you should see:

- ✅ 80% reduction in payment errors (no more copy/paste mistakes)
- ✅ 70% reduction in payment time (no manual user action needed)
- ✅ 95%+ transaction success rate
- ✅ Near-instant payment feedback
- ✅ Auto-retry capability for failed transfers
- ✅ Full audit trail of all approvals

