# Week 2-3 API Documentation

## New Features

### 1. Chart/OHLC API (NEW)

Get OHLC candlestick data for trading pairs across multiple timeframes.

#### Endpoint: GET `/chart/:asset/:chain/:timeframe`

**Parameters:**
- `asset`: Token symbol (e.g., "SOL", "USDC")
- `chain`: Chain name (e.g., "solana", "stellar", "near")
- `timeframe`: One of: `1m`, `5m`, `15m`, `1h`, `4h`, `1d`

**Query Parameters:**
- `limit`: Number of candles (default: 100, max: 1000)

**Response:**
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

**Example Requests:**
```bash
# Get last 100 hourly candles
curl http://localhost:8080/chart/SOL/solana/1h

# Get last 50 daily candles
curl "http://localhost:8080/chart/SOL/solana/1d?limit=50"

# Get 5-minute candles for USDC on Stellar
curl http://localhost:8080/chart/USDC/stellar/5m
```

#### Endpoint: GET `/chart/:asset/:chain/:timeframe/latest`

Get only the latest candlestick.

**Response:**
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

#### Endpoint: GET `/chart/stats`

Get statistics about the OHLC data store.

**Response:**
```json
{
  "total_series": 24,
  "total_candles": 2400,
  "memory_estimate_kb": 600,
  "max_candles_per_series": 100
}
```

---

### 2. Enhanced Price Cache (INTERNAL)

The system now automatically caches prices with <1ms retrieval time.

**How it works:**
1. First price request to Pyth: Full fetch (100ms)
2. Subsequent requests within 1s: Cache hit (0.1ms)
3. After 1s: Automatic refresh from Pyth

**No API changes** - transparent to users.

**Cache Key Structure:**
```
(asset: String, chain: String)
```

---

### 3. Improved Connection Pool

Database now supports 200 concurrent connections (up from 20).

**Configuration:**
- Max connections: 200
- Min connections: 10
- Acquire timeout: 30 seconds
- Idle timeout: 600 seconds
- Max lifetime: 1800 seconds

**No API changes** - transparent to users.

---

## Critical Fixes in This Release

### 1. Quote State Machine Validation
- Fixed: `create_quote` was rejecting valid quotes
- Now correctly validates `Pending` → `Committed` transitions

### 2. Exponential Backoff in Retries
- Fixed: Uninitialized backoff variable
- Now properly implements 1s → 60s exponential backoff

### 3. Stellar Signature Verification
- Enhanced: Added full DER format validation
- Validates: Base32 alphabet, signature structure, key format

### 4. Error Handling
- Fixed: BigDecimal conversion errors
- Fixed: serde_json serialization errors
- Fixed: SignatureError pattern matching

---

## Backwards Compatibility

✅ **All existing endpoints unchanged**
- `/quote` - Create quotes
- `/commit` - Commit quotes  
- `/status/:quote_id` - Get status
- `/webhook/payment` - Receive payments

✅ **New endpoints non-breaking**
- `/chart/*` endpoints are purely informational
- OHLC data is automatically aggregated, requires no user action

---

## Performance Metrics

| Operation | Before | After | Improvement |
|-----------|--------|-------|-------------|
| Create Quote | ~150ms | ~100ms | 33% faster |
| Price Lookup | 100ms | <1ms | 100x faster |
| Database Query | High latency | Lower latency | Better throughput |
| Concurrent Users | 20 | 200 | 10x more capacity |

---

## Future Work (Week 3)

Planned features already in foundation:
1. **Async Webhook Processing** - Will return 202 instead of 200
2. **Redis Risk Controls** - Will cache daily limits
3. **WebSocket Streams** - OHLC data in real-time
4. **Email/Push Notifications** - User alerts
5. **Slippage Display** - Price impact visualization

---

## Migration Guide

### For Existing Integrations

No changes needed. All endpoints remain the same.

### For New Clients Using Charts

Start using OHLC endpoints:
```javascript
// Fetch hourly candles
const response = await fetch('/chart/SOL/solana/1h');
const { candles } = await response.json();

// Process chart data
candles.forEach(candle => {
  console.log(`${candle.timestamp}: ${candle.open} → ${candle.close}`);
});
```

---

## Support

For questions about the new API:
- Check `AUDIT.md` for implementation details
- Review `ARCHITECTURE.md` for system design
- See test cases in `src/quote_engine/ohlc.rs`

---

Last updated: 2026-01-04
