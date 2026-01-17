use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};
use rust_decimal::Decimal;
use std::str::FromStr;
use chrono::{DateTime, Utc};

/// Pyth price feed identifiers for different chains/assets
pub struct PythPriceFeedIds {
    /// Solana mainnet price feed IDs
    pub solana_mainnet: HashMap<String, String>,
    /// Solana devnet price feed IDs
    pub solana_devnet: HashMap<String, String>,
    /// Stellar mainnet feed IDs
    pub stellar_mainnet: HashMap<String, String>,
    /// Stellar testnet feed IDs
    pub stellar_testnet: HashMap<String, String>,
    /// NEAR mainnet feed IDs
    pub near_mainnet: HashMap<String, String>,
    /// NEAR testnet feed IDs
    pub near_testnet: HashMap<String, String>,
}

impl PythPriceFeedIds {
    pub fn new() -> Self {
        let mut solana_mainnet = HashMap::new();
        let mut solana_devnet = HashMap::new();
        let mut stellar_mainnet = HashMap::new();
        let mut stellar_testnet = HashMap::new();
        let mut near_mainnet = HashMap::new();
        let mut near_testnet = HashMap::new();

        // Solana Mainnet Price Feed IDs
        solana_mainnet.insert(
            "SOL".to_string(),
            "H6ARHf6YXhGrYZZAr3qLq9NNUyRfQAccJFvvrH8B8io2".to_string(),
        );
        solana_mainnet.insert(
            "USDC".to_string(),
            "Gnt27xtC473ZT2Mw5u8wZ68Z3gULkSTb5DuxJN2UYLw6".to_string(),
        );
        solana_mainnet.insert(
            "USDT".to_string(),
            "3Mnn2nBVDwZfWvkjvfYrg57MkeoM6P25TpzxSAzxnUk9".to_string(),
        );
        solana_mainnet.insert(
            "ETH".to_string(),
            "JF3hscKv78LspujqqVANcNhalysumGKPQmdarumdQLe".to_string(),
        );
        solana_mainnet.insert(
            "BTC".to_string(),
            "GVXRSv1FM36is6iNcumEjVc2U2iJ7Gx3bBrYb6Ax9rC2".to_string(),
        );
        solana_mainnet.insert(
            "XLM".to_string(),
            "3Mnn2nBVDwZfWvkjvfYrg57MkeoM6P25TpzxSAzxnUk9".to_string(),
        );

        // Solana Devnet Price Feed IDs
        solana_devnet.insert(
            "SOL".to_string(),
            "J83w4HKfqxwcq3BEMMkPFSppjv3JW7MVvQ7gdNot2gS7".to_string(),
        );
        solana_devnet.insert(
            "USDC".to_string(),
            "5VAAA8Nm2kDMwkxwqHSEfQD87Lsmz8GPg6gsQkQr5S1j".to_string(),
        );
        solana_devnet.insert(
            "XLM".to_string(),
            "8GWTTbNiXXoUctiP6AoN9KaFbFreKr6MkbHRNatXjkxJ".to_string(),
        );

        // Stellar Mainnet Feed IDs (via Soroban bridge)
        stellar_mainnet.insert(
            "SOL".to_string(),
            "c1251ae89f0f6e1a645f5953aff3b6eff41db4e4".to_string(),
        );
        stellar_mainnet.insert(
            "USDC".to_string(),
            "c1251ae89f0f6e1a645f5953aff3b6eff41db4e4".to_string(),
        );
        stellar_mainnet.insert(
            "XLM".to_string(),
            "native".to_string(),
        );

        // Stellar Testnet Feed IDs
        stellar_testnet.insert(
            "SOL".to_string(),
            "c1251ae89f0f6e1a645f5953aff3b6eff41db4e4".to_string(),
        );
        stellar_testnet.insert(
            "USDC".to_string(),
            "c1251ae89f0f6e1a645f5953aff3b6eff41db4e4".to_string(),
        );
        stellar_testnet.insert(
            "XLM".to_string(),
            "native".to_string(),
        );

        // NEAR Mainnet Feed IDs
        near_mainnet.insert(
            "NEAR".to_string(),
            "near.priceoracle".to_string(),
        );
        near_mainnet.insert(
            "USDC".to_string(),
            "oracle.near".to_string(),
        );
        near_mainnet.insert(
            "USDT".to_string(),
            "oracle.near".to_string(),
        );

        // NEAR Testnet Feed IDs
        near_testnet.insert(
            "NEAR".to_string(),
            "priceoracle.testnet".to_string(),
        );
        near_testnet.insert(
            "USDC".to_string(),
            "oracle.testnet".to_string(),
        );

        Self {
            solana_mainnet,
            solana_devnet,
            stellar_mainnet,
            stellar_testnet,
            near_mainnet,
            near_testnet,
        }
    }

