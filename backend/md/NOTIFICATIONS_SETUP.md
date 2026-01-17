# Production Notification Setup Guide

Complete setup instructions for Email (Resend), Push (Firebase), and SMS (Twilio).

---

## 📧 EMAIL SETUP - RESEND

### Why Resend?
- ✅ Modern API (REST)
- ✅ Best-in-class deliverability
- ✅ Built-in email templates
- ✅ Easy authentication
- ✅ Affordable ($0.10-$0.20 per email)
- ✅ Free tier: 100 emails/day

### Step 1: Create Resend Account

```bash
# Visit https://resend.com
# Sign up with email
# Create workspace
# Get API key from dashboard
```

### Step 2: Configure API Key

```bash
# Add to .env
RESEND_API_KEY="re_xxxxxxxxxxxxx"
RESEND_FROM_EMAIL="noreply@your-domain.com"

# In Docker/production:
export RESEND_API_KEY="re_xxxxxxxxxxxxx"
export RESEND_FROM_EMAIL="noreply@your-domain.com"
```

### Step 3: Verify Sending Domain (Production)

```bash
# In Resend dashboard:
# 1. Go to "Domains"
# 2. Add your domain: mail.your-domain.com
# 3. Follow DNS instructions:
#    - Add SPF record
#    - Add DKIM record
#    - Add DMARC record (optional)
# 4. Click "Verify"

# Expected DNS records:
MX: mail.your-domain.com A 1.2.3.4
SPF: v=spf1 include:resend.com ~all
DKIM: Add resend-generated key to DNS
```

### Step 4: Test Email Sending

```rust
// In your code:
let client = ResendEmailClient::new(
    env::var("RESEND_API_KEY")?,
    env::var("RESEND_FROM_EMAIL")?,
);

client.send_email(
    "user@example.com",
    "Welcome to CrossChain Payments",
    "<h1>Welcome!</h1><p>Your payment is ready.</p>"
).await?;
```

### Step 5: Create Email Templates

```html
<!-- Payment Confirmation Template -->
<table style="font-family: Arial; max-width: 600px;">
  <tr>
    <td style="background-color: #f0f0f0; padding: 20px;">
      <h2>Payment Received</h2>
      <p>Your payment has been received and is processing.</p>
      <p><strong>Amount:</strong> {{amount}}</p>
      <p><strong>From:</strong> {{from_chain}}</p>
      <p><strong>To:</strong> {{to_chain}}</p>
      <p><strong>Transaction ID:</strong> {{tx_id}}</p>
      <p><a href="{{tracking_link}}">View Status</a></p>
    </td>
  </tr>
</table>
```

### Resend Integration Code

```rust
// In src/api/notifications_production.rs
impl ResendEmailClient {
    pub async fn send_payment_confirmation(
        &self,
        to: &str,
        amount: &str,
        from_chain: &str,
        to_chain: &str,
        tx_id: &str,
    ) -> AppResult<String> {
        let html = format!(
            r#"
            <table style="font-family: Arial; max-width: 600px;">
              <tr>
                <td style="background-color: #f0f0f0; padding: 20px;">
                  <h2>✓ Payment Received</h2>
                  <p>Your payment of <strong>{} {} → {}</strong> is being processed.</p>
                  <ul>
                    <li><strong>From:</strong> {}</li>
                    <li><strong>To:</strong> {}</li>
                    <li><strong>Transaction:</strong> <code>{}</code></li>
                  </ul>
                  <p style="color: #666;">Transaction processing typically takes 2-5 minutes.</p>
                </td>
              </tr>
            </table>
            "#,
            amount, from_chain, to_chain, from_chain, to_chain, tx_id
        );

        self.send_email(
            to,
            &format!("Payment Received: {} {}", amount, from_chain),
            &html,
        ).await
    }
}
```

### Monitoring Email

```bash
# Resend dashboard shows:
- Delivery rate
- Open rate
- Click rate
- Bounce rate
- Spam complaints

# Set up webhook for bounces:
POST /webhooks/resend
{
  "type": "email.bounced",
  "data": {
    "email": "bounced@example.com",
    "reason": "permanent_failure"
  }
}
```

---

## 🔔 PUSH NOTIFICATIONS - FIREBASE CLOUD MESSAGING

### Why Firebase?
- ✅ Works on iOS + Android
- ✅ Free tier included
- ✅ Simple setup
- ✅ Google-backed reliability
- ✅ Built-in analytics
- ✅ No per-message cost

