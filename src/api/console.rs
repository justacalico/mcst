//! Live console WebSocket: history + tailing log lines, commands in.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};

use crate::auth::AuthUser;
use crate::AppState;

/// `GET /api/servers/{id}/console` — upgrades to WS. Sends
/// `{"type":"history","lines":[...]}` then `{"type":"log","line":"..."}`.
/// Accepts `{"type":"command","command":"..."}` from the client.
pub async fn console_ws(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |sock| handle(s, id, sock))
}

async fn handle(s: Arc<AppState>, id: String, sock: WebSocket) {
    let Ok((history, mut rx)) = s.manager.console(&id).await else {
        return;
    };
    let (mut tx, mut rx_ws) = sock.split();
    let history_msg = serde_json::json!({"type": "history", "lines": history}).to_string();
    if tx.send(Message::Text(history_msg.into())).await.is_err() {
        return;
    }
    // Forward broadcast log lines.
    let mut forward = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(line) => {
                    let msg = serde_json::json!({"type": "log", "line": line}).to_string();
                    if tx.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    // Commands coming in over the socket.
    let mgr = s.manager.clone();
    let id2 = id.clone();
    let mut inbound = tokio::spawn(async move {
        while let Some(Ok(msg)) = rx_ws.next().await {
            let Message::Text(text) = msg else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if v.get("type").and_then(|t| t.as_str()) == Some("command") {
                if let Some(cmd) = v.get("command").and_then(|c| c.as_str()) {
                    let _ = mgr.send_command(&id2, cmd.trim_start_matches('/')).await;
                }
            }
        }
    });

    tokio::select! {
        _ = &mut forward => { inbound.abort(); }
        _ = &mut inbound => { forward.abort(); }
    }
}
