use crate::error::{AppError, AppResult};
use crate::ledger::models::Chain;
use crate::trading::models::{Trade, TradeStatus};
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;

pub struct TradeRepository {
    trades: tokio::sync::RwLock<HashMap<Uuid, Trade>>,
}

impl TradeRepository {
    pub fn new() -> Self {
        Self {
            trades: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_trade(&self, trade: Trade) -> AppResult<Trade> {
        let mut trades = self.trades.write().await;
        trades.insert(trade.id, trade.clone());
        Ok(trade)
    }

    pub async fn get_trade(&self, trade_id: Uuid) -> AppResult<Trade> {
        let trades = self.trades.read().await;
        trades
            .get(&trade_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Trade {} not found", trade_id)))
    }

    pub async fn update_trade(&self, trade: Trade) -> AppResult<Trade> {
        let mut trades = self.trades.write().await;
        trades.insert(trade.id, trade.clone());
        Ok(trade)
    }

    pub async fn get_user_trades(&self, user_id: Uuid) -> AppResult<Vec<Trade>> {
        let trades = self.trades.read().await;
        let user_trades = trades
            .values()
            .filter(|t| t.user_id == user_id)
            .cloned()
            .collect();
        Ok(user_trades)
    }

    pub async fn get_user_trades_by_status(
        &self,
        user_id: Uuid,
        status: TradeStatus,
    ) -> AppResult<Vec<Trade>> {
        let trades = self.trades.read().await;
        let user_trades = trades
            .values()
            .filter(|t| t.user_id == user_id && t.status == status)
            .cloned()
            .collect();
        Ok(user_trades)
    }

    pub async fn get_user_trades_by_chain(
        &self,
        user_id: Uuid,
        source_chain: Chain,
    ) -> AppResult<Vec<Trade>> {
        let trades = self.trades.read().await;
        let user_trades = trades
            .values()
            .filter(|t| t.user_id == user_id && t.source_chain == source_chain)
            .cloned()
            .collect();
        Ok(user_trades)
    }

    pub async fn mark_executing(&self, trade_id: Uuid, swap_tx: String) -> AppResult<Trade> {
        let mut trades = self.trades.write().await;
        let trade = trades
            .get_mut(&trade_id)
            .ok_or_else(|| AppError::NotFound(format!("Trade {} not found", trade_id)))?;

        trade.status = TradeStatus::ExecutingSwap;
        trade.swap_tx_hash = Some(swap_tx);
        trade.executed_at = Some(Utc::now());

        Ok(trade.clone())
    }

    pub async fn mark_swap_completed(
        &self,
        trade_id: Uuid,
        amount_out: Decimal,
        slippage: Decimal,
        execution_price: Decimal,
    ) -> AppResult<Trade> {
        let mut trades = self.trades.write().await;
        let trade = trades
            .get_mut(&trade_id)
            .ok_or_else(|| AppError::NotFound(format!("Trade {} not found", trade_id)))?;

        trade.status = TradeStatus::SwapCompleted;
        trade.amount_out_actual = Some(amount_out);
        trade.slippage_actual = Some(slippage);
        trade.execution_price = Some(execution_price);

        Ok(trade.clone())
    }

    pub async fn mark_settlement_in_progress(
        &self,
        trade_id: Uuid,
        destination_tx: String,
    ) -> AppResult<Trade> {
        let mut trades = self.trades.write().await;
        let trade = trades
            .get_mut(&trade_id)
            .ok_or_else(|| AppError::NotFound(format!("Trade {} not found", trade_id)))?;

        trade.status = TradeStatus::SettlementInProgress;
        trade.destination_tx_hash = Some(destination_tx);

        Ok(trade.clone())
    }

    pub async fn mark_completed(&self, trade_id: Uuid) -> AppResult<Trade> {
        let mut trades = self.trades.write().await;
        let trade = trades
            .get_mut(&trade_id)
            .ok_or_else(|| AppError::NotFound(format!("Trade {} not found", trade_id)))?;

        trade.status = TradeStatus::Completed;
        trade.completed_at = Some(Utc::now());

        Ok(trade.clone())
    }

    pub async fn mark_failed(&self, trade_id: Uuid, error: String) -> AppResult<Trade> {
        let mut trades = self.trades.write().await;
        let trade = trades
            .get_mut(&trade_id)
            .ok_or_else(|| AppError::NotFound(format!("Trade {} not found", trade_id)))?;

        trade.status = TradeStatus::Failed;
        trade.error_message = Some(error);
        trade.completed_at = Some(Utc::now());

        Ok(trade.clone())
    }

    pub async fn get_pending_settlements(&self) -> AppResult<Vec<Trade>> {
        let trades = self.trades.read().await;
        let pending = trades
            .values()
            .filter(|t| t.status == TradeStatus::SettlementInProgress)
            .cloned()
            .collect();
        Ok(pending)
    }

    pub async fn clear_all(&self) -> AppResult<()> {
        let mut trades = self.trades.write().await;
        trades.clear();
        Ok(())
    }
}
