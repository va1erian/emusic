//! WebSocket endpoint broadcasting scan and library events.

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;

use crate::auth::middleware::AuthDevice;
use crate::state::AppState;

/// `GET /api/v1/ws`
pub async fn ws(
    State(state): State<AppState>,
    AuthDevice(_): AuthDevice,
    upgrade: WebSocketUpgrade,
) -> Response {
    // Clients only receive events; cap inbound frames so a peer cannot send
    // large messages.
    let upgrade = upgrade
        .max_message_size(64 * 1024)
        .max_frame_size(16 * 1024);
    upgrade.on_upgrade(move |socket| handle(socket, state))
}

async fn handle(mut socket: WebSocket, state: AppState) {
    let mut events = state.scan.subscribe();
    let status = state.scan.status().await;
    let initial = serde_json::json!({ "type": "status", "status": status });
    if send_json(&mut socket, &initial).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) => {
                    if send_json(&mut socket, &event).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(Message::Ping(payload))) => {
                    if socket.send(Message::Pong(payload)).await.is_err() {
                        break;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) => break,
            },
        }
    }
}

async fn send_json(socket: &mut WebSocket, value: &impl Serialize) -> Result<(), ()> {
    let text = serde_json::to_string(value).map_err(|_| ())?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| ())
}
