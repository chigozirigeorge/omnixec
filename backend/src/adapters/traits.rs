use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use crate::error::AppResult;
use crate::execution::router::Executor;
use crate::ledger::models::Chain;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AssetInfo {
    pub chain: Chain,
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub logo_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceQuote {
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub rate: Decimal,
    pub dex_name: String,
    pub chain: Chain,
    pub slippage_percent: Decimal,
    pub execution_time_seconds: u64,
    pub liquidity_available: Decimal,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapRequest {
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub amount_in: Decimal,
    pub recipient_address: String,
    pub slippage_tolerance: Decimal,
    pub sender_address: String,
    pub sender_private_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapResult {
    pub transaction_hash: String,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub actual_rate: Decimal,
    pub gas_fee: Option<Decimal>,
    pub status: SwapStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SwapStatus {
    Pending,
    Confirmed,
    Failed,
}

#[async_trait]
pub trait DexAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    
    fn supported_chains(&self) -> Vec<Chain>;
    
    async fn get_price(
        &self,
        asset_in: &AssetInfo,
        asset_out: &AssetInfo,
        amount: Decimal,
    ) -> AppResult<PriceQuote>;
    
    async fn get_supported_assets(&self, chain: Chain) -> AppResult<Vec<AssetInfo>>;
    
    async fn swap(&self, request: SwapRequest) -> AppResult<SwapResult>;
    
    async fn estimate_gas(&self, asset_in: &AssetInfo, asset_out: &AssetInfo) -> AppResult<Decimal>;
    
    async fn is_available(&self) -> AppResult<bool> {
       Ok(true)
    }

}
