use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{debug};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

/// Price update event for WebSocket streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdate {
    pub asset: String,
    pub chain: String,
    pub price: Decimal,
    pub confidence: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// OHLC candle update for WebSocket streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhlcUpdate {
    pub asset: String,
    pub chain: String,
    pub timeframe: String,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub timestamp: u64,
}

/// WebSocket broadcast channel capacity
const BROADCAST_CAPACITY: usize = 1000;

/// WebSocket price feed broadcaster
/// Allows real-time streaming of price and OHLC updates to multiple clients
pub struct PriceFeedBroadcaster {
    /// Channel for price updates
    price_tx: broadcast::Sender<PriceUpdate>,
    /// Channel for OHLC updates
    ohlc_tx: broadcast::Sender<OhlcUpdate>,
}

impl PriceFeedBroadcaster {
    pub fn new() -> Self {
        let (price_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (ohlc_tx, _) = broadcast::channel(BROADCAST_CAPACITY);

        Self { price_tx, ohlc_tx }
    }

    /// Broadcast a price update to all subscribers
    pub fn broadcast_price(&self, update: PriceUpdate) {
        let _ = self.price_tx.send(update.clone());
        debug!("📡 Broadcast price: {}/{} = {}", update.asset, update.chain, update.price);
    }

    /// Broadcast an OHLC candle update to all subscribers
    pub fn broadcast_ohlc(&self, update: OhlcUpdate) {
        let _ = self.ohlc_tx.send(update.clone());
        debug!("📊 Broadcast OHLC: {} {} {}", update.asset, update.chain, update.timeframe);
    }

    /// Subscribe to price updates
    pub fn subscribe_prices(&self) -> broadcast::Receiver<PriceUpdate> {
        self.price_tx.subscribe()
    }

    /// Subscribe to OHLC updates
    pub fn subscribe_ohlc(&self) -> broadcast::Receiver<OhlcUpdate> {
        self.ohlc_tx.subscribe()
    }

    /// Get number of price subscribers
    pub fn price_subscriber_count(&self) -> usize {
        self.price_tx.receiver_count()
    }

    /// Get number of OHLC subscribers
    pub fn ohlc_subscriber_count(&self) -> usize {
        self.ohlc_tx.receiver_count()
    }
}

impl Default for PriceFeedBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn test_price_broadcast() {
        let broadcaster = PriceFeedBroadcaster::new();
        let mut rx = broadcaster.subscribe_prices();

        let update = PriceUpdate {
            asset: "SOL".to_string(),
            chain: "solana".to_string(),
            price: Decimal::from(100),
            confidence: Decimal::from(1),
            timestamp: Utc::now(),
        };

        broadcaster.broadcast_price(update.clone());

        let received = rx.recv().await.unwrap();
        assert_eq!(received.asset, "SOL");
        assert_eq!(received.price, Decimal::from(100));
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let broadcaster = Arc::new(PriceFeedBroadcaster::new());

        let mut rx1 = broadcaster.subscribe_prices();
        let mut rx2 = broadcaster.subscribe_prices();

        let update = PriceUpdate {
            asset: "SOL".to_string(),
            chain: "solana".to_string(),
            price: Decimal::from(100),
            confidence: Decimal::from(1),
            timestamp: Utc::now(),
        };

        broadcaster.broadcast_price(update);

        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        assert_eq!(received1.asset, received2.asset);
    }
}
