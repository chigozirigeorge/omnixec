use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use crate::adapters::traits::AssetInfo;
use crate::api::handler::AppState;
use crate::error::AppResult;
use crate::ledger::models::Chain;

#[derive(Serialize)]
pub struct DexInfo {
    pub name: String,
    pub chain: Chain,
    pub fee_tier: String,
    pub available: bool,
}

#[derive(Serialize)]
pub struct ChainDiscovery {
    pub chain: Chain,
    pub dexes: Vec<DexInfo>,
    pub supported_tokens: Vec<AssetInfo>,
}

#[derive(Deserialize)]
pub struct PriceQueryRequest {
    pub asset_in_address: String,
    pub asset_out_address: String,
    pub amount: String,
    pub chain: Chain,
}

#[derive(Serialize)]
pub struct AssetListResponse {
    pub chain: Chain,
    pub assets: Vec<AssetInfo>,
    pub total_count: usize,
}

pub async fn list_dexes_for_chain(
    State(state): State<AppState>,
    Path(chain): Path<Chain>,
) -> AppResult<Json<Vec<DexInfo>>> {
    let dex_names = state.adapter_registry.list_dexes_for_chain(chain).await;
    
    let dexes = dex_names
        .into_iter()
        .map(|name| DexInfo {
            name: name.clone(),
            chain,
            fee_tier: "0.25%".to_string(),
            available: true,
        })
        .collect();
    
    Ok(Json(dexes))
}

pub async fn get_chain_discovery(
    State(state): State<AppState>,
    Path(chain): Path<Chain>,
) -> AppResult<Json<ChainDiscovery>> {
    let dex_adapters = state.adapter_registry.get_all_dexes_for_chain(chain).await;
    
    let mut all_tokens = Vec::new();
    for adapter in &dex_adapters {
        if let Ok(tokens) = adapter.get_supported_assets(chain).await {
            all_tokens.extend(tokens);
        }
    }
    
    all_tokens.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    all_tokens.dedup_by(|a, b| a.address == b.address);
    
    let mut dexes = Vec::new();
    for adapter in dex_adapters {
        let available = adapter.is_available().await.unwrap_or(false);
        dexes.push(DexInfo {
            name: adapter.name().to_string(),
            chain,
            fee_tier: "0.25%".to_string(),
            available,
        });
    }
    
    Ok(Json(ChainDiscovery {
        chain,
        dexes,
        supported_tokens: all_tokens,
    }))
}

pub async fn list_assets_on_dex(
    State(state): State<AppState>,
    Path((dex_name, chain)): Path<(String, Chain)>,
) -> AppResult<Json<AssetListResponse>> {
    let adapter = state
        .adapter_registry
        .get_dex(&dex_name)
        .ok_or_else(|| {
            crate::error::ExecutionError::ChainExecutionFailed {
                chain,
                message: format!("DEX {} not found", dex_name),
            }
        })?;
    
    let assets = adapter.get_supported_assets(chain).await?;
    let total_count = assets.len();
    
    Ok(Json(AssetListResponse {
        chain,
        assets,
        total_count,
    }))
}

pub async fn get_all_chains() -> AppResult<Json<Vec<Chain>>> {
    Ok(Json(vec![Chain::Solana, Chain::Stellar, Chain::Near]))
}