### Step 1: Create Firebase Project

```bash
# Option A: Via Firebase Console
# 1. Go to https://console.firebase.google.com
# 2. Click "Create Project"
# 3. Name: "crosschain-payments"
# 4. Accept terms, create

# Option B: Via CLI
npm install -g firebase-tools
firebase login
firebase projects:create crosschain-payments
```

### Step 2: Enable Cloud Messaging

```bash
# In Firebase Console:
# 1. Go to Project Settings
# 2. Go to "Cloud Messaging" tab
# 3. Enable Cloud Messaging API
# 4. Copy "Server API Key"
```

### Step 3: Generate Service Account

```bash
# In Firebase Console:
# 1. Go to Project Settings → Service Accounts
# 2. Click "Generate New Private Key"
# 3. Save as firebase-credentials.json
# 4. Add to .gitignore

# Structure:
{
  "type": "service_account",
  "project_id": "crosschain-payments",
  "private_key_id": "key123",
  "private_key": "-----BEGIN PRIVATE KEY-----...",
  "client_email": "firebase@...",
  ...
}
```

### Step 4: Configure Environment

```bash
# Add to .env
FIREBASE_PROJECT_ID="crosschain-payments"
FIREBASE_CREDENTIALS_PATH="./firebase-credentials.json"

# Set access token (use google-authz library):
export FIREBASE_ACCESS_TOKEN=$(gcloud auth application-default print-access-token)
```

### Step 5: Client Registration (Mobile App)

```javascript
// React Native Example
import { initializeApp } from 'firebase/app';
import { getMessaging, getToken } from 'firebase/messaging';

const firebaseConfig = {
  apiKey: "AIza...",
  projectId: "crosschain-payments",
  messagingSenderId: "123456789",
  appId: "1:123456789:web:abcdef123456"
};

const app = initializeApp(firebaseConfig);
const messaging = getMessaging(app);

// Get device token
getToken(messaging, {
  vapidKey: "BNq..." // From Firebase Console
}).then(currentToken => {
  if (currentToken) {
    // Send currentToken to your backend
    fetch('/api/register-device', {
      method: 'POST',
      body: JSON.stringify({ device_token: currentToken })
    });
  }
});

// Listen for messages
onMessage(messaging, (payload) => {
  console.log('Message received:', payload);
  // Update UI
});
```

### Step 6: Backend Integration

```rust
// In src/api/notifications_production.rs
impl FirebasePushClient {
    pub async fn send_payment_notification(
        &self,
        device_token: &str,
        amount: &str,
        from_chain: &str,
        tx_id: &str,
    ) -> AppResult<String> {
        let mut data = std::collections::HashMap::new();
        data.insert("type".to_string(), "payment_received".to_string());
        data.insert("amount".to_string(), amount.to_string());
        data.insert("tx_id".to_string(), tx_id.to_string());
        data.insert("from_chain".to_string(), from_chain.to_string());

        self.send_push(
            device_token,
            "💰 Payment Received",
            &format!("{} {} incoming...", amount, from_chain),
            Some(data),
        ).await
    }
}
```

### Firebase Monitoring

```bash
# In Firebase Console:
# - Insights tab shows delivery stats
# - Message statistics
# - Error rates
# - Device statistics

# Send test message:
firebase functions:shell
> testMessage({
>   token: 'device-token-here',
>   data: { test: 'true' }
> })
```

---

## 📱 SMS SETUP - TWILIO

### Why Twilio?
- ✅ Global SMS delivery (190+ countries)
- ✅ Reliable (99.5% uptime)
- ✅ Easy setup
- ✅ Pay-as-you-go ($0.01-0.05 per SMS)
- ✅ Free trial credits
- ✅ Built-in DLR (Delivery Receipt)

### Step 1: Create Twilio Account

```bash
# Visit https://www.twilio.com/try-twilio
# Sign up with email
# Verify phone number
# Get free credits ($15)
```

### Step 2: Get Phone Number

```bash
# In Twilio Console:
# 1. Go to Phone Numbers
# 2. Click "Buy a Number"
# 3. Select country (US, UK, etc.)
# 4. Select features: SMS
# 5. Buy number (usually $1/month)
#    Example: +1-xxx-xxx-xxxx
```

### Step 3: Get API Credentials

