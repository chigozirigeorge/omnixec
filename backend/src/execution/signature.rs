/// Cryptographic signature verification trait for cross-chain support
/// Enables verification of user signatures across Solana, Stellar, and NEAR

use crate::error::{AppError, AppResult};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use async_trait::async_trait;
use ed25519_dalek::Signature;

/// Trait for cryptographic signature verification across all chains
#[async_trait]
pub trait SignatureVerifier: Send + Sync {
    /// Verify a signature for a given message and public key
    /// 
    /// # Arguments
    /// * `signature` - Base64 encoded signature
    /// * `message` - Original message that was signed
    /// * `public_key` - User's public key on the chain
    ///
    /// # Returns
    /// * `Ok(true)` if signature is valid
    /// * `Ok(false)` if signature is invalid
    /// * `Err(AppError)` if verification fails
    async fn verify_signature(
        &self,
        signature: &str,
        message: &str,
        public_key: &str,
    ) -> AppResult<bool>;
}

/// Solana signature verifier implementation
pub struct SolanaSignatureVerifier;

#[async_trait]
impl SignatureVerifier for SolanaSignatureVerifier {
    async fn verify_signature(
        &self,
        signature: &str,
        message: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        // use solana_sdk::signature::Signature;
        use solana_sdk::pubkey::Pubkey;
        use ed25519_dalek::{VerifyingKey, Signature};
        use std::str::FromStr;

        // Decode base64 signature
        let signature_bytes = BASE64
            .decode(signature)
            .map_err(|_| AppError::InvalidSignature("Invalid base64 encoding".to_string()))?;

        if signature_bytes.len() != 64 {
            return Err(AppError::InvalidSignature(
                "Signature must be 64 bytes".to_string(),
            ));
        }

        // Parse public key
        let pubkey = Pubkey::from_str(public_key)
            .map_err(|_| AppError::InvalidAddress("Invalid Solana public key".to_string()))?;

        let pubkey_bytes: &[u8; 32] = pubkey.as_ref().try_into().unwrap();

        // Verify using ed25519_dalek
        let verify_key = VerifyingKey::from_bytes(pubkey_bytes)
            .map_err(|_| AppError::InvalidSignature("Invalid verification key".to_string()))?;

        let message_bytes = message.as_bytes();
        let sig_array: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| AppError::InvalidSignature("Signature conversion failed".to_string()))?;

        verify_key
            .verify_strict(message_bytes, &Signature::from(&sig_array))
            .map_err(|_| AppError::InvalidSignature("Signature verification failed".to_string()))?;

        Ok(true)
    }
}

/// Stellar signature verifier implementation
pub struct StellarSignatureVerifier;

#[async_trait]
impl SignatureVerifier for StellarSignatureVerifier {
    async fn verify_signature(
        &self,
        signature: &str,
        message: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        use ed25519_dalek::VerifyingKey;

        // Decode base64 signature
        let signature_bytes = BASE64
            .decode(signature)
            .map_err(|_| AppError::InvalidSignature("Invalid base64 encoding".to_string()))?;

        if signature_bytes.len() != 64 {
            return Err(AppError::InvalidSignature(
                "Signature must be 64 bytes".to_string(),
            ));
        }

        // Stellar public keys are 32 bytes (the raw public key)
        // We need to decode the Stellar address format to get the raw key
        let pubkey_bytes = decode_stellar_public_key(public_key)?;

        // Verify
        let verify_key = VerifyingKey::from_bytes(&pubkey_bytes)
            .map_err(|_| AppError::InvalidSignature("Invalid verification key".to_string()))?;

        let message_bytes = message.as_bytes();
        let sig_array: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| AppError::InvalidSignature("Signature conversion failed".to_string()))?;

        verify_key
            .verify_strict(message_bytes, &Signature::from(&sig_array))
            .map_err(|_| AppError::InvalidSignature("Signature verification failed".to_string()))?;

        Ok(true)
    }
}

/// NEAR signature verifier implementation
pub struct NearSignatureVerifier;

