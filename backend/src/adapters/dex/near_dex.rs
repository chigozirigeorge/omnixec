use crate::adapters::traits::{AssetInfo, DexAdapter, PriceQuote, SwapRequest, SwapResult, SwapStatus};
use crate::error::{AppError, AppResult, ChainError, ExecutionError};
use async_trait::async_trait;
use near_jsonrpc_client::{JsonRpcClient, methods};
use rust_decimal::Decimal;
use crate::ledger::models::Chain;


pub struct NearDexAdapter {
    rpc_url: String,
}

impl NearDexAdapter {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }

    fn validate_near_account(&self, address: &str) -> AppResult<()> {
        // NEAR account IDs have specific format: alphanumeric + hyphens, 2-64 chars
        if address.len() < 2 || address.len() > 64 {
            return Err(AppError::ChainAdapter(ChainError::InvalidAddress { chain: Chain::Near, address: address.to_string() }.to_string()));
        }
        if !address
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(AppError::ChainAdapter(ChainError::InvalidAddress { chain: Chain::Near, address: address.to_string() }.to_string()));
        }
        Ok(())
    }

    fn calculate_rate(&self, asset_in: &AssetInfo, asset_out: &AssetInfo) -> Decimal {
        // Simplified: 1:1 rate with decimal scaling
        let in_decimals = Decimal::from(10_u64.pow(asset_in.decimals as u32));
        let out_decimals = Decimal::from(10_u64.pow(asset_out.decimals as u32));
        Decimal::from(1) * (out_decimals / in_decimals)
    }
}

#[async_trait]
impl DexAdapter for NearDexAdapter {
    fn name(&self) -> &'static str {
        "Ref Finance"
    }

    fn supported_chains(&self) -> Vec<Chain> {
        vec![Chain::Near]
    }

    async fn get_price(
        &self,
        asset_in: &AssetInfo,
        asset_out: &AssetInfo,
        amount: Decimal,
    ) -> AppResult<PriceQuote> {
        self.validate_near_account(&asset_in.address)?;
        self.validate_near_account(&asset_out.address)?;

        if asset_in.chain != Chain::Near || asset_out.chain != Chain::Near {
            return Err(AppError::Execution(ExecutionError::UnsupportedChain(
                Chain::Near
            )));
        }

        let rate = self.calculate_rate(asset_in, asset_out);
        let amount_out = amount * rate;
        let slippage_percent = Decimal::from_f64_retain(0.25); // 0.25% slippage

        Ok(PriceQuote {
            asset_in: asset_in.clone(),
            asset_out: asset_out.clone(),
            amount_in: amount,
            amount_out,
            rate,
            dex_name: "Ref Finance".to_string(),
            chain: Chain::Near,
            slippage_percent: slippage_percent.unwrap(),
            execution_time_seconds: 5,
            liquidity_available: Decimal::from(1_000_000), // Large liquidity estimate
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    /// Returns a list of supported NEAR assets.(Demo tokens)
    async fn get_supported_assets(&self, _chain: Chain) -> AppResult<Vec<AssetInfo>> {
        Ok(vec![
            AssetInfo {
                chain: Chain::Near,
                address: "near".to_string(),
                symbol: "NEAR".to_string(),
                name: "NEAR Protocol".to_string(),
                decimals: 24,
                logo_url: Some(
                    "https://assets.coingecko.com/coins/images/10365/standard/near_icon.png"
                        .to_string(),
                ),
            },
            AssetInfo {
                chain: Chain::Near,
                address: "usdc.near".to_string(),
                symbol: "USDC".to_string(),
                name: "USD Coin (wrapped)".to_string(),
                decimals: 6,
                logo_url: Some(
                    "https://assets.coingecko.com/coins/images/6319/standard/usdc.png"
                        .to_string(),
                ),
            },
            AssetInfo {
                chain: Chain::Near,
                address: "usdt.near".to_string(),
                symbol: "USDT".to_string(),
                name: "Tether USD (wrapped)".to_string(),
                decimals: 6,
                logo_url: Some(
                    "https://assets.coingecko.com/coins/images/325/standard/Tether.png"
                        .to_string(),
                ),
            },
            AssetInfo {
                chain: Chain::Near,
                address: "aurora".to_string(),
                symbol: "AURORA".to_string(),
                name: "Aurora".to_string(),
                decimals: 18,
                logo_url: Some(
                    "https://assets.coingecko.com/coins/images/19471/standard/aurora.png"
                        .to_string(),
                ),
            },
        ])
    }

    async fn swap(&self, request: SwapRequest) -> AppResult<SwapResult> {
        self.validate_near_account(&request.asset_in.address)?;
        self.validate_near_account(&request.asset_out.address)?;

        if request.asset_in.chain != Chain::Near || request.asset_out.chain != Chain::Near {
            return Err(AppError::UnsupportedChain(
                "NEAR DEX only supports NEAR assets".to_string(),
            ));
        }

        let rate = self.calculate_rate(&request.asset_in, &request.asset_out);
        let amount_out = request.amount_in * rate;
        let gas_fee = Decimal::from_f64_retain(0.00001); // 0.00001 NEAR

        Ok(SwapResult {
            transaction_hash: format!("NEAR-{}", uuid::Uuid::new_v4()),
            amount_in: request.amount_in,
            amount_out,
            actual_rate: rate,
            gas_fee: Some(gas_fee.unwrap()),
            status: SwapStatus::Pending,
        })
    }

    async fn estimate_gas(&self, _asset_in: &AssetInfo, _asset_out: &AssetInfo) -> AppResult<Decimal> {
        // NEAR gas estimation: fixed 0.00001 NEAR per swap
        Ok(Decimal::from_f64_retain(0.00001).unwrap())
    }

    async fn is_available(&self) -> AppResult<bool> {
        // Basic health check: try to reach RPC
        let client = JsonRpcClient::connect(self.rpc_url.clone());

        let request = methods::status::RpcStatusRequest; 

        // call a method on the server via the connected client
        let server_status = client.call(request).await;

        if server_status.is_ok()  {
            return Ok(true);
        } else {
           return Err(AppError::AdapterNotFound);
        }
         
    }
}
