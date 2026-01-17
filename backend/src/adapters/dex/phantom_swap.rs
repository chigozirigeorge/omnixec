use async_trait::async_trait;
use rust_decimal::Decimal;
use crate::adapters::traits::{AssetInfo, DexAdapter, PriceQuote, SwapRequest, SwapResult, SwapStatus};
use crate::error::{AppResult, ExecutionError};
use crate::ledger::models::Chain;
use chrono::Utc;
use std::str::FromStr;

pub struct PhantomSwapAdapter {
    rpc_url: String,
}

impl PhantomSwapAdapter {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }
    
    fn get_common_tokens() -> Vec<AssetInfo> {
        vec![
            AssetInfo {
                chain: Chain::Stellar,
                address: "native".to_string(),
                symbol: "XLM".to_string(),
                name: "Stellar Lumens".to_string(),
                decimals: 7,
                logo_url: Some("https://raw.githubusercontent.com/stellar/docs/master/static/img/stellar-logo.png".to_string()),
            },
            AssetInfo {
                chain: Chain::Stellar,
                address: "USDC:GA5ZSEJYB37JRC5AVCIA5MOP4IHTZMWYRZUW3HJSTENCEWXSEOFWYFFM".to_string(),
                symbol: "USDC".to_string(),
                name: "USD Coin".to_string(),
                decimals: 6,
                logo_url: None,
            },
        ]
    }
}

#[async_trait]
impl DexAdapter for PhantomSwapAdapter {
    fn name(&self) -> &'static str {
        "PhantomSwap"
    }
    
    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Stellar]
    }
    
    async fn get_price(
        &self,
        asset_in: &AssetInfo,
        asset_out: &AssetInfo,
        amount: Decimal,
    ) -> AppResult<PriceQuote> {
        if asset_in.chain != Chain::Stellar || asset_out.chain != Chain::Stellar {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: Chain::Stellar,
                message: "PhantomSwap only supports Stellar".to_string(),
            }.into());
        }
        
        let amount_out = amount * Decimal::from_str("1.0").unwrap();
        let rate = amount_out / amount;
        let slippage = Decimal::from_str("0.3").unwrap();
        
        Ok(PriceQuote {
            asset_in: asset_in.clone(),
            asset_out: asset_out.clone(),
            amount_in: amount,
            amount_out,
            rate,
            dex_name: "PhantomSwap".to_string(),
            chain: Chain::Stellar,
            slippage_percent: slippage,
            execution_time_seconds: 15,
            liquidity_available: Decimal::from(500_000),
            timestamp: Utc::now().timestamp(),
        })
    }
    
    async fn get_supported_assets(&self, chain: Chain) -> AppResult<Vec<AssetInfo>> {
        if chain != Chain::Stellar {
            return Err(ExecutionError::ChainExecutionFailed {
                chain,
                message: "PhantomSwap only supports Stellar".to_string(),
            }.into());
        }
        
        Ok(Self::get_common_tokens())
    }
    
    async fn swap(&self, request: SwapRequest) -> AppResult<SwapResult> {
        if request.asset_in.chain != Chain::Stellar {
            return Err(ExecutionError::ChainExecutionFailed {
                chain: request.asset_in.chain,
                message: "PhantomSwap only supports Stellar swaps".to_string(),
            }.into());
        }
        
        let amount_out = request.amount_in * Decimal::from_str("1.0").unwrap();
        let actual_rate = amount_out / request.amount_in;
        
        let tx_hash = format!(
            "{}",
            uuid::Uuid::new_v4().to_string().replace("-", "")
        );
        
        Ok(SwapResult {
            transaction_hash: tx_hash,
            amount_in: request.amount_in,
            amount_out,
            actual_rate,
            gas_fee: Some(Decimal::from_str("0.00001").unwrap()),
            status: SwapStatus::Pending,
        })
    }
    
    async fn estimate_gas(&self, _asset_in: &AssetInfo, _asset_out: &AssetInfo) -> AppResult<Decimal> {
        Ok(Decimal::from_str("0.00001").unwrap())
    }
}