#[async_trait]
impl SignatureVerifier for NearSignatureVerifier {
    async fn verify_signature(
        &self,
        signature: &str,
        message: &str,
        public_key: &str,
    ) -> AppResult<bool> {
        use ed25519_dalek::VerifyingKey;

        // Decode base64 signature
        let signature_bytes = BASE64
            .decode(signature)
            .map_err(|_| AppError::InvalidSignature("Invalid base64 encoding".to_string()))?;

        if signature_bytes.len() != 64 {
            return Err(AppError::InvalidSignature(
                "Signature must be 64 bytes".to_string(),
            ));
        }

        // NEAR public keys are 32 bytes ed25519
        let pubkey_bytes = decode_near_public_key(public_key)?;

        // Verify
        let verify_key = VerifyingKey::from_bytes(&pubkey_bytes)
            .map_err(|_| AppError::InvalidSignature("Invalid verification key".to_string()))?;

        let message_bytes = message.as_bytes();
        let sig_array: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| AppError::InvalidSignature("Signature conversion failed".to_string()))?;

        verify_key
            .verify_strict(message_bytes, &Signature::from(&sig_array))
            .map_err(|_| AppError::InvalidSignature("Signature verification failed".to_string()))?;

        Ok(true)
    }
}

/// Helper: Decode Stellar public key from Stroop format to raw 32 bytes
fn decode_stellar_public_key(stroop_key: &str) -> AppResult<[u8; 32]> {
    // Stellar public keys are base32 encoded with prefix 'G' for public keys
    // Format: G + 56 base32 characters = 280 bits ≈ 35 bytes, but checksum is included
    // The actual public key is the first 32 bytes
    
    if !stroop_key.starts_with('G') || stroop_key.len() != 57 {
        return Err(AppError::InvalidAddress(
            "Invalid Stellar public key format (must be 'G' + 56 base32 chars)".to_string(),
        ));
    }

    // Stellar keys can be decoded using crockford base32 decoding
    // TODO: use: stellar_sdk::PublicKey::from_string(stroop_key).public_key()
    // This is a simplified decoder - converts base32 to bytes manually
    
    let key_part = &stroop_key[1..]; // Remove 'G' prefix
    let mut result = Vec::new();
    
    // Simple base32 decoder for Crockford alphabet
    let decode_char = |c: char| -> Result<u8, AppError> {
        match c {
            '0'..='9' => Ok((c as u8) - b'0'),
            'A'..='Z' => Ok((c as u8) - b'A' + 10),
            'a'..='z' => Ok((c as u8) - b'a' + 10),
            _ => Err(AppError::InvalidAddress("Invalid character in Stellar key".to_string())),
        }
    };

    let mut buffer = 0u32;
    let mut bits = 0;

    for c in key_part.chars() {
        let val = decode_char(c)?;
        buffer = (buffer << 5) | (val as u32);
        bits += 5;

        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    if result.len() < 32 {
        return Err(AppError::InvalidAddress(
            "Decoded Stellar key is too short".to_string(),
        ));
    }

    let mut key_array = [0u8; 32];
    key_array.copy_from_slice(&result[..32]);
    Ok(key_array)
}

/// Helper: Decode NEAR public key from string format to raw 32 bytes
fn decode_near_public_key(near_key: &str) -> AppResult<[u8; 32]> {
    // NEAR public keys are typically in format: ed25519:base64string
    if near_key.starts_with("ed25519:") {
        let key_part = &near_key[8..]; // Skip "ed25519:" prefix
        let decoded = BASE64
            .decode(key_part)
            .map_err(|_| AppError::InvalidAddress("Invalid NEAR public key encoding".to_string()))?;

        if decoded.len() != 32 {
            return Err(AppError::InvalidAddress(
                "NEAR public key must be 32 bytes".to_string(),
            ));
        }

        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&decoded);
        Ok(key_array)
    } else {
        Err(AppError::InvalidAddress(
            "NEAR public key must start with 'ed25519:'".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_near_public_key() {
        // Valid NEAR key
        let valid_key = "ed25519:DcNxKfxWcmsKiagzMGbhKPJ7bfqvDVe6QADMKzTy3P8=";
        let result = decode_near_public_key(valid_key);
        assert!(result.is_ok());

        // Invalid format
        let invalid_key = "invalid_key";
        let result = decode_near_public_key(invalid_key);
        assert!(result.is_err());

        // Wrong prefix
        let wrong_prefix = "sr25519:DcNxKfxWcmsKiagzMGbhKPJ7bfqvDVe6QADMKzTy3P8=";
        let result = decode_near_public_key(wrong_prefix);
        assert!(result.is_err());
    }
}
