use crate::adapters::traits::AssetInfo;
use crate::ledger::models::Chain;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TradeStatus {
    Pending,
    QuoteAccepted,
    PaymentReceived,
    ExecutingSwap,
    SwapCompleted,
    SettlementInProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trade {
    pub id: Uuid,
    pub user_id: Uuid,
    pub source_wallet_id: Uuid,
    pub destination_wallet_id: Uuid,
    pub source_chain: Chain,
    pub destination_chain: Chain,
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub amount_in: Decimal,
    pub amount_out_expected: Decimal,
    pub amount_out_actual: Option<Decimal>,
    pub dex_used: String,
    pub status: TradeStatus,
    pub quote_id: String,
    pub source_tx_hash: Option<String>,
    pub swap_tx_hash: Option<String>,
    pub destination_tx_hash: Option<String>,
    pub gas_fees_paid: Option<Decimal>,
    pub slippage_actual: Option<Decimal>,
    pub execution_price: Option<Decimal>,
    pub created_at: DateTime<Utc>,
    pub executed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeQuote {
    pub id: String,
    pub trade_id: Option<Uuid>,
    pub user_id: Uuid,
    pub source_chain: Chain,
    pub destination_chain: Chain,
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub dex_name: String,
    pub route: Vec<RouteStep>,
    pub total_gas_estimate: Decimal,
    pub total_slippage_percent: Decimal,
    pub execution_price: Decimal,
    pub rate_of_exchange: Decimal,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteStep {
    pub chain: Chain,
    pub dex: String,
    pub asset_in: AssetInfo,
    pub asset_out: AssetInfo,
    pub expected_output: Decimal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub trade_id: Uuid,
    pub status: TradeStatus,
    pub source_tx_hash: Option<String>,
    pub swap_tx_hash: Option<String>,
    pub destination_tx_hash: Option<String>,
    pub amount_received: Option<Decimal>,
    pub gas_paid: Decimal,
    pub slippage_actual: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementBridgeTransaction {
    pub id: Uuid,
    pub trade_id: Uuid,
    pub source_tx_hash: String,
    pub destination_tx_hash: Option<String>,
    pub source_amount: Decimal,
    pub destination_amount: Decimal,
    pub source_chain: Chain,
    pub destination_chain: Chain,
    pub status: TradeStatus,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapAggregatorResult {
    pub best_dex: String,
    pub amount_out: Decimal,
    pub minimum_amount: Decimal,
    pub execution_price: Decimal,
    pub slippage_percent: Decimal,
    pub gas_estimate: Decimal,
    pub settlement_tx: Option<String>,
}

impl Trade {
    pub fn new(
        user_id: Uuid,
        source_wallet_id: Uuid,
        destination_wallet_id: Uuid,
        source_chain: Chain,
        destination_chain: Chain,
        asset_in: AssetInfo,
        asset_out: AssetInfo,
        amount_in: Decimal,
        amount_out_expected: Decimal,
        dex_used: String,
        quote_id: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            source_wallet_id,
            destination_wallet_id,
            source_chain,
            destination_chain,
            asset_in,
            asset_out,
            amount_in,
            amount_out_expected,
            amount_out_actual: None,
            dex_used,
            status: TradeStatus::Pending,
            quote_id,
            source_tx_hash: None,
            swap_tx_hash: None,
            destination_tx_hash: None,
            gas_fees_paid: None,
            slippage_actual: None,
            execution_price: None,
            created_at: Utc::now(),
            executed_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    pub fn mark_quote_accepted(mut self) -> Self {
        self.status = TradeStatus::QuoteAccepted;
        self
    }

    pub fn mark_executing_swap(mut self, swap_tx: String) -> Self {
        self.status = TradeStatus::ExecutingSwap;
        self.swap_tx_hash = Some(swap_tx);
        self.executed_at = Some(Utc::now());
        self
    }

    pub fn mark_swap_completed(mut self, amount_out: Decimal, slippage: Decimal) -> Self {
        self.status = TradeStatus::SwapCompleted;
        self.amount_out_actual = Some(amount_out);
        self.slippage_actual = Some(slippage);
        self
    }

    pub fn mark_settlement_in_progress(mut self, dest_tx: String) -> Self {
        self.status = TradeStatus::SettlementInProgress;
        self.destination_tx_hash = Some(dest_tx);
        self
    }

    pub fn mark_completed(mut self) -> Self {
        self.status = TradeStatus::Completed;
        self.completed_at = Some(Utc::now());
        self
    }

    pub fn mark_failed(mut self, error: String) -> Self {
        self.status = TradeStatus::Failed;
        self.error_message = Some(error);
        self.completed_at = Some(Utc::now());
        self
    }

    pub fn can_execute(&self) -> bool {
        matches!(self.status, TradeStatus::QuoteAccepted)
    }
}

impl TradeQuote {
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn ttl_seconds(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds().max(0)
    }
}
