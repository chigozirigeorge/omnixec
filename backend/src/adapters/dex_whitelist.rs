use rust_decimal::prelude::FromPrimitive;
/// DEX whitelist and token verification system
/// This module manages which DEXes and tokens are trusted for cross-chain transactions

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use crate::ledger::models::Chain;
use crate::error::AppResult;
use rust_decimal::Decimal;
use std::str::FromStr;

/// Supported DEX platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SupportedDex {
    /// Solana: Raydium, Marinade, Orca
    Raydium,
    PhantomSwap,
    Orca,
    
    /// Stellar: SDEX, Stellar Asset Network
    StellarDex,
    
    /// NEAR: Ref Finance
    RefFinance,
}

impl SupportedDex {
    pub fn name(&self) -> &'static str {
        match self {
            SupportedDex::Raydium => "Raydium",
            SupportedDex::PhantomSwap => "PhantomSwap",
            SupportedDex::Orca => "Orca",
            SupportedDex::StellarDex => "Stellar DEX",
            SupportedDex::RefFinance => "Ref Finance",
        }
    }

    pub fn chain(&self) -> Chain {
        match self {
            SupportedDex::Raydium | SupportedDex::PhantomSwap | SupportedDex::Orca => Chain::Solana,
            SupportedDex::StellarDex => Chain::Stellar,
            SupportedDex::RefFinance => Chain::Near,
        }
    }
}

/// Whitelisted token with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistedToken {
    /// Chain-specific token identifier
    pub token_id: String,
    /// Human-readable symbol (e.g., USDC, SOL, XLM)
    pub symbol: String,
    /// Token name
    pub name: String,
    /// Number of decimals (e.g., 6 for USDC, 8 for NEAR)
    pub decimals: u8,
    /// Chain where token exists
    pub chain: Chain,
    /// Minimum transaction amount (in base units)
    pub min_amount: Decimal,
    /// Maximum transaction amount (in base units)
    pub max_amount: Decimal,
    /// Whether token is testnet or mainnet
    pub is_testnet: bool,
    /// DEXes where this token can be traded
    pub supported_dexes: Vec<SupportedDex>,
}

/// DEX whitelist manager
pub struct DexWhitelist {
    /// Tokens by chain and symbol
    tokens_by_chain: HashMap<Chain, HashMap<String, WhitelistedToken>>,
    /// Tokens by chain and token_id
    tokens_by_id: HashMap<Chain, HashMap<String, WhitelistedToken>>,
}

impl DexWhitelist {
    pub fn new() -> Self {
        let mut whitelist = DexWhitelist {
            tokens_by_chain: HashMap::new(),
            tokens_by_id: HashMap::new(),
        };

        // Initialize with testnet tokens
        whitelist.init_solana_testnet();
        whitelist.init_stellar_testnet();
        whitelist.init_near_testnet();

        whitelist
    }

    /// Initialize Solana testnet tokens
    fn init_solana_testnet(&mut self) {
        let tokens = vec![
            WhitelistedToken {
                token_id: "So11111111111111111111111111111111111111112".to_string(),
                symbol: "SOL".to_string(),
                name: "Wrapped SOL".to_string(),
                decimals: 9,
                chain: Chain::Solana,
                min_amount: Decimal::from(1000), // 0.000001 SOL
                max_amount: Decimal::from_i64(10_000_000_000).unwrap(), // 10k SOL
                is_testnet: true,
                supported_dexes: vec![SupportedDex::Raydium, SupportedDex::PhantomSwap, SupportedDex::Orca],
            },
            WhitelistedToken {
                token_id: "EPjFWaLb3odcccccccccccccccccccccccccccccc".to_string(),
                symbol: "USDC".to_string(),
                name: "USD Coin".to_string(),
                decimals: 6,
                chain: Chain::Solana,
                min_amount: Decimal::from(1_000), // $0.001
                max_amount: Decimal::from_i64(1_000_000_000_000).unwrap(), // $1B
                is_testnet: true,
                supported_dexes: vec![SupportedDex::Raydium, SupportedDex::Orca],
            },
            WhitelistedToken {
                token_id: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BAwbfo".to_string(),
                symbol: "USDT".to_string(),
                name: "Tether USD".to_string(),
                decimals: 6,
                chain: Chain::Solana,
                min_amount: Decimal::from(1_000), // $0.001
                max_amount: Decimal::from_i64(1_000_000_000_000).unwrap(), // $1B
                is_testnet: true,
                supported_dexes: vec![SupportedDex::Raydium, SupportedDex::PhantomSwap],
            },
        ];

        self.add_tokens(tokens);
    }

