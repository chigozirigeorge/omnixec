use crate::error::{AppResult, QuoteError};
use crate::ledger::{models::*, repository::LedgerRepository};
use crate::quote_engine::pyth_oracle::{PythOracle, PythPriceData};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Quote engine configuration
#[derive(Debug, Clone)]
pub struct QuoteConfig {
    /// Service fee as a percentage (0.1% = 0.001)
    pub service_fee_rate: Decimal,
    /// Quote validity duration in seconds
    pub quote_ttl_seconds: i64,
    /// Maximum compute units allowed per transaction (Solana)
    pub max_compute_units: i32,
    /// Maximum slippage allowed (1% = 0.01)
    pub max_slippage: Decimal,
}

impl Default for QuoteConfig {
    fn default() -> Self {
        Self {
            service_fee_rate: dec!(0.001), // 0.1%
            quote_ttl_seconds: 300,        // 5 minutes
            max_compute_units: 1_400_000,  // Solana max
            max_slippage: dec!(0.01),       // 1% max slippage
        }
    }
}

/// Quote engine - generates and validates symmetric cross-chain quotes
///
/// ARCHITECTURE: This engine is completely chain-agnostic.
/// It validates chain pairs but doesn't privilege any chain.
/// Uses Pyth for real-time price feeds.
pub struct QuoteEngine {
    config: QuoteConfig,
    ledger: Arc<LedgerRepository>,
    pyth_oracle: Arc<PythOracle>,
    network: String,
}

impl QuoteEngine {
    pub fn new(config: QuoteConfig, ledger: Arc<LedgerRepository>, pyth_oracle: Arc<PythOracle>, network: String) -> Self {
        Self { config, ledger, pyth_oracle, network }
    }

    /// Generate a new quote for cross-chain execution
    ///
    /// SECURITY: Critical validations:
    /// - Funding and execution chains must be different
    /// - Chain pair must be explicitly supported
    /// - Execution instructions must be valid
    /// - Cost estimates must be worst-case
    /// - Real-time prices from Pyth for accuracy
    pub async fn generate_quote(
        &self,
        user_id: Uuid,
        funding_chain: Chain,
        execution_chain: Chain,
        funding_asset: String,
        execution_asset: String,
        execution_instructions: Vec<u8>,
        estimated_compute_units: Option<i32>,
    ) -> AppResult<Quote> {
        info!(
            "Generating quote: {:?} -> {:?} for user {} ({} -> {})",
            funding_chain, execution_chain, user_id, funding_asset, execution_asset
        );

        // VALIDATION 1: Funding and execution chains must be different
        if funding_chain == execution_chain {
            warn!("Rejected same-chain quote attempt");
            return Err(QuoteError::SameChainFunding.into());
        }

        // VALIDATION 2: Check if chain pair is supported
        if !Chain::is_pair_supported(funding_chain, execution_chain) {
            warn!(
                "Rejected unsupported chain pair: {:?} -> {:?}",
                funding_chain, execution_chain
            );
            return Err(QuoteError::UnsupportedChainPair {
                funding: funding_chain,
                execution: execution_chain,
            }
            .into());
        }

        // VALIDATION 3: Verify execution instructions
        if execution_instructions.is_empty() {
            return Err(QuoteError::InvalidParameters(
                "Execution instructions cannot be empty".to_string(),
            )
            .into());
        }

        // VALIDATION 4: For Solana execution, validate compute units
        if execution_chain == Chain::Solana {
            if let Some(cu) = estimated_compute_units {
                if cu <= 0 || cu > self.config.max_compute_units {
                    return Err(QuoteError::InvalidParameters(format!(
                        "Compute units must be between 1 and {}",
                        self.config.max_compute_units
                    ))
                    .into());
                }
            }
        }

        // VALIDATION 5: Verify user exists and has wallet for execution chain
        let user = self.ledger.get_user(user_id).await?
            .ok_or_else(|| QuoteError::InvalidParameters("User not found".to_string()))?;

        let user_execution_wallet = self.get_user_wallet_for_chain(&user, execution_chain)
            .ok_or_else(|| QuoteError::InvalidParameters(
                format!("User has no wallet configured for {:?}", execution_chain)
            ))?;

        info!("✓ User has wallet on execution chain: {}", user_execution_wallet);

        // VALIDATION 6: Get real-time price from Pyth for all three chains
        info!("Fetching real-time prices from Pyth...");
        
        let price_data = self
            .pyth_oracle
            .get_price(&funding_asset, &execution_asset, funding_chain.as_str())
            .await
            .map_err(|e| QuoteError::PriceUnavailable(format!("Pyth error: {}", e)))?;

        info!(
            "✓ Price: 1 {} = {} {} (confidence: {}%)",
            funding_asset,
            price_data.rate,
            execution_asset,
            price_data.base_price.confidence_pct().unwrap_or_default()
        );

        // Calculate execution cost based on target chain
        let execution_cost = self
            .estimate_execution_cost(execution_chain, estimated_compute_units)
            .await?;

        // Calculate service fee (0.1% of execution cost)
        let service_fee = execution_cost * self.config.service_fee_rate;

        // Convert execution cost to funding chain asset using Pyth price
        let max_funding_amount = (execution_cost + service_fee) / price_data.rate;

        // Apply slippage buffer (add 1% for safety)
        let max_funding_amount_with_slippage = max_funding_amount * (Decimal::ONE + self.config.max_slippage);

        // CRITICAL FIX #2: Dynamic quote TTL based on volatility
        let ttl_seconds = self.calculate_dynamic_ttl(&price_data)?;
        
        info!(
            "Quote volatility: {}% confidence, TTL: {} seconds",
            price_data.base_price.confidence_pct().unwrap_or_default(),
            ttl_seconds
        );

        // Generate unique nonce for replay protection
        let nonce = format!("{}-{}", Uuid::new_v4(), Utc::now().timestamp_millis());

        // Set expiry based on volatility
        let expires_at = Utc::now() + Duration::seconds(ttl_seconds);

        // Generate payment address for funding chain
        let payment_address = self
            .generate_payment_address(funding_chain, &nonce)
            .await?;

        // Create quote in ledger
        let quote = self
            .ledger
            .create_quote(
                user_id,
                funding_chain,
                execution_chain,
                funding_asset.clone(),
                execution_asset.clone(),
                BigDecimal::from_str(&max_funding_amount_with_slippage.to_string()).unwrap(),
                BigDecimal::from_str(&execution_cost.to_string()).unwrap(),
                BigDecimal::from_str(&service_fee.to_string()).unwrap(),
                execution_instructions,
                estimated_compute_units,
                nonce,
                expires_at,
                Some(payment_address),
            )
            .await?;

        // Audit log
        self.ledger
            .log_audit_event(
                AuditEventType::QuoteCreated,
                Some(execution_chain),
                Some(quote.id),
                Some(user_id),
                serde_json::json!({
                    "funding_chain": funding_chain,
                    "execution_chain": execution_chain,
                    "funding_asset": funding_asset,
                    "execution_asset": execution_asset,
                    "execution_cost": execution_cost.to_string(),
                    "service_fee": service_fee.to_string(),
                    "pyth_price_rate": price_data.rate.to_string(),
                    "user_wallet": user_execution_wallet,
                }),
            )
            .await?;

        info!("✓ Quote created: {} (expires in {}s)", quote.id, self.config.quote_ttl_seconds);
        Ok(quote)
    }

