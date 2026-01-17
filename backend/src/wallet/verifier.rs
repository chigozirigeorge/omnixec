use crate::error::{AppError, AppResult};
use crate::ledger::models::Chain;
use crate::wallet::models::{WalletVerificationRequest};

pub struct WalletVerifier;

impl WalletVerifier {
    pub fn verify_signature(request: &WalletVerificationRequest) -> AppResult<bool> {
        match request.chain {
            Chain::Solana => Self::verify_solana_signature(request),
            Chain::Stellar => Self::verify_stellar_signature(request),
            Chain::Near => Self::verify_near_signature(request),
        }
    }

    fn verify_solana_signature(request: &WalletVerificationRequest) -> AppResult<bool> {
        // Validate Solana address format (base58)
        Self::validate_solana_address(&request.address)?;

        // TODO: use solana_sdk to verify signature
        // For now, basic validation
        if request.signature.is_empty() || request.message.is_empty() {
            return Err(AppError::InvalidInput(
                "Signature and message cannot be empty".to_string(),
            ));
        }

        // Check signature length (Solana signatures are 128 chars in hex)
        if request.signature.len() != 128 {
            return Err(AppError::InvalidSignature(
                "Invalid Solana signature length".to_string(),
            ));
        }

        Ok(true)
    }

    fn verify_stellar_signature(request: &WalletVerificationRequest) -> AppResult<bool> {
        // Validate Stellar address format (G... prefix)
        if !request.address.starts_with('G') || request.address.len() != 56 {
            return Err(AppError::InvalidAddress(
                "Invalid Stellar address format".to_string(),
            ));
        }

        if request.signature.is_empty() || request.message.is_empty() {
            return Err(AppError::InvalidInput(
                "Signature and message cannot be empty".to_string(),
            ));
        }

        // TODO: use stellar_sdk to verify signature
        Ok(true)
    }

    fn verify_near_signature(request: &WalletVerificationRequest) -> AppResult<bool> {
        // Validate NEAR account format
        if request.address.len() < 2 || request.address.len() > 64 {
            return Err(AppError::InvalidAddress(
                "Invalid NEAR account ID length".to_string(),
            ));
        }

        if !request
            .address
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(AppError::InvalidAddress(
                "Invalid NEAR account ID format".to_string(),
            ));
        }

        if request.signature.is_empty() || request.message.is_empty() {
            return Err(AppError::InvalidInput(
                "Signature and message cannot be empty".to_string(),
            ));
        }

        // TODO: use near_crypto to verify signature
        Ok(true)
    }

    fn validate_solana_address(address: &str) -> AppResult<()> {
        // Basic Solana address validation (base58, ~44 chars)
        if address.len() < 32 || address.len() > 44 {
            return Err(AppError::InvalidAddress(
                "Invalid Solana address length".to_string(),
            ));
        }

        // Check for valid base58 characters
        if !address.chars().all(|c| {
            "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(c)
        }) {
            return Err(AppError::InvalidAddress(
                "Invalid Solana address: contains invalid characters".to_string(),
            ));
        }

        Ok(())
    }

    pub fn validate_wallet_address(chain: Chain, address: &str) -> AppResult<()> {
        match chain {
            Chain::Solana => Self::validate_solana_address(address),
            Chain::Stellar => {
                if !address.starts_with('G') || address.len() != 56 {
                    return Err(AppError::InvalidAddress(
                        "Invalid Stellar address format".to_string(),
                    ));
                }
                Ok(())
            }
            Chain::Near => {
                if address.len() < 2 || address.len() > 64 {
                    return Err(AppError::InvalidAddress(
                        "Invalid NEAR account ID length".to_string(),
                    ));
                }
                if !address.chars().all(|c| {
                    c.is_alphanumeric() || c == '-' || c == '_' || c == '.'
                }) {
                    return Err(AppError::InvalidAddress(
                        "Invalid NEAR account ID format".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }
}
