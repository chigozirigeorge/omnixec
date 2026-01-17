use crate::adapters::traits::AssetInfo;
use crate::api::handler::AppState;
use crate::ledger::models::Chain;
use crate::quote_engine::realtime::RealtimeQuoteEngine;
use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{interval, Duration};

#[derive(Deserialize)]
pub struct QuoteStreamRequest {
    pub asset_in_chain: Chain,
    pub asset_in_address: String,
    pub asset_out_chain: Chain,
    pub asset_out_address: String,
    pub amount: Decimal,
    pub update_interval_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct QuoteStreamUpdate {
    pub best_dex: String,
    pub best_amount_out: Decimal,
    pub best_rate: Decimal,
    pub best_slippage: Decimal,
    pub all_quotes: Vec<DexQuoteUpdate>,
    pub timestamp: String,
}

#[derive(Serialize)]
pub struct DexQuoteUpdate {
    pub dex_name: String,
    pub amount_out: Decimal,
    pub rate: Decimal,
    pub slippage_percent: Decimal,
}

pub async fn stream_quotes(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move | socket| handle_quote_stream(socket, state.realtime_quote_engine.clone()))
}

async fn handle_quote_stream(
    socket: WebSocket,
    engine: Arc<RealtimeQuoteEngine>,
) {
    let (mut sender, mut receiver) = socket.split();

    // Spawn task to read incoming messages
    let engine_clone = engine.clone();
    let mut recv_task = tokio::spawn(async move {
        let mut request: Option<QuoteStreamRequest> = None;
        let mut update_interval = Duration::from_millis(500);

        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<QuoteStreamRequest>(&text) {
                        Ok(req) => {
                            update_interval =
                                Duration::from_millis(req.update_interval_ms.unwrap_or(500));
                            request = Some(req);
                        }
                        Err(e) => {
                            eprintln!("Failed to parse quote request: {}", e);
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
        (request, update_interval)
    });

    // Spawn task to send updates
    let send_task = tokio::spawn(async move {
        let mut last_request = None;
        let mut update_interval = Duration::from_millis(500);
        let mut ticker = interval(update_interval);

        loop {
            tokio::select! {
                result = &mut recv_task => {
                    if let Ok((req, intervals)) = result {
                        last_request = req;
                        update_interval = intervals;
                        ticker = interval(update_interval);
                    }
                    break;
                }
                _ = ticker.tick() => {
                    if let Some(ref req) = last_request {
                        let asset_in = AssetInfo {
                            chain: req.asset_in_chain,
                            address: req.asset_in_address.clone(),
                            symbol: String::new(),
                            name: String::new(),
                            decimals: 18,
                            logo_url: None,
                        };

                        let asset_out = AssetInfo {
                            chain: req.asset_out_chain,
                            address: req.asset_out_address.clone(),
                            symbol: String::new(),
                            name: String::new(),
                            decimals: 18,
                            logo_url: None,
                        };

                        match engine_clone
                            .get_best_quote(&asset_in, &asset_out, req.amount)
                            .await
                        {
                            Ok(quote) => {
                                let update = QuoteStreamUpdate {
                                    best_dex: quote.best_dex,
                                    best_amount_out: quote.best_amount_out,
                                    best_rate: quote.best_rate,
                                    best_slippage: quote.best_slippage,
                                    all_quotes: quote
                                        .all_quotes
                                        .iter()
                                        .map(|q| DexQuoteUpdate {
                                            dex_name: q.dex_name.clone(),
                                            amount_out: q.amount_out,
                                            rate: q.rate,
                                            slippage_percent: q.slippage_percent,
                                        })
                                        .collect(),
                                    timestamp: quote.timestamp.to_rfc3339(),
                                };

                                if let Ok(json) = serde_json::to_string(&update) {
                                    if sender
                                        .send(Message::Text(json))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                }
            }
        }
    });

    send_task.await.ok();
}