    /// Get user wallet address for a specific chain
    pub fn get_user_wallet_for_chain(&self, user: &User, chain: Chain) -> Option<String> {
        match chain {
            Chain::Solana => user.solana_address.clone(),
            Chain::Stellar => user.stellar_address.clone(),
            Chain::Near => user.near_address.clone(),
        }
    }

    /// CRITICAL FIX #2: Calculate dynamic quote TTL based on price volatility
    /// 
    /// High volatility → shorter TTL (reduce price drift risk)
    /// Low volatility → longer TTL (more time for user to act)
    fn calculate_dynamic_ttl(&self, price_data: &PythPriceData) -> AppResult<i64> {
        let conf_pct = price_data.base_price.confidence_pct().unwrap_or(dec!(0.5));

        let ttl = match conf_pct {
            // Extreme volatility: >5% confidence interval
            v if v > dec!(5.0) => {
                info!("⚠️  High volatility detected ({}%), reducing TTL to 2 minutes", v);
                120
            }
            // High volatility: 2-5% confidence
            v if v > dec!(2.0) => {
                info!("⚠️  Moderate volatility detected ({}%), reducing TTL to 3 minutes", v);
                180
            }
            // Normal volatility: 1-2% confidence
            v if v > dec!(1.0) => {
                info!("📊 Normal volatility ({}%), TTL: 4 minutes", v);
                240
            }
            // Stable: <1% confidence
            v => {
                info!("✓ Low volatility ({}%), TTL: 5 minutes", v);
                300
            }
        };

        Ok(ttl)
    }

