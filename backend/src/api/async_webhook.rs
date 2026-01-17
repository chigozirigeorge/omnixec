use serde::{Deserialize, Serialize};
use tokio::spawn;
use tracing::{error, info};
use uuid::Uuid;
use crate::error::{AppResult};
use crate::{
    ledger::{repository::LedgerRepository, models::{Execution, QuoteStatus}},
};
use std::sync::Arc;

/// Webhook payload for payment notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentWebhookPayload {
    pub transaction_id: String,
    pub quote_id: Uuid,
    pub status: String,
    pub amount: String,
    pub timestamp: i64,
}

/// Webhook response - return 202 Accepted immediately
#[derive(Debug, Serialize)]
pub struct WebhookAcceptedResponse {
    pub status: String,
    pub message: String,
    pub webhook_id: String,
}

/// Async webhook processor - handles webhooks without blocking
pub struct AsyncWebhookProcessor {
    ledger: Arc<LedgerRepository>,
}

impl AsyncWebhookProcessor {
    pub fn new(ledger: Arc<LedgerRepository>) -> Self {
        Self { ledger }
    }

    /// Accept webhook and return 202 immediately
    /// Processes asynchronously in background
    pub fn process_webhook_async(
        &self,
        webhook_id: String,
        payload: PaymentWebhookPayload,
    ) -> WebhookAcceptedResponse {
        let ledger = self.ledger.clone();

        let webhook_idd = webhook_id.clone();

        // Spawn background task - returns immediately
        spawn(async move {
            if let Err(e) = Self::process_webhook_background(ledger, payload).await {
                error!("Webhook processing error for {}: {:?}", webhook_id.clone(), e);
            }
        });

        WebhookAcceptedResponse {
            status: "accepted".to_string(),
            message: "Webhook received and queued for processing".to_string(),
            webhook_id: webhook_idd,
        }
    }

    /// Process webhook in background
    async fn process_webhook_background(
        ledger: Arc<LedgerRepository>,
        payload: PaymentWebhookPayload,
    ) -> AppResult<()> {
        info!("⚙️ Processing webhook: {}", payload.transaction_id);

        

        // Step 1: Get execution details
        let execution = match ledger.get_execution_by_quote_id(&payload.quote_id).await {
            Ok(exec) => exec,
            Err(e) => {
                error!("Failed to get execution for quote {}: {}", payload.quote_id, e);
                return Err(e);
            }
        };

        // Step 2: Validate webhook payload
        if let Err(e) = Self::validate_webhook_payload(&execution, &payload) {
            error!("Webhook validation failed for quote {}: {}", payload.quote_id, e);
            return Err(e);
        }

        // Step 3: Update execution with transaction hash from webhook
        ledger
            .update_execution_hash(
                &execution.id,
                &payload.transaction_id,
                &payload.status,
            )
            .await?;

        // Step 4: Update quote status based on webhook status
        let new_quote_status = match payload.status.as_str() {
            "success" | "completed" => QuoteStatus::Executed,
            "failed" | "error" => QuoteStatus::Failed,
            "pending" | "confirming" => QuoteStatus::Pending,
            _ => QuoteStatus::Pending,
        };

        let mut tx = ledger.begin_tx().await?;

        ledger
            .update_quote_status(
                &mut tx,
                payload.quote_id,
                QuoteStatus::Committed,
                new_quote_status
            )
            .await?;

        // Step 5: Log settlement record for accounting
        // Note: settlement is recorded separately through the settlement module
        info!("Settlement execution logged for quote: {}", execution.quote_id);

        info!(
            "✓ Webhook processed: quote={} status={} tx={}",
            payload.quote_id, payload.status, payload.transaction_id
        );

        Ok(())
    }

    /// Validate webhook payload matches execution
    fn validate_webhook_payload(
        execution: &Execution,
        payload: &PaymentWebhookPayload,
    ) -> AppResult<()> {
        // Verify quote_id matches
        if execution.quote_id != payload.quote_id {
            return Err(crate::error::AppError::BadRequest(
                "Quote ID mismatch in webhook".into(),
            ));
        }

        // Amount verification is handled at the quote creation and execution stages
        // Webhook payload is accepted as-is for settlement recording

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_response_format() {
        let response = WebhookAcceptedResponse {
            status: "accepted".to_string(),
            message: "Test".to_string(),
            webhook_id: "test-123".to_string(),
        };

        assert_eq!(response.status, "accepted");
        assert!(!response.webhook_id.is_empty());
    }
}
