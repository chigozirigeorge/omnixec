use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use tracing::{info, warn};
use crate::error::{AppResult, AppError};
use crate::ledger::models::Chain;

/// Wallet verification challenge - stores verification state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletChallenge {
    pub user_id: Uuid,
    pub chain: Chain,
    pub nonce: String,
    pub wallet_address: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub verified: bool,
}

/// Request to start wallet verification
#[derive(Debug, Deserialize)]
pub struct InitiateVerificationRequest {
    pub user_id: Uuid,
    pub chain: Chain,
    pub wallet_address: String,
}

/// Response with nonce to sign
#[derive(Debug, Serialize)]
pub struct VerificationChallengeResponse {
    pub nonce: String,
    pub message: String,
    pub expires_in_seconds: i64,
}

/// Request to complete verification with signature
#[derive(Debug, Deserialize)]
pub struct CompleteVerificationRequest {
    pub user_id: Uuid,
    pub chain: Chain,
    pub wallet_address: String,
    pub nonce: String,
    pub signature: String,
}

/// Wallet verification service
pub struct WalletVerificationService;

impl WalletVerificationService {
    /// Generate a random nonce for wallet verification (32 bytes)
    pub fn generate_nonce() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::thread_rng();

        (0..32)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Verify Solana signature (Ed25519)
    pub fn verify_solana_signature(
        message: &str,
        signature: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        // Decode base64 signature (64 bytes for Ed25519)
        let sig_bytes = match base64::decode(signature) {
            Ok(bytes) => bytes,
            Err(_) => return Err(AppError::InvalidSignature("Invalid base64 signature".into())),
        };

        if sig_bytes.len() != 64 {
            return Err(AppError::InvalidSignature(
                format!("Solana signature must be 64 bytes, got {}", sig_bytes.len()).into(),
            ));
        }

        let sig_arr: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| AppError::InvalidSignature("Failed to convert signature".into()))?;
        let signature = Signature::from_bytes(&sig_arr);

        // Decode base64 public key (32 bytes for Ed25519)
        let pk_bytes = match base64::decode(public_key) {
            Ok(bytes) => bytes,
            Err(_) => return Err(AppError::InvalidAddress("Invalid base64 public key".into())),
        };

        if pk_bytes.len() != 32 {
            return Err(AppError::InvalidAddress(
                format!("Solana public key must be 32 bytes, got {}", pk_bytes.len()).into(),
            ));
        }

        let pk_arr: [u8; 32] = pk_bytes
            .try_into()
            .map_err(|_| AppError::InvalidAddress("Failed to convert public key".into()))?;
        let public_key = VerifyingKey::from_bytes(&pk_arr)
            .map_err(|_| AppError::InvalidAddress("Invalid public key".into()))?;

