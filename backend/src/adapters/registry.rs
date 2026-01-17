use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use crate::adapters::traits::DexAdapter;
use crate::ledger::models::Chain;

pub struct AdapterRegistry {
    dex_adapters: HashMap<String, Arc<dyn DexAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            dex_adapters: HashMap::new(),
        }
    }
    
    pub fn register_dex(&mut self, name: String, adapter: Arc<dyn DexAdapter>) {
        info!("Registering DEX adapter: {}", name);
        self.dex_adapters.insert(name, adapter);
    }
    
    pub fn get_dex(&self, name: &str) -> Option<Arc<dyn DexAdapter>> {
        self.dex_adapters.get(name).cloned()
    }
    
    pub async fn list_dexes_for_chain(&self, chain: Chain) -> Vec<String> {
        let mut result = Vec::new();
        for (name, adapter) in &self.dex_adapters {
            if let Ok(true) = adapter.is_available().await {
                if adapter.supported_chains().contains(&chain) {
                    result.push(name.clone());
                }
            }
        }
        result
    }
    
    pub async fn get_all_dexes_for_chain(&self, chain: Chain) -> Vec<Arc<dyn DexAdapter>> {
        let mut result = Vec::new();
        for (_, adapter) in &self.dex_adapters {
            if let Ok(true) = adapter.is_available().await {
                if adapter.supported_chains().contains(&chain) {
                    result.push(adapter.clone());
                }
            }
        }
        result
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
