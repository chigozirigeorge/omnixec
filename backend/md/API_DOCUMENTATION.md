# Complete API Reference

## Base URL
```
http://localhost:8080
```

---

## Authentication
All endpoints require HTTPS in production. Currently uses IP whitelist for authorization.

---

## Quote Management API

### 1. Create Quote
**Endpoint:** `POST /quote`

**Description:** Generate a new cross-chain quote for a transaction.

**Request Body:**
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "funding_chain": "solana",
  "execution_chain": "stellar",
  "funding_asset": "SOL",
  "execution_asset": "USDC",
  "execution_instructions_base64": "base64_encoded_instruction_data",
  "estimated_compute_units": 200000
}
```

**Request Fields:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| user_id | UUID | Yes | User identifier |
| funding_chain | String | Yes | Source chain: `solana`, `stellar`, `near` |
| execution_chain | String | Yes | Destination chain (must differ from funding) |
| funding_asset | String | Yes | Source asset symbol (e.g., SOL, USDC) |
| execution_asset | String | Yes | Target asset symbol |
| execution_instructions_base64 | String | Yes | Base64-encoded execution instructions |
| estimated_compute_units | Integer | No | For Solana: compute units (1-1,400,000) |

**Response (200 OK):**
```json
{
  "quote_id": "660f9511-f3ac-42d5-b827-557766550111",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "funding_chain": "solana",
  "execution_chain": "stellar",
  "funding_asset": "SOL",
  "execution_asset": "USDC",
  "funding_amount": "100.00",
  "execution_amount": "1234.56",
  "execution_cost": "0.50",
  "service_fee": "0.10",
  "max_funding_amount": "100.50",
  "status": "Pending",
  "expires_at": 1704330000,
  "created_at": 1704326400
}
```

**Error Responses:**
- `400 Bad Request` - Invalid parameters
- `404 Not Found` - User not found
- `422 Unprocessable Entity` - Chain pair not supported

---

### 2. Commit Quote
**Endpoint:** `POST /commit`

**Description:** Commit a quote and lock funds for execution.

**Request Body:**
```json
{
  "quote_id": "660f9511-f3ac-42d5-b827-557766550111"
}
```

**Response (200 OK):**
```json
{
  "quote_id": "660f9511-f3ac-42d5-b827-557766550111",
  "status": "committed",
  "message": "Quote committed, funds locked, and execution initiated",
  "execution_chain": "stellar"
}
```

**Notes:**
- Funds are atomically locked
- Execution starts immediately in background
- Returns 200 OK with commitment confirmation

---

### 3. Get Quote Status
**Endpoint:** `GET /status/:quote_id`

**Description:** Get current status of a quote and its execution.

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| quote_id | UUID | The quote identifier |

**Response (200 OK):**
```json
{
  "quote_id": "660f9511-f3ac-42d5-b827-557766550111",
  "status": "Executed",
  "funding_amount": "100.00",
  "execution_amount": "1234.56",
  "funding_chain": "solana",
  "execution_chain": "stellar",
  "created_at": 1704326400,
  "executed_at": 1704327000,
  "transaction_hash": "xxxxxxxxxxxxxxxxxxxxxxxx",
  "execution_details": {
    "status": "Success",
    "fee_paid": "0.05",
    "confirmation_time_secs": 120
  }
}
```

**Possible Statuses:**
- `Pending` - Quote created, awaiting commitment
- `Committed` - Funds locked, execution in progress
- `Executed` - Successfully completed
- `Failed` - Execution failed
- `Expired` - Quote expired (15 min TTL)

---

## Chart/OHLC API

### 4. Get OHLC Candles
**Endpoint:** `GET /chart/:asset/:chain/:timeframe`

**Description:** Get historical candlestick (OHLC) data.

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| asset | String | Token symbol (SOL, USDC, etc.) |
| chain | String | Chain name (solana, stellar, near) |
| timeframe | String | 1m, 5m, 15m, 1h, 4h, 1d |

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| limit | Integer | 100 | Number of candles (max 1000) |

**Example Request:**
```bash
GET /chart/SOL/solana/1h?limit=24
```

**Response (200 OK):**
```json
{
  "asset": "SOL",
  "chain": "solana",
  "timeframe": "1h",
  "count": 24,
  "candles": [
    {
      "timestamp": 1704326400,
      "open": "100.00",
      "high": "105.50",
      "low": "99.25",
      "close": "104.75",
      "volume": "1500000",
      "trades": 342
    }
  ]
}
```

**Candle Fields:**
| Field | Type | Description |
|-------|------|-------------|
| timestamp | Integer | Unix timestamp (start of candle) |
| open | Decimal | Opening price |
| high | Decimal | Highest price in period |
| low | Decimal | Lowest price in period |
| close | Decimal | Closing price |
| volume | Decimal | Trading volume |
| trades | Integer | Number of trades |

---

### 5. Get Latest Candle
**Endpoint:** `GET /chart/:asset/:chain/:timeframe/latest`

**Description:** Get only the most recent candlestick.

**Response (200 OK):**
```json
{
  "timestamp": 1704330000,
  "open": "104.75",
  "high": "105.25",
  "low": "104.00",
  "close": "104.50",
  "volume": "50000",
  "trades": 12
}
```

---

### 6. Get Chart Statistics
**Endpoint:** `GET /chart/stats`

**Description:** Get OHLC data store statistics.

**Response (200 OK):**
```json
{
  "total_series": 24,
  "total_candles": 2400,
  "memory_estimate_kb": 600,
  "max_candles_per_series": 100
}
```

---

## Webhook API

### 7. Payment Webhook
**Endpoint:** `POST /webhook/payment`

**Description:** Receive payment completion notifications (202 Accepted).

**Request Body:**
```json
{
  "transaction_id": "tx_1234567890",
  "quote_id": "660f9511-f3ac-42d5-b827-557766550111",
  "status": "confirmed",
  "amount": "100.00",
  "timestamp": 1704327000
}
```

**Response (202 Accepted):**
```json
{
  "status": "accepted",
  "message": "Webhook received and queued for processing",
  "webhook_id": "wh_abc123xyz"
}
```

**Notes:**
- Returns immediately (202)
- Processing happens asynchronously
- No blocking on long operations

---

### 8. Chain-Specific Webhooks
**Endpoints:**
- `POST /webhook/stellar` - Stellar network notifications
- `POST /webhook/near` - NEAR network notifications
- `POST /webhook/solana` - Solana network notifications

**Format:** Same as Payment Webhook

---

## Treasury & Admin API

### 9. Get Treasury Balances
**Endpoint:** `GET /admin/treasury`

**Description:** Get treasury balances on all chains.

**Response (200 OK):**
```json
{
  "solana": {
    "balance": "10000.00",
    "currency": "SOL",
    "last_updated": 1704330000
  },
  "stellar": {
    "balance": "50000.00",
    "currency": "XLM",
    "last_updated": 1704330000
  },
  "near": {
    "balance": "5000.00",
    "currency": "NEAR",
    "last_updated": 1704330000
  }
}
```

---

### 10. Get Chain Treasury Balance
**Endpoint:** `GET /admin/treasury/:chain`

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| chain | String | Chain name: solana, stellar, near |

**Response (200 OK):**
```json
{
  "chain": "solana",
  "balance": "10000.00",
  "currency": "SOL",
  "last_updated": 1704330000,
  "reserved": "1000.00",
  "available": "9000.00"
}
```

---

## Health & Status API

### 11. Health Check
**Endpoint:** `GET /health`

**Description:** Check API health and readiness.

**Response (200 OK):**
```json
{
  "status": "healthy",
  "timestamp": 1704330000,
  "database": "connected",
  "cache": "operational",
  "risk_controls": "active",
  "version": "1.0.0"
}
```

---

## Error Handling

### Standard Error Response
All errors return appropriate HTTP status codes with JSON body:

```json
{
  "error": "InvalidParameters",
  "message": "Execution instructions cannot be empty",
  "request_id": "req_abc123xyz"
}
```

### Common Error Codes
| Code | Status | Description |
|------|--------|-------------|
| 400 | Bad Request | Invalid parameters |
| 401 | Unauthorized | Missing/invalid authentication |
| 404 | Not Found | Resource not found |
| 409 | Conflict | State conflict (e.g., quote already executed) |
| 422 | Unprocessable | Invalid chain pair or unsupported operation |
| 429 | Too Many Requests | Rate limited |
| 500 | Internal Server Error | Server error |
| 502 | Bad Gateway | External service error |
| 503 | Service Unavailable | Maintenance or overload |

---

## Rate Limiting

**Current Limits:**
- Quote creation: 1000 req/min per user
- Status checks: Unlimited
- Webhooks: Unlimited
- Chart API: 10000 req/min

**Rate Limit Headers:**
```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1704333600
```

---

## Examples

### Example 1: Create and Execute Quote

```bash
# 1. Create quote
curl -X POST http://localhost:8080/quote \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "550e8400-e29b-41d4-a716-446655440000",
    "funding_chain": "solana",
    "execution_chain": "stellar",
    "funding_asset": "SOL",
    "execution_asset": "USDC",
    "execution_instructions_base64": "...",
    "estimated_compute_units": 200000
  }'

