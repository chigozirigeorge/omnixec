use crate::adapters::traits::AssetInfo;
use crate::api::handler::AppState;
use crate::error::{AppError, AppResult};
use crate::ledger::models::Chain;
use crate::trading::models::{Trade};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct InitiateTradeRequest {
    pub user_id: Uuid,
    pub source_wallet_id: Uuid,
    pub destination_wallet_id: Uuid,
    pub source_chain: Chain,
    pub destination_chain: Chain,
    pub asset_in_address: String,
    pub asset_out_address: String,
    pub amount_in: Decimal,
}

#[derive(Serialize)]
pub struct InitiateTradeResponse {
    pub quote_id: String,
    pub trade_id: Uuid,
    pub source_chain: Chain,
    pub destination_chain: Chain,
    pub amount_in: Decimal,
    pub amount_out_expected: Decimal,
    pub dex: String,
    pub total_gas_estimate: Decimal,
    pub slippage_percent: Decimal,
    pub expires_in_seconds: i64,
    pub execution_price: String,
}

#[derive(Deserialize)]
pub struct ExecuteTradeRequest {
    pub trade_id: Uuid,
    pub user_id: Uuid,
    pub accept_slippage_percent: Decimal,
}

#[derive(Serialize)]
pub struct ExecuteTradeResponse {
    pub trade_id: Uuid,
    pub status: String,
    pub swap_transaction: String,
    pub estimated_completion_seconds: u64,
}

#[derive(Serialize)]
pub struct TradeStatusResponse {
    pub trade_id: Uuid,
    pub status: String,
    pub source_chain: Chain,
    pub destination_chain: Chain,
    pub amount_in: Decimal,
    pub amount_out_expected: Decimal,
    pub amount_out_actual: Option<Decimal>,
    pub source_tx: Option<String>,
    pub swap_tx: Option<String>,
    pub destination_tx: Option<String>,
    pub slippage_actual: Option<Decimal>,
    pub gas_fees_paid: Option<Decimal>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Serialize)]
pub struct UserTradesResponse {
    pub user_id: Uuid,
    pub trades: Vec<TradeStatusResponse>,
    pub total_count: usize,
    pub completed_count: usize,
    pub pending_count: usize,
}

