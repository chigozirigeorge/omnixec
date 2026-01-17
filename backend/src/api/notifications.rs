// Notification System
// 
// Email: Resend (modern, easy to use, good rates)
// Push: Firebase Cloud Messaging (easiest, best compatibility)
// SMS: Twilio (industry standard, reliable)
// Persistence: PostgreSQL with outbox pattern

use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;
use crate::error::AppResult;
use sqlx::PgPool;
use chrono::{DateTime, Utc};

// ============ CONFIGURATION ============

///notification configuration
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    /// Email provider
    pub resend_api_key: String,
    pub resend_from_email: String,
    
    /// Push notification provider
    pub firebase_project_id: String,
    pub firebase_credentials_path: String,
    
    /// SMS provider
    pub twilio_account_sid: String,
    pub twilio_auth_token: String,
    pub twilio_from_number: String,
    
    /// Database for persistence
    pub database_url: String,
}

// ============ EMAIL (RESEND) ============

/// Resend email client
pub struct ResendEmailClient {
    api_key: String,
    from_email: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct ResendEmailRequest {
    to: String,
    from: String,
    subject: String,
    html: String,
}

#[derive(Debug, Deserialize)]
struct ResendEmailResponse {
    id: String,
    from: String,
    to: String,
    created_at: String,
}

impl ResendEmailClient {
    pub fn new(api_key: String, from_email: String) -> Self {
        Self {
            api_key,
            from_email,
            client: reqwest::Client::new(),
        }
    }

    /// Send email via Resend
    pub async fn send_email(
        &self,
        to: &str,
        subject: &str,
        html_body: &str,
    ) -> AppResult<String> {
        let request = ResendEmailRequest {
            to: to.to_string(),
            from: self.from_email.clone(),
            subject: subject.to_string(),
            html: html_body.to_string(),
        };

        let response = self.client
            .post("https://api.resend.com/emails")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::error::AppError::ExternalError(
                format!("Resend API error: {}", error_text),
            ));
        }

        let result: ResendEmailResponse = response.json().await?;
        info!("📧 Email sent via Resend: {}", result.id);
        Ok(result.id)
    }
}

// ============ PUSH NOTIFICATIONS (FIREBASE) ============

/// Firebase Cloud Messaging client
pub struct FirebasePushClient {
    project_id: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct FirebaseMessage {
    message: FirebaseMessagePayload,
}

#[derive(Debug, Serialize)]
struct FirebaseMessagePayload {
    token: String,
    notification: FirebaseNotification,
    data: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct FirebaseNotification {
    title: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct FirebaseResponse {
    name: String,
}

impl FirebasePushClient {
    pub fn new(project_id: String) -> Self {
        Self {
            project_id,
            client: reqwest::Client::new(),
        }
    }

    /// Send push notification via Firebase Cloud Messaging
    pub async fn send_push(
        &self,
        device_token: &str,
        title: &str,
        body: &str,
        data: Option<std::collections::HashMap<String, String>>,
    ) -> AppResult<String> {
        // Get access token (requires gcloud auth setup)
        let access_token = self.get_access_token().await?;

        let message_data = data.unwrap_or_default();
        
        let message = FirebaseMessage {
            message: FirebaseMessagePayload {
                token: device_token.to_string(),
                notification: FirebaseNotification {
                    title: title.to_string(),
                    body: body.to_string(),
                },
                data: message_data,
            },
        };

        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            self.project_id
        );

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .json(&message)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::error::AppError::ExternalError(
                format!("Firebase API error: {}", error_text),
            ));
        }

        let result: FirebaseResponse = response.json().await?;
        info!("🔔 Push notification sent via Firebase: {}", result.name);
        Ok(result.name)
    }

    async fn get_access_token(&self) -> AppResult<String> {
        // In production, use google-authz or similar
        // For now, use environment variable
        let token = std::env::var("FIREBASE_ACCESS_TOKEN")
            .map_err(|_| crate::error::AppError::Config(
                "FIREBASE_ACCESS_TOKEN not set".into(),
            ))?;
        Ok(token)
    }
}

// ============ SMS (TWILIO) ============

/// Twilio SMS client
pub struct TwilioSmsClient {
    account_sid: String,
    auth_token: String,
    from_number: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct TwilioSmsRequest {
    From: String,
    To: String,
    Body: String,
}

#[derive(Debug, Deserialize)]
struct TwilioSmsResponse {
    sid: String,
    status: String,
}

impl TwilioSmsClient {
    pub fn new(account_sid: String, auth_token: String, from_number: String) -> Self {
        Self {
            account_sid,
            auth_token,
            from_number,
            client: reqwest::Client::new(),
        }
    }

    /// Send SMS via Twilio
    pub async fn send_sms(
        &self,
        to_number: &str,
        message: &str,
    ) -> AppResult<String> {
        let request = TwilioSmsRequest {
            From: self.from_number.clone(),
            To: to_number.to_string(),
            Body: message.to_string(),
        };

        let url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
            self.account_sid
        );