```bash
# In Twilio Console → Account:
# 1. Find "Account SID": AC...
# 2. Find "Auth Token": (keep secret!)
# 3. Find "Twilio Phone Number": +1-xxx-xxx-xxxx

# Add to .env:
TWILIO_ACCOUNT_SID="ACxxxxxxxxxxxxx"
TWILIO_AUTH_TOKEN="xxxxxxxxxxxxxxxxxxx"
TWILIO_FROM_NUMBER="+1-xxx-xxx-xxxx"

# Export to production:
export TWILIO_ACCOUNT_SID="AC..."
export TWILIO_AUTH_TOKEN="..."
export TWILIO_FROM_NUMBER="+1..."
```

### Step 4: Configure Numbers (International)

```bash
# In Twilio Console → Programmable SMS:

# Add numbers for different regions:
# US/Canada: +1-xxx-xxx-xxxx
# UK: +44-xxxx-xxxx
# EU: +31-x-xxxxxxxx
# APAC: +61-x-xxxx-xxxx

# Environment variable for primary:
PRIMARY_SMS_NUMBER="+1-xxx-xxx-xxxx"
```

### Step 5: Test SMS Sending

```rust
// In src/api/notifications_production.rs
impl TwilioSmsClient {
    pub async fn send_payment_alert(
        &self,
        phone_number: &str,
        amount: &str,
        chain: &str,
    ) -> AppResult<String> {
        let message = format!(
            "CrossChain Payment Alert: {} {} payment received on {}. Check your account for details.",
            amount, chain, chain
        );

        self.send_sms(phone_number, &message).await
    }
}
```

### Step 6: Configure Webhooks (Optional)

```bash
# In Twilio Console → Phone Numbers → Configure:
# Message Callback URL: https://your-api.com/webhooks/twilio/sms
# HTTP POST

# Backend webhook handler:
POST /webhooks/twilio/sms
{
  "MessageSid": "SMxxxxxx",
  "From": "+1-xxx-xxx-xxxx",
  "To": "+1-yyy-yyy-yyyy",
  "Body": "Message content",
  "MessageStatus": "delivered|failed|undelivered"
}
```

### Twilio Monitoring

```bash
# In Twilio Console:
# - Logs: see all SMS sent/received
# - Analytics: delivery rates
# - Spending: how much spent on SMS
# - Errors: bounces, failures

# Cost example:
# US SMS: $0.01 per message
# International: $0.01-$0.05 per message
# 1000 SMS/month to US: ~$10
```

---

## 🗄️ DATABASE SCHEMA FOR NOTIFICATIONS

```sql
-- Create notifications table for persistence
CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    channel VARCHAR(50) NOT NULL, -- 'email', 'push', 'sms'
    priority VARCHAR(50) NOT NULL, -- 'low', 'normal', 'high'
    recipient VARCHAR(255) NOT NULL, -- email, phone, device token
    subject VARCHAR(255) NOT NULL,
    body TEXT NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- 'pending', 'sent', 'failed', 'bounced'
    external_id VARCHAR(255), -- Resend/Twilio/Firebase ID
    retry_count INTEGER DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    sent_at TIMESTAMP WITH TIME ZONE,
    
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Indexes for efficient querying
CREATE INDEX idx_notifications_status ON notifications(status);
CREATE INDEX idx_notifications_user_id ON notifications(user_id);
CREATE INDEX idx_notifications_channel ON notifications(channel);
CREATE INDEX idx_notifications_created_at ON notifications(created_at);

-- Notification delivery log (for analytics)
CREATE TABLE notification_delivery_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    notification_id UUID NOT NULL,
    attempt_number INTEGER NOT NULL,
    status VARCHAR(50) NOT NULL, -- 'success', 'failed', 'retry'
    error_message TEXT,
    duration_ms INTEGER,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    FOREIGN KEY (notification_id) REFERENCES notifications(id)
);

-- User notification preferences
CREATE TABLE notification_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL UNIQUE,
    email_enabled BOOLEAN DEFAULT TRUE,
    push_enabled BOOLEAN DEFAULT TRUE,
    sms_enabled BOOLEAN DEFAULT FALSE, -- Opt-in only
    device_tokens TEXT[] DEFAULT ARRAY[]::TEXT[], -- Push tokens
    phone_number VARCHAR(20), -- For SMS
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    FOREIGN KEY (user_id) REFERENCES users(id)
);
```

---

## 🔧 ENVIRONMENT VARIABLES SUMMARY