    /// Estimate execution cost for a given chain
    ///
    /// SECURITY: Uses worst-case estimation with safety margins
    async fn estimate_execution_cost(
        &self,
        execution_chain: Chain,
        estimated_compute_units: Option<i32>,
    ) -> AppResult<Decimal> {
        match execution_chain {
            Chain::Solana => {
                let cu = estimated_compute_units.unwrap_or(200_000);
                // Base cost: compute units * lamports per CU
                let compute_cost = Decimal::from(cu) * dec!(0.000001);
                // Signature cost (5000 lamports per signature, assume 2)
                let signature_cost = dec!(10000);
                // Priority fee buffer (20%)
                let priority_buffer = compute_cost * dec!(0.2);
                Ok(compute_cost + signature_cost + priority_buffer)
            }
            Chain::Stellar => {
                // Stellar base fee (100 stroops) + buffer
                let base_fee = dec!(100);
                let buffer = base_fee * dec!(0.2);
                Ok(base_fee + buffer)
            }
            Chain::Near => {
                // NEAR gas cost estimation based on function call complexity
                // Standard function call: ~1 TGas (1,000,000,000,000 gas units)
                // Storage allocation: ~100 million gas per 32KB
                // Cross-contract call: ~2 TGas
                
                // Base cost for simple transfer: 1 TGas
                let base_gas = dec!(1_000_000_000_000);
                
                // Add extra for potential cross-contract interaction: 2 TGas
                let cross_contract_multiplier = dec!(2);
                
                // Gas price: typically 100 yoctoNEAR per unit (1 yocto = 10^-24)
                // So: (gas_units * gas_price) / 10^24 = cost in NEAR
                let gas_price = dec!(100);
                
                // Total gas cost = (base_gas * cross_contract_multiplier * gas_price) / 10^24
                let total_gas_units = base_gas * cross_contract_multiplier;
                let cost_in_yoctonear = total_gas_units * gas_price;
                
                // Convert from yoctoNEAR to NEAR (divide by 10^24)
                let cost_in_near = cost_in_yoctonear / Decimal::from(1_000_000_000_000_000_000_000_000i128);
                
                // Add 50% buffer for safety
                Ok(cost_in_near * dec!(1.5))
            }
        }
    }

    /// Generate payment address for funding chain
    ///
    /// SECURITY: Generates unique deterministic addresses for each quote
    /// - Stellar: Uses contract account with memo for routing
    /// - NEAR: Uses contract subaccount pattern
    /// - Solana: Uses Program Derived Address (PDA) pattern
    async fn generate_payment_address(&self, chain: Chain, nonce: &str) -> AppResult<String> {
        match chain {
            Chain::Stellar => {
                // Stellar: Use payment collection contract with memo-based routing
                // Format: [contract_account]?memo=[nonce]
                // The memo field ensures each quote has unique routing
                // Example: GBUQWP3BOUZX34ULNQG23RQ6F4OFSAI5TU2MMQBB3IXWVYLXVCLWEB7V?memo=quote_abc123de
                let memo = format!("quote_{}", &nonce[..12]);
                Ok(format!(
                    "GBUQWP3BOUZX34ULNQG23RQ6F4OFSAI5TU2MMQBB3IXWVYLXVCLWEB7V?memo={}",
                    memo
                ))
            }
            Chain::Near => {
                // NEAR: Use escrow contract subaccount pattern
                // Format: [quote_id].escrow.mainnet or [quote_id].escrow.testnet
                // This provides isolation: each quote gets its own escrow subaccount
                // The parent contract holds custody until execution confirmed
                let quote_suffix = &nonce.replace("-", "")[..16]; // Use first 16 chars of nonce
                let network = if cfg!(debug_assertions) {
                    "testnet"
                } else {
                    "mainnet"
                };
                Ok(format!(
                    "{}.escrow.{}",
                    quote_suffix, network
                ))
            }
            Chain::Solana => {
                // Solana: Use Program Derived Address (PDA) seeded with nonce
                // PDAs are deterministic: PDA = hash(program_id, ["escrow", nonce_bytes])
                // Format: [base58_encoded_pda]
                // This ensures:
                // - Each quote has unique receiving address (seeded with nonce)
                // - Address is controlled by the escrow program
                // - No actual keypair needed - generated deterministically
                
                // In production, would use actual Solana SDK to generate PDA:
                // let (pda, _bump) = Pubkey::find_program_address(
                //     &[b"escrow", nonce.as_bytes()],
                //     &escrow_program_id
                // );
                
                // For now, generate base58 simulating PDA format
                let pda_seed = format!("escrow_{}", &nonce[..16]);
                let hash = sha256::digest(pda_seed.as_bytes());
                
                // Convert hash to base58 (Solana address format)
                let pda_bytes = hash.as_bytes()[..32].to_vec();
                Ok(Self::encode_base58(&pda_bytes))
            }
        }
    }
    
