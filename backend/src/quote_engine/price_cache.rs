use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Cached price entry with timestamp
#[derive(Debug, Clone)]
pub struct CachedPrice {
    pub price: Decimal,
    pub confidence: Decimal,
    pub timestamp: DateTime<Utc>,
    pub age_ms: u64,
}

impl CachedPrice {
    /// Check if cache entry is still valid (< 1 second)
    pub fn is_valid(&self) -> bool {
        let age = Utc::now() - self.timestamp;
        age.num_milliseconds() < 1000 // Cache for 1 second = <1ms per quote
    }
}

/// Price cache key: (asset, chain)
type CacheKey = (String, String);

/// In-memory price cache with TTL
pub struct PriceCache {
    cache: Arc<RwLock<HashMap<CacheKey, CachedPrice>>>,
    ttl_ms: u64,
}

impl PriceCache {
    pub fn new(ttl_ms: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl_ms,
        }
    }

    /// Get cached price if it exists and is valid
    pub async fn get(&self, asset: &str, chain: &str) -> Option<CachedPrice> {
        let key = (asset.to_string(), chain.to_string());
        let cache = self.cache.read().await;

        if let Some(entry) = cache.get(&key) {
            if entry.is_valid() {
                debug!("✓ Price cache hit: {} on {}", asset, chain);
                return Some(entry.clone());
            } else {
                debug!("⚠ Price cache stale: {} on {}", asset, chain);
                return None;
            }
        }

        None
    }

    /// Set cached price
    pub async fn set(&self, asset: &str, chain: &str, price: Decimal, confidence: Decimal) {
        let key = (asset.to_string(), chain.to_string());
        let entry = CachedPrice {
            price,
            confidence,
            timestamp: Utc::now(),
            age_ms: 0,
        };

        let mut cache = self.cache.write().await;
        cache.insert(key.clone(), entry);
        debug!("💾 Cached price: {} on {} = {}", asset, chain, price);
    }

    /// Clear cache (for testing or manual refresh)
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        info!("🔄 Price cache cleared");
    }

    /// Get cache size
    pub async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Cleanup expired entries
    pub async fn cleanup_expired(&self) {
        let now = Utc::now();
        let mut cache = self.cache.write().await;

        let before = cache.len();
        cache.retain(|_, entry| {
            let age = now - entry.timestamp;
            age.num_milliseconds() < self.ttl_ms as i64
        });
        let after = cache.len();

        if before > after {
            info!("🧹 Cleaned up {} expired price entries", before - after);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_price_cache() {
        let cache = PriceCache::new(5000); // 5 second TTL

        // Set a price
        cache.set("SOL", "solana", Decimal::from(100), Decimal::from(1)).await;

        // Retrieve immediately
        let cached = cache.get("SOL", "solana").await;
        assert!(cached.is_some());

        // Check size
        assert_eq!(cache.size().await, 1);

        // Clear cache
        cache.clear().await;
        assert_eq!(cache.size().await, 0);
    }

    #[tokio::test]
    async fn test_cache_expiry() {
        let cache = PriceCache::new(100); // 100ms TTL

        cache.set("SOL", "solana", Decimal::from(100), Decimal::from(1)).await;

        // Should be valid immediately
        assert!(cache.get("SOL", "solana").await.is_some());

        // Wait for expiry
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

        // Should be expired
        assert!(cache.get("SOL", "solana").await.is_none());
    }
}
