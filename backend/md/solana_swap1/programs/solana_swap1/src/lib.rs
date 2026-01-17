pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("3gmvhqWzKGDc9oK5sSfmMZAviaKCoNbtXeKZsq8kzwa9");

#[program]
pub mod solana_swap1 {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }
}
