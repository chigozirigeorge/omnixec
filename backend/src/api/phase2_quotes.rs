use crate::adapters::traits::AssetInfo;
use crate::api::handler::AppState;
use crate::error::AppResult;
use crate::ledger::models::Chain;

use axum::{
    extract::{State},
    http::StatusCode,
    Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct BestQuoteResponse {
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub amount_in: Decimal,
    pub best_dex: String,
    pub best_amount_out: Decimal,
    pub best_rate: Decimal,
    pub best_slippage: Decimal,
    pub dex_options: Vec<DexOption>,
    pub aggregated_liquidity: Decimal,
    pub timestamp: String,
}

#[derive(Serialize)]
pub struct DexOption {
    pub dex_name: String,
    pub amount_out: Decimal,
    pub rate: Decimal,
    pub slippage_percent: Decimal,
}

#[derive(Deserialize)]
pub struct QuoteRequest {
    pub asset_in_chain: Chain,
    pub asset_in_address: String,
    pub asset_out_chain: Chain,
    pub asset_out_address: String,
    pub amount: Decimal,
}

#[derive(Serialize)]
pub struct PriceImpactResponse {
    pub spot_price: Decimal,
    pub execution_price: Decimal,
    pub price_impact_bps: Decimal,
    pub slippage_percent: Decimal,
    pub recommended_slippage_tolerance: Decimal,
}

#[derive(Serialize)]
pub struct RoutesResponse {
    pub best_route: RouteResponse,
    pub alternative_routes: Vec<RouteResponse>,
}

#[derive(Serialize)]
pub struct RouteResponse {
    pub hops: usize,
    pub path: Vec<PathStep>,
    pub total_amount_out: Decimal,
    pub estimated_gas_fees: Decimal,
    pub total_slippage: Decimal,
}

#[derive(Serialize)]
pub struct PathStep {
    pub asset: AssetInfo,
    pub dex: String,
}

pub async fn get_best_quote(
    State(state): State<AppState>,
    Json(req): Json<QuoteRequest>,
) -> AppResult<(StatusCode, Json<BestQuoteResponse>)> {
    let asset_in = AssetInfo {
        chain: req.asset_in_chain,
        address: req.asset_in_address,
        symbol: String::new(),
        name: String::new(),
        decimals: 18,
        logo_url: None,
    };

    let asset_out = AssetInfo {
        chain: req.asset_out_chain,
        address: req.asset_out_address,
        symbol: String::new(),
        name: String::new(),
        decimals: 18,
        logo_url: None,
    };

    let quote = state
        .realtime_quote_engine
        .get_best_quote(&asset_in, &asset_out, req.amount)
        .await?;

    let response = BestQuoteResponse {
        asset_in: quote.asset_in,
        asset_out: quote.asset_out,
        amount_in: quote.amount_in,
        best_dex: quote.best_dex,
        best_amount_out: quote.best_amount_out,
        best_rate: quote.best_rate,
        best_slippage: quote.best_slippage,
        dex_options: quote
            .all_quotes
            .iter()
            .map(|q| DexOption {
                dex_name: q.dex_name.clone(),
                amount_out: q.amount_out,
                rate: q.rate,
                slippage_percent: q.slippage_percent,
            })
            .collect(),
        aggregated_liquidity: quote.aggregated_liquidity,
        timestamp: quote.timestamp.to_rfc3339(),
    };

    Ok((StatusCode::OK, Json(response)))
}

pub async fn get_price_impact(
    State(state): State<AppState>,
    Json(req): Json<QuoteRequest>,
) -> AppResult<(StatusCode, Json<PriceImpactResponse>)> {
    let asset_in = AssetInfo {
        chain: req.asset_in_chain,
        address: req.asset_in_address,
        symbol: String::new(),
        name: String::new(),
        decimals: 18,
        logo_url: None,
    };

    let asset_out = AssetInfo {
        chain: req.asset_out_chain,
        address: req.asset_out_address,
        symbol: String::new(),
        name: String::new(),
        decimals: 18,
        logo_url: None,
    };

    let impact = state
        .realtime_quote_engine
        .estimate_execution_price_impact(&asset_in, &asset_out, req.amount)
        .await?;

    let response = PriceImpactResponse {
        spot_price: impact.spot_price,
        execution_price: impact.execution_price,
        price_impact_bps: impact.price_impact_bps,
        slippage_percent: impact.slippage_percent,
        recommended_slippage_tolerance: impact.recommended_slippage_tolerance,
    };

    Ok((StatusCode::OK, Json(response)))
}

pub async fn find_routes(
    State(state): State<AppState>,
    Json(req): Json<QuoteRequest>,
) -> AppResult<(StatusCode, Json<RoutesResponse>)> {
    let asset_in = AssetInfo {
        chain: req.asset_in_chain,
        address: req.asset_in_address,
        symbol: String::new(),
        name: String::new(),
        decimals: 18,
        logo_url: None,
    };

    let asset_out = AssetInfo {
        chain: req.asset_out_chain,
        address: req.asset_out_address,
        symbol: String::new(),
        name: String::new(),
        decimals: 18,
        logo_url: None,
    };

    let routes = state
        .realtime_quote_engine
        .find_multi_hop_routes(&asset_in, &asset_out, req.amount, 3)
        .await?;

    if routes.is_empty() {
        return Err(crate::error::AppError::NoLiquidityAvailable(
            "No viable routes found".to_string(),
        ));
    }

    let best_route = &routes[0];
    let best_route_response = RouteResponse {
        hops: best_route.hops,
        path: best_route
            .path
            .iter()
            .zip(best_route.dex_sequence.iter())
            .map(|(asset, dex)| PathStep {
                asset: asset.clone(),
                dex: dex.clone(),
            })
            .collect(),
        total_amount_out: best_route.total_amount_out,
        estimated_gas_fees: best_route.estimated_gas_fees,
        total_slippage: best_route.total_slippage,
    };

    let alternative_routes = routes
        .iter()
        .skip(1)
        .map(|route| RouteResponse {
            hops: route.hops,
            path: route
                .path
                .iter()
                .zip(route.dex_sequence.iter())
                .map(|(asset, dex)| PathStep {
                    asset: asset.clone(),
                    dex: dex.clone(),
                })
                .collect(),
            total_amount_out: route.total_amount_out,
            estimated_gas_fees: route.estimated_gas_fees,
            total_slippage: route.total_slippage,
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(RoutesResponse {
            best_route: best_route_response,
            alternative_routes,
        }),
    ))
}
