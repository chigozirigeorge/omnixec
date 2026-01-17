use crate::api::handler::AppState;
use crate::error::AppResult;
use crate::ledger::models::Chain;
use crate::wallet::models::{UserWallet, WalletVerificationRequest, WalletVerificationResponse};
use crate::wallet::verifier::WalletVerifier;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct RegisterWalletRequest {
    /// Optional: user_id. If not provided, a new user is created
    pub user_id: Option<Uuid>,
    pub chain: Chain,
    pub address: String,
}

#[derive(Serialize)]
pub struct RegisterWalletResponse {
    pub wallet_id: Uuid,
    pub user_id: Uuid,
    pub chain: Chain,
    pub address: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct UserWalletsResponse {
    pub user_id: Uuid,
    pub wallets: Vec<WalletInfoResponse>,
}

#[derive(Serialize)]
pub struct WalletInfoResponse {
    pub wallet_id: Uuid,
    pub chain: Chain,
    pub address: String,
    pub status: String,
    pub verified_at: Option<String>,
}

#[derive(Serialize)]
pub struct PortfolioResponse {
    pub user_id: Uuid,
    pub wallets: Vec<ChainPortfolioResponse>,
    pub total_portfolio_usd: Option<String>,
}

#[derive(Serialize)]
pub struct ChainPortfolioResponse {
    pub chain: String,
    pub address: String,
    pub native_balance: String,
    pub native_symbol: String,
    pub tokens: Vec<TokenBalanceResponse>,
    pub chain_total_usd: Option<String>,
}

#[derive(Serialize)]
pub struct TokenBalanceResponse {
    pub symbol: String,
    pub balance: String,
    pub usd_value: Option<String>,
}

pub async fn register_wallet(
    State(state): State<AppState>,
    Json(req): Json<RegisterWalletRequest>,
) -> AppResult<(StatusCode, Json<RegisterWalletResponse>)> {
    // Validate wallet address format
    WalletVerifier::validate_wallet_address(req.chain, &req.address)?;

    // Check if wallet already exists
    let existing = state
        .wallet_repository
        .get_wallet_by_address(req.chain, &req.address)
        .await?;

    if existing.is_some() {
        return Err(crate::error::AppError::InvalidInput(
            "Wallet already registered".to_string(),
        ));
    }

    // Use provided user_id or generate a new one
    let user_id = req.user_id.unwrap_or_else(Uuid::new_v4);

    // Create new wallet
    let wallet = UserWallet::new(user_id, req.chain, req.address.clone());
    let wallet = state.wallet_repository.register_wallet(wallet).await?;

    let response = RegisterWalletResponse {
        wallet_id: wallet.id,
        user_id: wallet.user_id,
        chain: wallet.chain,
        address: wallet.address,
        status: format!("{:?}", wallet.status).to_lowercase(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn verify_wallet(
    State(state): State<AppState>,
    Json(req): Json<WalletVerificationRequest>,
) -> AppResult<(StatusCode, Json<WalletVerificationResponse>)> {
    // Validate address format
    WalletVerifier::validate_wallet_address(req.chain, &req.address)?;

    // Verify signature
    let _verified = WalletVerifier::verify_signature(&req)?;

    // Get wallet
    let wallet = state
        .wallet_repository
        .get_wallet_by_address(req.chain, &req.address)
        .await?
        .ok_or(crate::error::AppError::NotFound(
            "Wallet not found".to_string(),
        ))?;

    // Mark as verified
    let verified_wallet = state
        .wallet_repository
        .verify_wallet(wallet.id)
        .await?;

    let response = WalletVerificationResponse {
        wallet_id: verified_wallet.id,
        chain: verified_wallet.chain,
        address: verified_wallet.address.clone(),
        verified: verified_wallet.is_verified(),
        verified_at: verified_wallet.verified_at,
    };

    Ok((StatusCode::OK, Json(response)))
}

pub async fn get_user_wallets(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<UserWalletsResponse>)> {
    let wallets = state
        .wallet_repository
        .get_user_wallets(user_id)
        .await?;

    let wallet_responses: Vec<_> = wallets
        .iter()
        .map(|w| WalletInfoResponse {
            wallet_id: w.id,
            chain: w.chain,
            address: w.address.clone(),
            status: format!("{:?}", w.status).to_lowercase(),
            verified_at: w.verified_at.map(|t| t.to_rfc3339()),
        })
        .collect();

    let response = UserWalletsResponse {
        user_id,
        wallets: wallet_responses,
    };

    Ok((StatusCode::OK, Json(response)))
}

pub async fn get_user_portfolio(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<PortfolioResponse>)> {
    let chain_balances = state
        .wallet_repository
        .get_user_all_balances(user_id)
        .await?;

    let mut total_usd = Decimal::ZERO;

    let portfolios: Vec<_> = chain_balances
        .iter()
        .map(|chain_bal| {
            if let Some(usd) = chain_bal.total_usd {
                total_usd += usd;
            }

            ChainPortfolioResponse {
                chain: chain_bal.chain.to_string(),
                address: String::new(), // Would need wallet address lookup
                native_balance: chain_bal.native_balance.to_string(),
                native_symbol: chain_bal.native_symbol.clone(),
                tokens: chain_bal
                    .tokens
                    .iter()
                    .map(|t| TokenBalanceResponse {
                        symbol: t.token_symbol.clone(),
                        balance: t.balance.to_string(),
                        usd_value: t.balance_usd.map(|v| v.to_string()),
                    })
                    .collect(),
                chain_total_usd: chain_bal.total_usd.map(|v| v.to_string()),
            }
        })
        .collect();

    let response = PortfolioResponse {
        user_id,
        wallets: portfolios,
        total_portfolio_usd: if total_usd > Decimal::ZERO {
            Some(total_usd.to_string())
        } else {
            None
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

pub async fn get_wallet_balance(
    State(state): State<AppState>,
    Path((user_id, chain_str)): Path<(Uuid, String)>,
) -> AppResult<(StatusCode, Json<ChainPortfolioResponse>)> {
    let chain = match chain_str.to_lowercase().as_str() {
        "solana" | "sol" => Chain::Solana,
        "stellar" | "xlm" => Chain::Stellar,
        "near" => Chain::Near,
        _ => {
            return Err(crate::error::AppError::InvalidInput(
                format!("Unknown chain: {}", chain_str),
            ))
        }
    };

    let chain_balance = state
        .wallet_repository
        .get_user_balance_by_chain(user_id, chain)
        .await?;

    let response = ChainPortfolioResponse {
        chain: chain_balance.chain.to_string(),
        address: String::new(),
        native_balance: chain_balance.native_balance.to_string(),
        native_symbol: chain_balance.native_symbol,
        tokens: chain_balance
            .tokens
            .iter()
            .map(|t| TokenBalanceResponse {
                symbol: t.token_symbol.clone(),
                balance: t.balance.to_string(),
                usd_value: t.balance_usd.map(|v| v.to_string()),
            })
            .collect(),
        chain_total_usd: chain_balance.total_usd.map(|v| v.to_string()),
    };

    Ok((StatusCode::OK, Json(response)))
}