    /// Initialize Stellar testnet tokens
    fn init_stellar_testnet(&mut self) {
        let tokens = vec![
            WhitelistedToken {
                token_id: "native".to_string(),
                symbol: "XLM".to_string(),
                name: "Stellar Lumens".to_string(),
                decimals: 7,
                chain: Chain::Stellar,
                min_amount: Decimal::from(1), // 0.0000001 XLM
                max_amount: Decimal::from_i64(10_000_000_000).unwrap(), // 100M XLM
                is_testnet: true,
                supported_dexes: vec![SupportedDex::StellarDex],
            },
            WhitelistedToken {
                token_id: "USDC:GA5ZSEJYB37JRC5J3A7FUBRXVQBNDZTQYUWZONEQ5ESXISVHX3IDGISQ".to_string(),
                symbol: "USDC".to_string(),
                name: "USD Coin".to_string(),
                decimals: 6,
                chain: Chain::Stellar,
                min_amount: Decimal::from(1_000_000), // $0.01
                max_amount: Decimal::from_i64(1_000_000_000_000).unwrap(), // $1B
                is_testnet: true,
                supported_dexes: vec![SupportedDex::StellarDex],
            },
        ];

        self.add_tokens(tokens);
    }

    /// Initialize NEAR testnet tokens
    fn init_near_testnet(&mut self) {
        let tokens = vec![
            WhitelistedToken {
                token_id: "wrap.near".to_string(), // NEAR is native
                symbol: "NEAR".to_string(),
                name: "NEAR Protocol".to_string(),
                decimals: 24,
                chain: Chain::Near,
                min_amount: Decimal::from_i64(1_000_000_000_000_000_000).unwrap(), // 0.000001 NEAR
                max_amount: Decimal::from_str("1000000000000000000000").unwrap(), // 1k NEAR
                is_testnet: true,
                supported_dexes: vec![SupportedDex::RefFinance],
            },
            WhitelistedToken {
                token_id: "usdc.testnet".to_string(),
                symbol: "USDC".to_string(),
                name: "USD Coin".to_string(),
                decimals: 6,
                chain: Chain::Near,
                min_amount: Decimal::from(1_000), // $0.001
                max_amount: Decimal::from_i64(1_000_000_000_000).unwrap(), // $1B
                is_testnet: true,
                supported_dexes: vec![SupportedDex::RefFinance],
            },
        ];

        self.add_tokens(tokens);
    }

    fn add_tokens(&mut self, tokens: Vec<WhitelistedToken>) {
        for token in tokens {
            // Index by symbol
            self.tokens_by_chain
                .entry(token.chain)
                .or_insert_with(HashMap::new)
                .insert(token.symbol.clone(), token.clone());

            // Index by token_id
            self.tokens_by_id
                .entry(token.chain)
                .or_insert_with(HashMap::new)
                .insert(token.token_id.clone(), token);
        }
    }

    /// Get token by symbol and chain
    pub fn get_by_symbol(&self, chain: Chain, symbol: &str) -> AppResult<WhitelistedToken> {
        self.tokens_by_chain
            .get(&chain)
            .and_then(|m| m.get(symbol))
            .cloned()
            .ok_or_else(|| {
                crate::error::AppError::Config(format!(
                    "Token {} not whitelisted on {:?}",
                    symbol, chain
                ))
            })
    }

