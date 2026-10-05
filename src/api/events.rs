//! Panel-wide events WebSocket: status changes, stats ticks, player counts.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};

use crate::auth::AuthUser;
use crate::AppState;

pub async fn events_ws(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |sock| handle(s, sock))
}

async fn handle(s: Arc<AppState>, sock: WebSocket) {
    let mut rx = s.manager.events();
    let (mut tx, mut rx_ws) = sock.split();
    let mut forward = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let Ok(text) = serde_json::to_string(&ev) else { continue };
                    if tx.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });
    // Drain incoming (pings/close) so the connection stays healthy.
    let mut drain = tokio::spawn(async move {
        while let Some(Ok(msg)) = rx_ws.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });
    tokio::select! {
        _ = &mut forward => { drain.abort(); }
        _ = &mut drain => { forward.abort(); }
    }
}