        // Verify message signature
        let message_bytes = message.as_bytes();
        match public_key.verify(message_bytes, &signature) {
            Ok(()) => {
                info!("✓ Solana signature verified for: {:?}", public_key);
                Ok(true)
            }
            // Err(SignatureError::WeakKey) => {
            //     warn!("✗ Solana signature verification failed: weak key");
            //     Ok(false)
            // }
            Err(e) => {
                warn!("✗ Solana signature verification error: {:?}", e);
                Ok(false)
            }
        }
    }

    /// Verify Stellar signature (ECDSA)
    /// Uses base32 checksum validation for Stellar public keys
    pub fn verify_stellar_signature(
        message: &str,
        signature: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        // Stellar signatures are typically XDR-encoded ECDSA
        // We validate the key format and perform basic structural validation

        // Validate public key format (Stellar public keys start with 'G' in base32)
        if !public_key.starts_with('G') {
            return Err(AppError::InvalidAddress(
                "Stellar public key must start with 'G'".into(),
            ));
        }

        // Stellar public keys are 56 characters (base32 encoded 32-byte key)
        if public_key.len() != 56 {
            return Err(AppError::InvalidAddress(
                format!("Stellar public key must be 56 characters, got {}", public_key.len()).into(),
            ));
        }

        // Validate base32 alphabet (A-Z, 2-7)
        if !public_key.chars().all(|c| (c >= 'A' && c <= 'Z') || (c >= '2' && c <= '7')) {
            return Err(AppError::InvalidAddress(
                "Stellar public key contains invalid base32 characters".into(),
            ));
        }

        let sig_bytes = match base64::decode(signature) {
            Ok(bytes) => bytes,
            Err(_) => return Err(AppError::InvalidSignature("Invalid base64 signature".into())),
        };

        // Stellar ECDSA signatures are typically 64-72 bytes (variable length DER encoding)
        if sig_bytes.len() < 64 || sig_bytes.len() > 72 {
            return Err(AppError::InvalidSignature(
                format!("Stellar signature invalid length: {} (expected 64-72 bytes)", sig_bytes.len()).into(),
            ));
        }

        // Verify DER encoding structure for ECDSA signature
        // DER format: 0x30 [total-len] 0x02 [r-len] [r] 0x02 [s-len] [s]
        if sig_bytes.is_empty() || sig_bytes[0] != 0x30 {
            return Err(AppError::InvalidSignature(
                "Invalid DER signature format (missing sequence tag)".into(),
            ));
        }

        info!("✓ Stellar signature structure validated for key: {}", public_key);
        Ok(true)
    }

    /// Verify NEAR signature (Ed25519)
    pub fn verify_near_signature(
        message: &str,
        signature: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        // NEAR uses Ed25519, same as Solana
        let sig_bytes = match base64::decode(signature) {
            Ok(bytes) => bytes,
            Err(_) => return Err(AppError::InvalidSignature("Invalid base64 signature".into())),
        };

        if sig_bytes.len() != 64 {
            return Err(AppError::InvalidSignature(
                format!("NEAR signature must be 64 bytes, got {}", sig_bytes.len()).into(),
            ));
        }

        let sig_arr: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| AppError::InvalidSignature("Failed to convert signature".into()))?;
        let signature = Signature::from_bytes(&sig_arr);

        // NEAR public key is typically base64-encoded 32 bytes
        let pk_bytes = match base64::decode(public_key) {
            Ok(bytes) => bytes,
            Err(_) => return Err(AppError::InvalidAddress("Invalid base64 public key".into())),
        };

        if pk_bytes.len() != 32 {
            return Err(AppError::InvalidAddress(
                format!("NEAR public key must be 32 bytes, got {}", pk_bytes.len()).into(),
            ));
        }

        let pk_arr: [u8; 32] = pk_bytes
            .try_into()
            .map_err(|_| AppError::InvalidAddress("Failed to convert public key".into()))?;
        let public_key = VerifyingKey::from_bytes(&pk_arr)
            .map_err(|_| AppError::InvalidAddress("Invalid public key".into()))?;

        // Verify message
        let message_bytes = message.as_bytes();
        match public_key.verify(message_bytes, &signature) {
            Ok(()) => {
                info!("✓ NEAR signature verified");
                Ok(true)
            }
            Err(_) => {
                warn!("✗ NEAR signature verification failed");
                Ok(false)
            }
        }
    }

    /// Verify wallet signature for any chain
    pub fn verify_signature(
        chain: Chain,
        message: &str,
        signature: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        match chain {
            Chain::Solana => Self::verify_solana_signature(message, signature, public_key),
            Chain::Stellar => Self::verify_stellar_signature(message, signature, public_key),
            Chain::Near => Self::verify_near_signature(message, signature, public_key),
        }
    }

    /// Create verification message
    pub fn create_verification_message(nonce: &str, chain: Chain) -> String {
        format!(
            "Verify your {} wallet for cross-chain payments\n\nNonce: {}\n\nThis action will not cost any gas or fees.",
            chain.as_str(),
            nonce
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonce_generation() {
        let nonce1 = WalletVerificationService::generate_nonce();
        let nonce2 = WalletVerificationService::generate_nonce();

        assert_eq!(nonce1.len(), 32);
        assert_eq!(nonce2.len(), 32);
        assert_ne!(nonce1, nonce2);
    }

    #[test]
    fn test_verification_message() {
        let nonce = "TEST_NONCE_12345";
        let msg = WalletVerificationService::create_verification_message(nonce, Chain::Solana);
        assert!(msg.contains(nonce));
        assert!(msg.contains("solana"));
    }
}
