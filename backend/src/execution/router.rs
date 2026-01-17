use crate::error::{AppResult, ExecutionError};
use crate::ledger::models::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, instrument};

/// Executor trait - implemented by each chain's executor
///
/// SECURITY: All executors must implement idempotent execution
#[async_trait]
pub trait Executor: Send + Sync {
    /// Execute a transaction on this chain using treasury funds
    ///
    /// INVARIANTS:
    /// - Must be idempotent (same quote never executes twice)
    /// - Must validate quote.execution_chain matches this executor
    /// - Must record execution atomically with spending
    async fn execute(&self, quote: &Quote) -> AppResult<Execution>;

    /// Get the chain this executor handles
    fn chain(&self) -> Chain;

    /// Check if executor has sufficient treasury balance
    async fn check_treasury_balance(&self, required: rust_decimal::Decimal) -> AppResult<()>;

    /// Get current treasury balance
    async fn get_treasury_balance(&self) -> AppResult<rust_decimal::Decimal>;

    /// Transfer funds from user wallet to treasury (for settlement)
    /// Returns transaction hash/ID
    async fn transfer_to_treasury(&self, token_or_asset: &str, amount: &str) -> AppResult<String>;

}


/// ExecutionRouter - routes executions to the appropriate chain executor
///
/// ARCHITECTURE: This is the key abstraction that enables symmetric cross-chain execution.
/// Any chain can be an execution target, and the router dynamically selects the correct executor.
pub struct ExecutionRouter {
    executors: HashMap<Chain, Arc<dyn Executor>>,
}

impl ExecutionRouter {
    /// Create a new execution router
    pub fn new() -> Self {
        Self {
            executors: HashMap::new(),
        }
    }

    /// Register an executor for a chain
    ///
    /// SECURITY: Only call this during system initialization
    pub fn register_executor(&mut self, chain: Chain, executor: Arc<dyn Executor>) {
        info!("Registering executor for chain: {:?}", chain);
        self.executors.insert(chain, executor);
    }

    /// Execute a quote on the appropriate chain
    ///
    /// SECURITY CRITICAL: This is the main entry point for all executions
    #[instrument(skip(self, quote), fields(quote_id = %quote.id, execution_chain = ?quote.execution_chain))]
    pub async fn execute(&self, quote: &Quote) -> AppResult<Execution> {
        info!(
            "Routing execution for quote {} to chain {:?}",
            quote.id, quote.execution_chain
        );

        // Validate quote has valid chain pair
        if !quote.has_valid_chain_pair() {
            return Err(ExecutionError::InvalidChainPair {
                funding: quote.funding_chain,
                execution: quote.execution_chain,
            }
            .into());
        }

        // Get executor for execution chain
        let executor = self
            .executors
            .get(&quote.execution_chain)
            .ok_or(ExecutionError::UnsupportedChain(quote.execution_chain))?;

        // Verify executor matches quote execution chain
        if executor.chain() != quote.execution_chain {
            return Err(ExecutionError::ExecutorChainMismatch {
                expected: quote.execution_chain,
                actual: executor.chain(),
            }
            .into());
        }

        // Check treasury balance before execution
        executor
            .check_treasury_balance(quote.execution_cost)
            .await?;

        // Execute on target chain
        executor.execute(quote).await
    }

    /// Get all registered chains
    pub fn registered_chains(&self) -> Vec<Chain> {
        self.executors.keys().copied().collect()
    }

    /// Check if a chain is supported for execution
    pub fn supports_chain(&self, chain: Chain) -> bool {
        self.executors.contains_key(&chain)
    }

    /// Get treasury balances for all chains
    pub async fn get_all_treasury_balances(
        &self,
    ) -> AppResult<HashMap<Chain, rust_decimal::Decimal>> {
        let mut balances = HashMap::new();

        for (chain, executor) in &self.executors {
            let balance = executor.get_treasury_balance().await?;
            balances.insert(*chain, balance);
        }

        Ok(balances)
    }
}

impl Default for ExecutionRouter {
    fn default() -> Self {
        Self::new()
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use rust_decimal_macros::dec;

//     // Mock executor for testing
//     struct MockExecutor {
//         chain: Chain,
//     }

//     #[async_trait]
//     impl Executor for MockExecutor {
//         async fn execute(&self, quote: &Quote) -> AppResult<Execution> {
//             Ok(Execution {
//                 id: uuid::Uuid::new_v4(),
//                 quote_id: quote.id,
//                 execution_chain: self.chain,
//                 transaction_hash: Some("mock_tx".to_string()),
//                 status: ExecutionStatus::Success,
//                 gas_used: Some(dec!(5000)),
//                 error_message: None,
//                 retry_count: 0,
//                 executed_at: chrono::Utc::now(),
//                 completed_at: Some(chrono::Utc::now()),
//             })
//         }

//         fn chain(&self) -> Chain {
//             self.chain
//         }

//         async fn check_treasury_balance(
//             &self,
//             _required: rust_decimal::Decimal,
//         ) -> AppResult<()> {
//             Ok(())
//         }

//         async fn get_treasury_balance(&self) -> AppResult<rust_decimal::Decimal> {
//             Ok(dec!(1000000))
//         }
//     }

//     #[tokio::test]
//     async fn test_router_registration() {
//         let mut router = ExecutionRouter::new();

//         let solana_executor = Arc::new(MockExecutor {
//             chain: Chain::Solana,
//         });
//         let stellar_executor = Arc::new(MockExecutor {
//             chain: Chain::Stellar,
//         });

//         router.register_executor(Chain::Solana, solana_executor);
//         router.register_executor(Chain::Stellar, stellar_executor);

//         assert!(router.supports_chain(Chain::Solana));
//         assert!(router.supports_chain(Chain::Stellar));
//         assert!(!router.supports_chain(Chain::Near));
//     }

//     #[test]
//     fn test_chain_pair_validation() {
//         // Same chain should not be valid
//         let quote = Quote {
//             id: uuid::Uuid::new_v4(),
//             user_id: uuid::Uuid::new_v4(),
//             funding_chain: Chain::Solana,
//             execution_chain: Chain::Solana, // Same!
//             funding_asset: "SOL".to_string(),
//             execution_asset: "SOL".to_string(),
//             max_funding_amount: dec!(1000000),
//             execution_cost: dec!(1000000),
//             service_fee: dec!(1000),
//             execution_instructions: vec![],
//             estimated_compute_units: None,
//             nonce: "test".to_string(),
//             status: QuoteStatus::Pending,
//             expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
//             payment_address: None,
//             created_at: chrono::Utc::now(),
//             updated_at: chrono::Utc::now(),
//         };

//         assert!(!quote.has_valid_chain_pair());
//     }
// }