```bash
# Email (Resend)
RESEND_API_KEY="re_xxxxxxxxxxxxx"
RESEND_FROM_EMAIL="noreply@your-domain.com"

# Push (Firebase)
FIREBASE_PROJECT_ID="crosschain-payments"
FIREBASE_CREDENTIALS_PATH="./firebase-credentials.json"
FIREBASE_ACCESS_TOKEN="ya29.xxxxx"

# SMS (Twilio)
TWILIO_ACCOUNT_SID="ACxxxxxxxxxxxxx"
TWILIO_AUTH_TOKEN="xxxxxxxxxxxxxxxx"
TWILIO_FROM_NUMBER="+1-xxx-xxx-xxxx"

# Database
DATABASE_URL="postgresql://user:pass@localhost/crosschain"

# Notification settings
NOTIFICATION_MAX_RETRIES=3
NOTIFICATION_RETRY_DELAY_SECONDS=300
NOTIFICATION_BATCH_SIZE=10
```

---

## 📊 Notification Flow Architecture

```
User Action (Quote Created)
    ↓
Event Published to Queue
    ↓
Notification Service Consumes Event
    ↓
Persist to Database (outbox pattern)
    ↓
Send via Provider (Email/Push/SMS)
    ↓
Update Status in Database
    ↓
Retry Logic (exponential backoff)
    ↓
Final Status (sent/failed/bounced)
    ↓
Log to Analytics
    ↓
Alert ops if critical failure
```

---

## 🧪 Testing Notifications

### Test Email (Resend)

```bash
# Create test recipient email first
# Resend allows sending to any email in test mode

curl -X POST https://api.resend.com/emails \
  -H "Authorization: Bearer re_xxxxx" \
  -d '{
    "from": "noreply@your-domain.com",
    "to": "test@example.com",
    "subject": "Test Email",
    "html": "<h1>Test</h1>"
  }'
```

### Test SMS (Twilio)

```bash
# Use Twilio test credentials (available in console)
# Messages go to registered phone only

curl -X POST https://api.twilio.com/2010-04-01/Accounts/AC123/Messages.json \
  -u "$TWILIO_ACCOUNT_SID:$TWILIO_AUTH_TOKEN" \
  -d "From=+1-xxx-xxx-xxxx&To=+1-yyy-yyy-yyyy&Body=Test"
```

### Test Push (Firebase)

```bash
# Use Firebase console to send test message
# Or use REST API with valid device token

curl -X POST https://fcm.googleapis.com/v1/projects/PROJECT_ID/messages:send \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -d '{
    "message": {
      "token": "device-token",
      "notification": {
        "title": "Test",
        "body": "Test message"
      }
    }
  }'
```

---

## 🚨 Monitoring & Alerts

### Key Metrics to Monitor

```
Email (Resend):
- Delivery rate (target: >99%)
- Bounce rate (target: <0.5%)
- Open rate (target: >20%)
- Complaint rate (target: <0.1%)

Push (Firebase):
- Delivery rate (target: >95%)
- Click-through rate
- Uninstall rate
- Error rate (target: <1%)

SMS (Twilio):
- Delivery rate (target: >98%)
- Bounce rate (target: <1%)
- Cost per message
- Unsubscribe rate
```

### Alert Rules

```yaml
alerts:
  - name: email_delivery_low
    threshold: delivery_rate < 95%
    action: page_on_call
    
  - name: sms_cost_spike
    threshold: cost > daily_avg * 3
    action: alert_finance_team
    
  - name: push_tokens_stale
    threshold: > 30% invalid_tokens
    action: alert_mobile_team
    
  - name: notifications_pending
    threshold: pending_count > 1000
    action: investigate_queue
```

---

## ✅ Deployment Checklist

- [ ] Resend account created and API key secured
- [ ] Firebase project created and credentials stored
- [ ] Twilio account created and phone number purchased
- [ ] All environment variables set in production
- [ ] Database schema created with notifications table
- [ ] Email templates created and tested
- [ ] Push notification client set up
- [ ] SMS number obtained and configured
- [ ] Webhooks configured for delivery receipts
- [ ] Retry logic implemented
- [ ] Monitoring and alerts configured
- [ ] Fallback channels defined
- [ ] Privacy policy updated
- [ ] Terms of service updated

---

**Next**: Update `src/api/mod.rs` to export `notifications_production` module and integrate into main application.