pub async fn initiate_trade(
    State(state): State<AppState>,
    Json(req): Json<InitiateTradeRequest>,
) -> AppResult<(StatusCode, Json<InitiateTradeResponse>)> {
    // Validate wallets
    let source_wallet = state
        .wallet_repository
        .get_wallet(req.source_wallet_id)
        .await?;

    let destination_wallet = state
        .wallet_repository
        .get_wallet(req.destination_wallet_id)
        .await?;

    // Check wallet verification
    if !source_wallet.is_verified() {
        return Err(AppError::InvalidInput(
            "Source wallet must be verified".to_string(),
        ));
    }

    if !destination_wallet.is_verified() {
        return Err(AppError::InvalidInput(
            "Destination wallet must be verified".to_string(),
        ));
    }

    // Create asset info
    let asset_in = AssetInfo {
        chain: req.source_chain,
        address: req.asset_in_address,
        symbol: String::new(),
        name: String::new(),
        decimals: 18,
        logo_url: None,
    };

    let asset_out = AssetInfo {
        chain: req.destination_chain,
        address: req.asset_out_address,
        symbol: String::new(),
        name: String::new(),
        decimals: 18,
        logo_url: None,
    };

    // Get best quote
    let quote = state
        .realtime_quote_engine
        .get_best_quote(&asset_in, &asset_out, req.amount_in)
        .await?;

    let quote_id = Uuid::new_v4().to_string();
    let trade = Trade::new(
        req.user_id,
        req.source_wallet_id,
        req.destination_wallet_id,
        req.source_chain,
        req.destination_chain,
        asset_in,
        asset_out,
        req.amount_in,
        quote.best_amount_out,
        quote.best_dex.clone(),
        quote_id.clone(),
    );

    let trade = state.trade_repository.create_trade(trade).await?;

    let response = InitiateTradeResponse {
        quote_id,
        trade_id: trade.id,
        source_chain: trade.source_chain,
        destination_chain: trade.destination_chain,
        amount_in: trade.amount_in,
        amount_out_expected: trade.amount_out_expected,
        dex: trade.dex_used,
        total_gas_estimate: Decimal::from_str("0.001").unwrap_or_default(),
        slippage_percent: quote.best_slippage,
        expires_in_seconds: 60,
        execution_price: quote.best_rate.to_string(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn execute_trade(
    State(state): State<AppState>,
    Json(req): Json<ExecuteTradeRequest>,
) -> AppResult<(StatusCode, Json<ExecuteTradeResponse>)> {
    // Verify user ownership
    let mut trade = state.trade_repository.get_trade(req.trade_id).await?;

    if trade.user_id != req.user_id {
        return Err(AppError::Unauthorized);
    }

    // Check wallet still verified
    let source_wallet = state
        .wallet_repository
        .get_wallet(trade.source_wallet_id)
        .await?;

    if !source_wallet.is_verified() {
        return Err(AppError::InvalidInput(
            "Source wallet verification lost".to_string(),
        ));
    }

    // Validate slippage tolerance
    if req.accept_slippage_percent < trade.slippage_actual.unwrap_or_default() {
        return Err(AppError::InvalidInput(
            "Slippage tolerance too low".to_string(),
        ));
    }

    // Mark quote as accepted
    trade.status = crate::trading::models::TradeStatus::QuoteAccepted;
    let trade = state.trade_repository.update_trade(trade).await?;

    // Simulate swap execution (in production, would call actual executor)
    let swap_tx = format!("0x{}", uuid::Uuid::new_v4().simple());

    let trade = state
        .trade_repository
        .mark_executing(trade.id, swap_tx.clone())
        .await?;

    let response = ExecuteTradeResponse {
        trade_id: trade.id,
        status: format!("{:?}", trade.status).to_lowercase(),
        swap_transaction: swap_tx,
        estimated_completion_seconds: 30,
    };

    Ok((StatusCode::ACCEPTED, Json(response)))
}

pub async fn get_trade_status(
    State(state): State<AppState>,
    Path(trade_id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<TradeStatusResponse>)> {
    let trade = state.trade_repository.get_trade(trade_id).await?;

    let response = TradeStatusResponse {
        trade_id: trade.id,
        status: format!("{:?}", trade.status).to_lowercase(),
        source_chain: trade.source_chain,
        destination_chain: trade.destination_chain,
        amount_in: trade.amount_in,
        amount_out_expected: trade.amount_out_expected,
        amount_out_actual: trade.amount_out_actual,
        source_tx: trade.source_tx_hash,
        swap_tx: trade.swap_tx_hash,
        destination_tx: trade.destination_tx_hash,
        slippage_actual: trade.slippage_actual,
        gas_fees_paid: trade.gas_fees_paid,
        created_at: trade.created_at.to_rfc3339(),
        completed_at: trade.completed_at.map(|t| t.to_rfc3339()),
    };

    Ok((StatusCode::OK, Json(response)))
}

pub async fn get_user_trades(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<UserTradesResponse>)> {
    let trades = state.trade_repository.get_user_trades(user_id).await?;

    let completed_count = trades
        .iter()
        .filter(|t| t.status == crate::trading::models::TradeStatus::Completed)
        .count();

    let pending_count = trades
        .iter()
        .filter(|t| {
            matches!(
                t.status,
                crate::trading::models::TradeStatus::Pending
                    | crate::trading::models::TradeStatus::ExecutingSwap
                    | crate::trading::models::TradeStatus::SettlementInProgress
            )
        })
        .count();

    let trade_responses: Vec<_> = trades
        .iter()
        .map(|t| TradeStatusResponse {
            trade_id: t.id,
            status: format!("{:?}", t.status).to_lowercase(),
            source_chain: t.source_chain,
            destination_chain: t.destination_chain,
            amount_in: t.amount_in,
            amount_out_expected: t.amount_out_expected,
            amount_out_actual: t.amount_out_actual,
            source_tx: t.source_tx_hash.clone(),
            swap_tx: t.swap_tx_hash.clone(),
            destination_tx: t.destination_tx_hash.clone(),
            slippage_actual: t.slippage_actual,
            gas_fees_paid: t.gas_fees_paid,
            created_at: t.created_at.to_rfc3339(),
            completed_at: t.completed_at.map(|t| t.to_rfc3339()),
        })
        .collect();

    let response = UserTradesResponse {
        user_id,
        total_count: trades.len(),
        completed_count,
        pending_count,
        trades: trade_responses,
    };

    Ok((StatusCode::OK, Json(response)))
}

pub async fn get_user_trades_by_chain(
    State(state): State<AppState>,
    Path((user_id, chain_str)): Path<(Uuid, String)>,
) -> AppResult<(StatusCode, Json<UserTradesResponse>)> {
    let chain = match chain_str.to_lowercase().as_str() {
        "solana" | "sol" => Chain::Solana,
        "stellar" | "xlm" => Chain::Stellar,
        "near" => Chain::Near,
        _ => {
            return Err(AppError::InvalidInput(format!("Unknown chain: {}", chain_str)))
        }
    };

    let trades = state
        .trade_repository
        .get_user_trades_by_chain(user_id, chain)
        .await?;

    let completed_count = trades
        .iter()
        .filter(|t| t.status == crate::trading::models::TradeStatus::Completed)
        .count();

    let pending_count = trades
        .iter()
        .filter(|t| {
            matches!(
                t.status,
                crate::trading::models::TradeStatus::Pending
                    | crate::trading::models::TradeStatus::ExecutingSwap
                    | crate::trading::models::TradeStatus::SettlementInProgress
            )
        })
        .count();

    let trade_responses: Vec<_> = trades
        .iter()
        .map(|t| TradeStatusResponse {
            trade_id: t.id,
            status: format!("{:?}", t.status).to_lowercase(),
            source_chain: t.source_chain,
            destination_chain: t.destination_chain,
            amount_in: t.amount_in,
            amount_out_expected: t.amount_out_expected,
            amount_out_actual: t.amount_out_actual,
            source_tx: t.source_tx_hash.clone(),
            swap_tx: t.swap_tx_hash.clone(),
            destination_tx: t.destination_tx_hash.clone(),
            slippage_actual: t.slippage_actual,
            gas_fees_paid: t.gas_fees_paid,
            created_at: t.created_at.to_rfc3339(),
            completed_at: t.completed_at.map(|t| t.to_rfc3339()),
        })
        .collect();

    let response = UserTradesResponse {
        user_id,
        total_count: trades.len(),
        completed_count,
        pending_count,
        trades: trade_responses,
    };

    Ok((StatusCode::OK, Json(response)))
}