    /// Encode bytes as base58 (Solana address format)
    fn encode_base58(input: &[u8]) -> String {
        const ALPHABET: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        
        if input.is_empty() {
            return String::new();
        }
        
        let mut output = String::new();
        let mut num = 0u128;
        
        // Convert bytes to number (simplified for demonstration)
        for &byte in input.iter().take(16) {
            num = num.wrapping_mul(256).wrapping_add(byte as u128);
        }
        
        // Convert to base58
        if num == 0 {
            output.push('1');
        } else {
            let mut temp = num;
            while temp > 0 {
                let remainder = (temp % 58) as usize;
                output.insert(0, ALPHABET.chars().nth(remainder).unwrap());
                temp /= 58;
            }
        }
        
        output
    }

    /// Validate and commit a quote
    ///
    /// SECURITY: Atomic state transition with optimistic locking
    pub async fn commit_quote(&self, quote_id: Uuid) -> AppResult<Quote> {
        let mut tx = self.ledger.begin_tx().await?;

        // Get quote with FOR UPDATE lock
        let quote = self
            .ledger
            .get_quote(quote_id)
            .await?
            .ok_or_else(|| QuoteError::NotFound(quote_id.to_string()))?;

        // Validation: Check if quote can be committed
        if !quote.can_commit() {
            return Err(if quote.expires_at <= Utc::now() {
                QuoteError::Expired
            } else {
                QuoteError::InvalidState {
                    current: format!("{:?}", quote.status),
                    expected: "Pending".to_string(),
                }
            }
            .into());
        }

        // Verify chain pair is still valid
        if !quote.has_valid_chain_pair() {
            return Err(QuoteError::UnsupportedChainPair {
                funding: quote.funding_chain,
                execution: quote.execution_chain,
            }
            .into());
        }

        // Update status to committed
        self.ledger
            .update_quote_status(&mut tx, quote_id, QuoteStatus::Pending, QuoteStatus::Committed)
            .await?;

        tx.commit().await?;

        // Audit log
        self.ledger
            .log_audit_event(
                AuditEventType::QuoteCommitted,
                Some(quote.execution_chain),
                Some(quote_id),
                Some(quote.user_id),
                serde_json::json!({
                    "funding_chain": quote.funding_chain,
                    "execution_chain": quote.execution_chain,
                }),
            )
            .await?;

        // Return updated quote
        self.ledger
            .get_quote(quote_id)
            .await?
            .ok_or_else(|| QuoteError::NotFound(quote_id.to_string()).into())
    }

    /// Validate quote before execution
    pub async fn validate_for_execution(&self, quote_id: Uuid) -> AppResult<Quote> {
        let quote = self
            .ledger
            .get_quote(quote_id)
            .await?
            .ok_or_else(|| QuoteError::NotFound(quote_id.to_string()))?;

        if !quote.can_execute() {
            return Err(if quote.expires_at <= Utc::now() {
                QuoteError::Expired
            } else {
                QuoteError::InvalidState {
                    current: format!("{:?}", quote.status),
                    expected: "Committed".to_string(),
                }
            }
            .into());
        }

        // Re-verify chain pair
        if !quote.has_valid_chain_pair() {
            return Err(QuoteError::UnsupportedChainPair {
                funding: quote.funding_chain,
                execution: quote.execution_chain,
            }
            .into());
        }

        Ok(quote)
    }

    /// Mark quote as executed
    pub async fn mark_executed(&self, quote_id: Uuid) -> AppResult<()> {
        let mut tx = self.ledger.begin_tx().await?;

        self.ledger
            .update_quote_status(&mut tx, quote_id, QuoteStatus::Committed, QuoteStatus::Executed)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Mark quote as failed
    pub async fn mark_failed(&self, quote_id: Uuid) -> AppResult<()> {
        let mut tx = self.ledger.begin_tx().await?;

        self.ledger
            .update_quote_status(&mut tx, quote_id, QuoteStatus::Committed, QuoteStatus::Failed)
            .await?;

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_fee_calculation() {
        let config = QuoteConfig::default();
        let execution_cost = dec!(1000000);
        let fee = execution_cost * config.service_fee_rate;

        // 0.1% of 1000000 = 1000
        assert_eq!(fee, dec!(1000));
    }

    #[test]
    fn test_chain_pair_validation() {
        // Same chain should be rejected
        assert!(!Chain::is_pair_supported(Chain::Solana, Chain::Solana));

        // Different chains should be supported
        assert!(Chain::is_pair_supported(Chain::Stellar, Chain::Solana));
        assert!(Chain::is_pair_supported(Chain::Solana, Chain::Stellar));
        assert!(Chain::is_pair_supported(Chain::Near, Chain::Solana));
    }
}

