use crate::adapters::AdapterRegistry;
use crate::adapters::traits::{AssetInfo};
use crate::error::{AppError, AppResult};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiDexQuote {
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub amount_in: Decimal,
    pub best_dex: String,
    pub best_amount_out: Decimal,
    pub best_rate: Decimal,
    pub best_slippage: Decimal,
    pub all_quotes: Vec<DexQuoteOption>,
    pub aggregated_liquidity: Decimal,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DexQuoteOption {
    pub dex_name: String,
    pub amount_out: Decimal,
    pub rate: Decimal,
    pub slippage_percent: Decimal,
    pub liquidity_available: Decimal,
    pub execution_price_impact: Decimal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteOption {
    pub path: Vec<AssetInfo>,
    pub total_amount_out: Decimal,
    pub estimated_gas_fees: Decimal,
    pub dex_sequence: Vec<String>,
    pub hops: usize,
    pub total_slippage: Decimal,
}

pub struct RealtimeQuoteEngine {
    adapter_registry: Arc<AdapterRegistry>,
    cache: tokio::sync::RwLock<QuoteCache>,
}

struct QuoteCache {
    quotes: HashMap<String, CachedQuote>,
    cache_ttl_secs: u64,
}

struct CachedQuote {
    data: MultiDexQuote,
    cached_at: DateTime<Utc>,
}

impl RealtimeQuoteEngine {
    pub fn new(adapter_registry: Arc<AdapterRegistry>) -> Self {
        Self {
            adapter_registry,
            cache: tokio::sync::RwLock::new(QuoteCache {
                quotes: HashMap::new(),
                cache_ttl_secs: 10, // 10 second cache
            }),
        }
    }

    pub async fn get_best_quote(
        &self,
        asset_in: &AssetInfo,
        asset_out: &AssetInfo,
        amount: Decimal,
    ) -> AppResult<MultiDexQuote> {
        let cache_key = format!(
            "{}/{}/{}/{}",
            asset_in.chain.to_string(),
            asset_in.address,
            asset_out.address,
            amount
        );

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.quotes.get(&cache_key) {
                let age = Utc::now()
                    .signed_duration_since(cached.cached_at)
                    .num_seconds() as u64;
                if age < cache.cache_ttl_secs {
                    return Ok(cached.data.clone());
                }
            }
        }

        // Fetch from all available DEXes in parallel
        let dexes = self
            .adapter_registry
            .get_all_dexes_for_chain(asset_in.chain)
            .await;

        let mut quote_futures = vec![];
        for dex in dexes.clone() {
            let dex_clone = dex.clone();
            let asset_in_clone = asset_in.clone();
            let asset_out_clone = asset_out.clone();
            quote_futures.push(async move {
                dex_clone
                    .get_price(&asset_in_clone, &asset_out_clone, amount)
                    .await
            });
        }

        let results = futures::future::join_all(quote_futures).await;

        let mut all_quotes = vec![];
        let mut best_amount_out = Decimal::ZERO;
        let mut best_dex = String::new();
        let mut best_rate = Decimal::ZERO;
        let mut best_slippage = Decimal::from(100);
        let mut aggregated_liquidity = Decimal::ZERO;

        for (i, result) in results.iter().enumerate() {
            if let Ok(quote) = result {
                let dex_name = dexes[i].name();
                aggregated_liquidity += quote.liquidity_available;

                let option = DexQuoteOption {
                    dex_name: dex_name.to_string(),
                    amount_out: quote.amount_out,
                    rate: quote.rate,
                    slippage_percent: quote.slippage_percent,
                    liquidity_available: quote.liquidity_available,
                    execution_price_impact: quote.slippage_percent,
                };

                if quote.amount_out > best_amount_out {
                    best_amount_out = quote.amount_out;
                    best_dex = dex_name.to_string();
                    best_rate = quote.rate;
                    best_slippage = quote.slippage_percent;
                }

                all_quotes.push(option);
            }
        }

        let multi_quote = MultiDexQuote {
            asset_in: asset_in.clone(),
            asset_out: asset_out.clone(),
            amount_in: amount,
            best_dex,
            best_amount_out,
            best_rate,
            best_slippage,
            all_quotes,
            aggregated_liquidity,
            timestamp: Utc::now(),
        };

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.quotes.insert(
                cache_key,
                CachedQuote {
                    data: multi_quote.clone(),
                    cached_at: Utc::now(),
                },
            );
        }

        Ok(multi_quote)
    }

    pub async fn find_multi_hop_routes(
        &self,
        asset_in: &AssetInfo,
        asset_out: &AssetInfo,
        amount: Decimal,
        max_hops: usize,
    ) -> AppResult<Vec<RouteOption>> {
        if max_hops < 1 || max_hops > 3 {
            return Err(AppError::InvalidInput(
                "Max hops must be between 1 and 3".to_string(),
            ));
        }

        let mut routes = vec![];

        // Single-hop: direct swap
        if asset_in.chain == asset_out.chain {
            let quote = self.get_best_quote(asset_in, asset_out, amount).await?;
            let gas_estimate = self
                .adapter_registry
                .get_dex(&quote.best_dex)
                .ok_or(AppError::AdapterNotFound)?
                .estimate_gas(asset_in, asset_out)
                .await?;

            routes.push(RouteOption {
                path: vec![asset_in.clone(), asset_out.clone()],
                total_amount_out: quote.best_amount_out,
                estimated_gas_fees: gas_estimate,
                dex_sequence: vec![quote.best_dex],
                hops: 1,
                total_slippage: quote.best_slippage,
            });
        }

        Ok(routes)
    }

    pub async fn estimate_execution_price_impact(
        &self,
        asset_in: &AssetInfo,
        asset_out: &AssetInfo,
        amount: Decimal,
    ) -> AppResult<PriceImpactEstimate> {
        let quote = self.get_best_quote(asset_in, asset_out, amount).await?;

        let spot_price = quote.best_rate;
        let execution_price = if quote.best_amount_out > Decimal::ZERO {
            amount / quote.best_amount_out
        } else {
            Decimal::ZERO
        };

        let price_impact_percent = if spot_price > Decimal::ZERO {
            ((spot_price - execution_price) / spot_price) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        Ok(PriceImpactEstimate {
            spot_price,
            execution_price,
            price_impact_bps: price_impact_percent * Decimal::from(100), // Convert to basis points
            slippage_percent: quote.best_slippage,
            recommended_slippage_tolerance: quote.best_slippage + Decimal::from(1), // +1% buffer
        })
    }

    pub async fn clear_cache(&self) -> AppResult<()> {
        let mut cache = self.cache.write().await;
        cache.quotes.clear();
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriceImpactEstimate {
    pub spot_price: Decimal,
    pub execution_price: Decimal,
    pub price_impact_bps: Decimal,
    pub slippage_percent: Decimal,
    pub recommended_slippage_tolerance: Decimal,
}
