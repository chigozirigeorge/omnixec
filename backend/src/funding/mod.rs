// Funding adapters for detecting payments on each chain
// These monitor blockchain events and trigger quote commits

use crate::ledger::repository::LedgerRepository;
use std::sync::Arc;

pub struct StellarMonitor {
    horizon_url: String,
    ledger: Arc<LedgerRepository>,
}

impl StellarMonitor {
    pub fn new(horizon_url: String, ledger: Arc<LedgerRepository>) -> Self {
        Self { horizon_url, ledger }
    }

    pub async fn start(&self) {
        // Monitor Stellar payments via Horizon streaming
        // When payment detected with quote memo, trigger commit
    }
}

pub struct NearMonitor {
    rpc_url: String,
    ledger: Arc<LedgerRepository>,
}

impl NearMonitor {
    pub fn new(rpc_url: String, ledger: Arc<LedgerRepository>) -> Self {
        Self { rpc_url, ledger }
    }

    pub async fn start(&self) {
        // Monitor Near transactions via RPC
    }
}

pub struct SolanaMonitor {
    rpc_url: String,
    ledger: Arc<LedgerRepository>,
}

impl SolanaMonitor {
    pub fn new(rpc_url: String, ledger: Arc<LedgerRepository>) -> Self {
        Self { rpc_url, ledger }
    }

    pub async fn start(&self) {
        // Monitor Solana transactions via RPC
    }
}