    pub fn get_feed_id(&self, asset: &str, chain: &str, network: &str) -> Option<String> {
        let map = match (chain, network) {
            ("solana", "mainnet") => &self.solana_mainnet,
            ("solana", _) => &self.solana_devnet,
            ("stellar", "mainnet") => &self.stellar_mainnet,
            ("stellar", _) => &self.stellar_testnet,
            ("near", "mainnet") => &self.near_mainnet,
            ("near", _) => &self.near_testnet,
            _ => return None,
        };
        map.get(asset).cloned()
    }
}

/// Response from Pyth REST API
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PythPriceResponse {
    pub id: String,
    pub price: PythPrice,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PythPrice {
    #[serde(rename = "price")]
    pub price: String,
    #[serde(rename = "conf")]
    pub confidence: String,
    #[serde(rename = "expo")]
    pub exponent: i32,
    #[serde(rename = "publish_time")]
    pub publish_time: i64,
}

impl PythPrice {
    /// Convert Pyth price to decimal (handle exponent)
    pub fn to_decimal(&self) -> Result<Decimal, Box<dyn std::error::Error>> {
        let price = Decimal::from_str(&self.price)?;
        let exponent = self.exponent;

        let adjusted = if exponent < 0 {
            price / Decimal::from(10i64.pow((-exponent) as u32))
        } else {
            price * Decimal::from(10i64.pow(exponent as u32))
        };

        Ok(adjusted)
    }

    /// Get confidence interval as percentage
    pub fn confidence_pct(&self) -> Result<Decimal, Box<dyn std::error::Error>> {
        let conf = Decimal::from_str(&self.confidence)?;
        let price = Decimal::from_str(&self.price)?;

        Ok((conf / price) * Decimal::from(100))
    }

    /// Check if price is fresh (less than 5 seconds old)
    pub fn is_fresh(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        (now - self.publish_time).abs() < 5
    }
}

/// Complete price data for a pair with timestamp
#[derive(Debug, Clone)]
pub struct PythPriceData {
    pub base: String,
    pub quote: String,
    pub rate: Decimal,
    pub base_price: PythPrice,
    pub quote_price: PythPrice,
    pub timestamp: DateTime<Utc>,
}

impl PythPriceData {
    /// Calculate amount out with slippage protection
    pub fn calculate_output(&self, amount_in: Decimal, slippage_pct: Decimal) -> Decimal {
        let output = amount_in * self.rate;
        let slippage_amount = output * (slippage_pct / Decimal::from(100));
        output - slippage_amount
    }

    /// Get confidence bounds for output
    pub fn output_confidence_bounds(
        &self,
        amount_in: Decimal,
    ) -> Result<(Decimal, Decimal), Box<dyn std::error::Error>> {
        let base_conf_pct = self.base_price.confidence_pct()?;
        let quote_conf_pct = self.quote_price.confidence_pct()?;

        let total_conf_pct = base_conf_pct + quote_conf_pct;

        let output = amount_in * self.rate;
        let conf_amount = output * (total_conf_pct / Decimal::from(100));

        Ok((output - conf_amount, output + conf_amount))
    }
}

/// Pyth Oracle Client - supports multiple chains
pub struct PythOracle {
    client: Client,
    base_url: String,
    network: String,
    feed_ids: PythPriceFeedIds,
    cache: std::sync::Arc<parking_lot::RwLock<HashMap<String, (PythPriceData, DateTime<Utc>)>>>,
}

impl PythOracle {
    pub fn new(network: &str) -> Self {
        let base_url = match network {
            "mainnet" => "https://hermes.pyth.network",
            _ => "https://hermes-beta.pyth.network",
        };

        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            network: network.to_string(),
            feed_ids: PythPriceFeedIds::new(),
            cache: std::sync::Arc::new(parking_lot::RwLock::new(HashMap::new())),
        }
    }

    /// Get real-time price for asset pair on specific chain
    pub async fn get_price(
        &self,
        base: &str,
        quote: &str,
        chain: &str,
    ) -> Result<PythPriceData, Box<dyn std::error::Error>> {
        // Check cache first (5 second TTL)
        let cache_key = format!("{}/{}/{}", base, quote, chain);
        {
            let cache = self.cache.read();
            if let Some((data, timestamp)) = cache.get(&cache_key) {
                let age = Utc::now().signed_duration_since(*timestamp);
                if age.num_seconds() < 5 {
                    info!(
                        "✓ Cache hit for {}/{} on {} (age: {}s)",
                        base,
                        quote,
                        chain,
                        age.num_seconds()
                    );
                    return Ok(data.clone());
                }
            }
        }

        // Get fresh price from Pyth
        let base_feed_id = self
            .feed_ids
            .get_feed_id(base, chain, &self.network)
            .ok_or(format!("Unknown asset: {} on {}", base, chain))?;

        let quote_feed_id = self
            .feed_ids
            .get_feed_id(quote, chain, &self.network)
            .ok_or(format!("Unknown asset: {} on {}", quote, chain))?;

        let base_price = self.fetch_price(&base_feed_id).await?;
        let quote_price = self.fetch_price(&quote_feed_id).await?;

        if !base_price.is_fresh() || !quote_price.is_fresh() {
            warn!(
                "Stale price data for {}/{}: base age={}s, quote age={}s",
                base,
                quote,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
                    - base_price.publish_time,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
                    - quote_price.publish_time
            );
        }

        let base_decimal = base_price.to_decimal()?;
        let quote_decimal = quote_price.to_decimal()?;
        let rate = base_decimal / quote_decimal;

        let price_data = PythPriceData {
            base: base.to_string(),
            quote: quote.to_string(),
            rate,
            base_price,
            quote_price,
            timestamp: Utc::now(),
        };

        // Update cache
        {
            let mut cache = self.cache.write();
            cache.insert(cache_key, (price_data.clone(), Utc::now()));
        }

        info!(
            "✓ Fetched price: {} {} = {} {} (confidence: {}%)",
            price_data.rate,
            base,
            1,
            quote,
            price_data
                .base_price
                .confidence_pct()
                .unwrap_or_default()
        );

        Ok(price_data)
    }

    /// Get price for all pairs needed for a transaction
    pub async fn get_multi_price(
        &self,
        pairs: Vec<(&str, &str, &str)>,
    ) -> Result<Vec<PythPriceData>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();
        for (base, quote, chain) in pairs {
            results.push(self.get_price(base, quote, chain).await?);
        }
        Ok(results)
    }

    async fn fetch_price(&self, feed_id: &str) -> Result<PythPrice, Box<dyn std::error::Error>> {
        let url = format!("{}/api/latest_price_feeds?ids={}", self.base_url, feed_id);

        let response = self.client.get(&url).send().await?;
        let body: serde_json::Value = response.json().await?;

        if let Some(prices) = body.get("prices").and_then(|p| p.as_array()) {
            if let Some(first) = prices.first() {
                let price_obj = first
                    .get("price")
                    .ok_or("Missing price field")?
                    .clone();

                let price: PythPrice = serde_json::from_value(price_obj)?;
                return Ok(price);
            }
        }

        Err("No price data in response".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_exponent_handling() {
        let price = PythPrice {
            price: "50000".to_string(),
            confidence: "100".to_string(),
            exponent: -8,
            publish_time: 1234567890,
        };

        let decimal = price.to_decimal().unwrap();
        assert_eq!(decimal, Decimal::from_str("0.5").unwrap());
    }

    #[test]
    fn test_confidence_interval() {
        let price = PythPrice {
            price: "1000".to_string(),
            confidence: "10".to_string(),
            exponent: -2,
            publish_time: 1234567890,
        };

        let conf = price.confidence_pct().unwrap();
        assert!(conf > Decimal::ZERO);
    }
}
