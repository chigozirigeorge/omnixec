use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_address: String,
    pub stellar_horizon_url: String,
    pub near_rpc_url: String,
    pub solana_rpc_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, config::ConfigError> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost/backend".to_string()),
            bind_address: std::env::var("BIND_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            stellar_horizon_url: std::env::var("STELLAR_HORIZON_URL")
                .unwrap_or_else(|_| "https://horizon.stellar.org".to_string()),
            near_rpc_url: std::env::var("NEAR_RPC_URL")
                .unwrap_or_else(|_| "https://rpc.mainnet.near.org".to_string()),
            solana_rpc_url: std::env::var("SOLANA_RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
        })
    }
}