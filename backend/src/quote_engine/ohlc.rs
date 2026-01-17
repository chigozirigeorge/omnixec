use chrono::{Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use crate::error::{AppResult};

/// OHLC (Open, High, Low, Close) candlestick data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OhlcCandle {
    /// Unix timestamp for the start of the candle
    pub timestamp: u64,
    /// Open price
    pub open: Decimal,
    /// High price (maximum)
    pub high: Decimal,
    /// Low price (minimum)
    pub low: Decimal,
    /// Close price
    pub close: Decimal,
    /// Trading volume
    pub volume: Decimal,
    /// Number of trades
    pub trades: u64,
}

impl OhlcCandle {
    pub fn new(timestamp: u64, price: Decimal) -> Self {
        Self {
            timestamp,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: Decimal::ZERO,
            trades: 0,
        }
    }

    /// Update candle with a new price point
    pub fn update(&mut self, price: Decimal, volume: Decimal) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += volume;
        self.trades += 1;
    }
}

/// Timeframe for OHLC data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    /// 1 minute
    #[serde(rename = "1m")]
    OneMinute,
    /// 5 minutes
    #[serde(rename = "5m")]
    FiveMinutes,
    /// 15 minutes
    #[serde(rename = "15m")]
    FifteenMinutes,
    /// 1 hour
    #[serde(rename = "1h")]
    OneHour,
    /// 4 hours
    #[serde(rename = "4h")]
    FourHours,
    /// 1 day
    #[serde(rename = "1d")]
    OneDay,
}

impl Timeframe {
    /// Get duration in seconds
    pub fn duration_secs(&self) -> u64 {
        match self {
            Timeframe::OneMinute => 60,
            Timeframe::FiveMinutes => 300,
            Timeframe::FifteenMinutes => 900,
            Timeframe::OneHour => 3600,
            Timeframe::FourHours => 14400,
            Timeframe::OneDay => 86400,
        }
    }

    /// Get bucket key for a timestamp
    pub fn bucket_key(&self, timestamp: u64) -> u64 {
        timestamp - (timestamp % self.duration_secs())
    }
}

/// OHLC data store key: (asset, chain, timeframe)
type OhlcKey = (String, String, Timeframe);

/// In-memory OHLC data store with time-series aggregation
pub struct OhlcStore {
    /// Candlestick data indexed by key and timestamp
    candles: Arc<RwLock<HashMap<OhlcKey, Vec<OhlcCandle>>>>,
    /// Maximum number of candles to keep per series
    max_candles_per_series: usize,
}

impl OhlcStore {
    pub fn new(max_candles_per_series: usize) -> Self {
        Self {
            candles: Arc::new(RwLock::new(HashMap::new())),
            max_candles_per_series,
        }
    }

    /// Add a price update to the OHLC store
    pub async fn add_price(&self, asset: &str, chain: &str, timeframe: Timeframe, price: Decimal, volume: Decimal) -> AppResult<()> {
        let key = (asset.to_string(), chain.to_string(), timeframe);
        let now = Utc::now().timestamp() as u64;
        let bucket = timeframe.bucket_key(now);

        let mut store = self.candles.write().await;
        let series = store.entry(key.clone()).or_insert_with(Vec::new);

        // Check if we need to create a new candle or update the last one
        if series.is_empty() || series.last().unwrap().timestamp != bucket {
            // Create new candle
            series.push(OhlcCandle::new(bucket, price));
        } else {
            // Update existing candle
            series.last_mut().unwrap().update(price, volume);
        }

        // Keep only the last N candles to prevent unbounded memory growth
        if series.len() > self.max_candles_per_series {
            series.remove(0);
        }

        debug!("📊 OHLC updated: {}/{} {:?} - bucket {}", asset, chain, timeframe, bucket);
        Ok(())
    }

    /// Get OHLC candles for a given range
    pub async fn get_candles(
        &self,
        asset: &str,
        chain: &str,
        timeframe: Timeframe,
        limit: Option<usize>,
    ) -> AppResult<Vec<OhlcCandle>> {
        let key = (asset.to_string(), chain.to_string(), timeframe);
        let store = self.candles.read().await;

        match store.get(&key) {
            Some(series) => {
                let limit = limit.unwrap_or(100).min(1000); // Max 1000 candles
                let start = series.len().saturating_sub(limit);
                Ok(series[start..].to_vec())
            }
            None => Ok(Vec::new()),
        }
    }

    /// Get the latest candle
    pub async fn get_latest_candle(
        &self,
        asset: &str,
        chain: &str,
        timeframe: Timeframe,
    ) -> AppResult<Option<OhlcCandle>> {
        let key = (asset.to_string(), chain.to_string(), timeframe);
        let store = self.candles.read().await;

        Ok(store.get(&key).and_then(|series| series.last().cloned()))
    }

    /// Clear all candles (for testing)
    pub async fn clear(&self) {
        let mut store = self.candles.write().await;
        store.clear();
        info!("🔄 OHLC store cleared");
    }

    /// Get store statistics
    pub async fn stats(&self) -> OhlcStoreStats {
        let store = self.candles.read().await;
        let total_series = store.len();
        let total_candles: usize = store.values().map(|s| s.len()).sum();

        OhlcStoreStats {
            total_series,
            total_candles,
            memory_estimate_kb: (total_candles * 256) / 1024, // Rough estimate: ~256 bytes per candle
        }
    }
}

/// Statistics about the OHLC store
#[derive(Debug, Clone, Serialize)]
pub struct OhlcStoreStats {
    pub total_series: usize,
    pub total_candles: usize,
    pub memory_estimate_kb: usize,
}

/// Request to get OHLC data
#[derive(Debug, Deserialize)]
pub struct OhlcRequest {
    pub asset: String,
    pub chain: String,
    pub timeframe: Timeframe,
    pub limit: Option<usize>,
}

/// OHLC API response
#[derive(Debug, Serialize)]
pub struct OhlcResponse {
    pub asset: String,
    pub chain: String,
    pub timeframe: Timeframe,
    pub candles: Vec<OhlcCandle>,
    pub count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeframe_duration() {
        assert_eq!(Timeframe::OneMinute.duration_secs(), 60);
        assert_eq!(Timeframe::OneHour.duration_secs(), 3600);
        assert_eq!(Timeframe::OneDay.duration_secs(), 86400);
    }

    #[test]
    fn test_timeframe_bucket() {
        let tf = Timeframe::FiveMinutes;
        let bucket = tf.bucket_key(1000);
        assert_eq!(bucket, 900); // Should align to 5-minute boundary
    }

    #[tokio::test]
    async fn test_ohlc_store() {
        let store = OhlcStore::new(100);

        // Add prices
        let price1 = Decimal::from(100);
        let price2 = Decimal::from(105);
        let price3 = Decimal::from(103);

        store.add_price("SOL", "solana", Timeframe::OneMinute, price1, Decimal::from(1000)).await.unwrap();
        store.add_price("SOL", "solana", Timeframe::OneMinute, price2, Decimal::from(2000)).await.unwrap();
        store.add_price("SOL", "solana", Timeframe::OneMinute, price3, Decimal::from(1500)).await.unwrap();

        // Get candles
        let candles = store.get_candles("SOL", "solana", Timeframe::OneMinute, None).await.unwrap();
        assert_eq!(candles.len(), 1); // Should have 1 candle since all prices are in same minute

        let candle = &candles[0];
        assert_eq!(candle.open, price1);
        assert_eq!(candle.high, price2);
        assert_eq!(candle.low, price1);
        assert_eq!(candle.close, price3);
    }
}