# Response: {"quote_id": "660f9511-f3ac-42d5-b827-557766550111", ...}

# 2. Commit quote
curl -X POST http://localhost:8080/commit \
  -H "Content-Type: application/json" \
  -d '{
    "quote_id": "660f9511-f3ac-42d5-b827-557766550111"
  }'

# 3. Check status
curl http://localhost:8080/status/660f9511-f3ac-42d5-b827-557766550111
```

### Example 2: Get OHLC Chart Data

```bash
# Get last 100 hourly candles for SOL on Solana
curl http://localhost:8080/chart/SOL/solana/1h?limit=100

# Get latest 1-minute candle
curl http://localhost:8080/chart/SOL/solana/1m/latest

# Get 5-minute candles for USDC on Stellar
curl http://localhost:8080/chart/USDC/stellar/5m
```

---

## Pagination & Filtering

### Chart Pagination
Use `limit` parameter to control result size:
```bash
GET /chart/SOL/solana/1h?limit=50
```

### Time Range (Future)
```bash
GET /chart/SOL/solana/1h?start=1704326400&end=1704330000&limit=100
```

---

## WebSocket Streaming (Week 3)

### Subscribe to Price Updates
```javascript
const ws = new WebSocket('ws://localhost:8080/ws/prices');

ws.onmessage = (event) => {
  const update = JSON.parse(event.data);
  console.log(`${update.asset}: $${update.price}`);
};
```

### Subscribe to OHLC Updates
```javascript
const ws = new WebSocket('ws://localhost:8080/ws/ohlc');

ws.onmessage = (event) => {
  const candle = JSON.parse(event.data);
  console.log(`OHLC: ${candle.asset} ${candle.timeframe}`);
};
```

---

## SDKs & Client Libraries

### JavaScript/TypeScript
```bash
npm install crosschain-payments-sdk
```

### Python
```bash
pip install crosschain-payments
```

### Go
```bash
go get github.com/crosschain/payments-go
```

---

**Last Updated:** 2026-01-04
**Version:** 1.0.0