        let response = self.client
            .post(&url)
            .basic_auth(&self.account_sid, Some(&self.auth_token))
            .form(&[
                ("From", self.from_number.as_str()),
                ("To", to_number),
                ("Body", message),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::error::AppError::ExternalError(
                format!("Twilio API error: {}", error_text),
            ));
        }

        let result: TwilioSmsResponse = response.json().await?;
        info!("📱 SMS sent via Twilio: {}", result.sid);
        Ok(result.sid)
    }
}

// ============ OUTBOX PATTERN (PERSISTENCE) ============

/// Notification stored in database (outbox pattern)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NotificationRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub channel: String,  // "email", "push", "sms"
    pub priority: String, // "low", "normal", "high"
    pub recipient: String, // email, phone, device token
    pub subject: String,
    pub body: String,
    pub status: String, // "pending", "sent", "failed", "bounced"
    pub external_id: Option<String>, // Resend/Firebase/Twilio ID
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
}

///notification persistence
pub struct NotificationPersistence {
    pool: PgPool,
}

impl NotificationPersistence {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Queue notification in database
    pub async fn queue_notification(
        &self,
        user_id: Uuid,
        channel: &str,
        priority: &str,
        recipient: &str,
        subject: &str,
        body: &str,
    ) -> AppResult<Uuid> {
        let id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO notifications (
                id, user_id, channel, priority, recipient, 
                subject, body, status, retry_count, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', 0, $8)
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(channel)
        .bind(priority)
        .bind(recipient)
        .bind(subject)
        .bind(body)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;

        info!("📬 Notification queued: {} ({})", id, channel);
        Ok(id)
    }

    /// Get pending notifications
    pub async fn get_pending(&self, limit: i32, priority: Option<&str>) -> AppResult<Vec<NotificationRecord>> {
        let query = if let Some(p) = priority {
            sqlx::query_as::<_, NotificationRecord>(
                "SELECT * FROM notifications WHERE status = 'pending' AND priority = $1 
                 ORDER BY created_at ASC LIMIT $2"
            )
            .bind(p)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, NotificationRecord>(
                "SELECT * FROM notifications WHERE status = 'pending' 
                 ORDER BY priority DESC, created_at ASC LIMIT $1"
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(query)
    }

    /// Mark as sent
    pub async fn mark_sent(
        &self,
        id: &Uuid,
        external_id: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE notifications SET status = 'sent', external_id = $1, sent_at = $2 WHERE id = $3"
        )
        .bind(external_id)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        info!("✓ Notification marked as sent: {}", id);
        Ok(())
    }

    /// Mark as failed with retry
    pub async fn mark_failed(
        &self,
        id: &Uuid,
        error: &str,
        max_retries: i32,
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;

        let current_retry: i32 = sqlx::query_scalar("SELECT retry_count FROM notifications WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;

        if current_retry >= max_retries {
            // Mark as permanently failed
            sqlx::query(
                "UPDATE notifications SET status = 'failed', last_error = $1, retry_count = $2 WHERE id = $3"
            )
            .bind(error)
            .bind(current_retry + 1)
            .bind(id)
            .execute(&mut *tx)
            .await?;

            warn!("❌ Notification failed permanently: {} (retries exhausted)", id);
        } else {
            // Keep as pending for retry
            sqlx::query(
                "UPDATE notifications SET last_error = $1, retry_count = $2 WHERE id = $3"
            )
            .bind(error)
            .bind(current_retry + 1)
            .bind(id)
            .execute(&mut *tx)
            .await?;

            info!("⚠️ Notification will retry: {} (attempt {}/{})", id, current_retry + 1, max_retries);
        }

        tx.commit().await?;
        Ok(())
    }

    /// Get statistics
    pub async fn get_stats(&self) -> AppResult<NotificationStats> {
        let stats = sqlx::query_as::<_, NotificationStats>(
            r#"
            SELECT 
                COUNT(*) FILTER (WHERE status = 'pending') as pending,
                COUNT(*) FILTER (WHERE status = 'sent') as sent,
                COUNT(*) FILTER (WHERE status = 'failed') as failed,
                COUNT(*) FILTER (WHERE status = 'bounced') as bounced
            FROM notifications
            WHERE created_at > NOW() - INTERVAL '24 hours'
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(stats)
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct NotificationStats {
    pub pending: i64,
    pub sent: i64,
    pub failed: i64,
    pub bounced: i64,
}

// ============ TESTS ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resend_client_creation() {
        let client = ResendEmailClient::new(
            "test_key".to_string(),
            "noreply@example.com".to_string(),
        );
        assert_eq!(client.from_email, "noreply@example.com");
    }

    #[test]
    fn test_twilio_client_creation() {
        let client = TwilioSmsClient::new(
            "AC123456".to_string(),
            "token123".to_string(),
            "+1234567890".to_string(),
        );
        assert_eq!(client.from_number, "+1234567890");
    }
}