    /// Get token by token_id and chain
    pub fn get_by_id(&self, chain: Chain, token_id: &str) -> AppResult<WhitelistedToken> {
        self.tokens_by_id
            .get(&chain)
            .and_then(|m| m.get(token_id))
            .cloned()
            .ok_or_else(|| {
                crate::error::AppError::Config(format!(
                    "Token ID {} not whitelisted on {:?}",
                    token_id, chain
                ))
            })
    }

    /// Verify amount is within token limits
    pub fn verify_amount(
        &self,
        chain: Chain,
        symbol: &str,
        amount: Decimal,
    ) -> AppResult<()> {
        let token = self.get_by_symbol(chain, symbol)?;

        if amount < token.min_amount {
            return Err(crate::error::AppError::Config(format!(
                "Amount {} is below minimum {} for {}",
                amount, token.min_amount, token.symbol
            )));
        }

        if amount > token.max_amount {
            return Err(crate::error::AppError::Config(format!(
                "Amount {} exceeds maximum {} for {}",
                amount, token.max_amount, token.symbol
            )));
        }

        Ok(())
    }

    /// Check if token pair is tradeable on DEX
    pub fn verify_dex_pair(
        &self,
        dex: SupportedDex,
        from_chain: Chain,
        from_symbol: &str,
        to_chain: Chain,
        to_symbol: &str,
    ) -> AppResult<()> {
        let from_token = self.get_by_symbol(from_chain, from_symbol)?;
        let to_token = self.get_by_symbol(to_chain, to_symbol)?;

        if !from_token.supported_dexes.contains(&dex) {
            return Err(crate::error::AppError::Config(format!(
                "{} not supported on {} DEX",
                from_symbol,
                dex.name()
            )));
        }

        if !to_token.supported_dexes.contains(&dex) {
            return Err(crate::error::AppError::Config(format!(
                "{} not supported on {} DEX",
                to_symbol,
                dex.name()
            )));
        }

        Ok(())
    }

    /// Get all tokens for a chain
    pub fn get_tokens_for_chain(&self, chain: Chain) -> Vec<WhitelistedToken> {
        self.tokens_by_chain
            .get(&chain)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all supported DEXes for a chain
    pub fn get_dexes_for_chain(&self, chain: Chain) -> Vec<SupportedDex> {
        let mut dexes = HashSet::new();
        
        if let Some(tokens) = self.tokens_by_chain.get(&chain) {
            for token in tokens.values() {
                for dex in &token.supported_dexes {
                    dexes.insert(*dex);
                }
            }
        }

        dexes.into_iter().collect()
    }
}

impl Default for DexWhitelist {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solana_tokens_loaded() {
        let whitelist = DexWhitelist::new();
        let sol = whitelist.get_by_symbol(Chain::Solana, "SOL").unwrap();
        assert_eq!(sol.decimals, 9);
        assert_eq!(sol.chain, Chain::Solana);
    }

    #[test]
    fn test_token_amount_validation() {
        let whitelist = DexWhitelist::new();
        let amount = Decimal::from(500_000_000); // Valid amount
        assert!(whitelist.verify_amount(Chain::Solana, "SOL", amount).is_ok());
    }

    #[test]
    fn test_token_amount_too_small() {
        let whitelist = DexWhitelist::new();
        let amount = Decimal::from(1); // Too small
        assert!(whitelist.verify_amount(Chain::Solana, "SOL", amount).is_err());
    }

    #[test]
    fn test_dex_pair_verification() {
        let whitelist = DexWhitelist::new();
        assert!(whitelist
            .verify_dex_pair(
                SupportedDex::Raydium,
                Chain::Solana,
                "SOL",
                Chain::Solana,
                "USDC"
            )
            .is_ok());
    }
}
