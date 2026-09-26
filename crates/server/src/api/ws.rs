//! WebSocket endpoint for real-time updates.

use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    use axum::extract::ws::Message;
    let welcome = serde_json::json!({
        "event": "connected",
        "message": "emusic-server WebSocket ready"
    });
    if socket
        .send(Message::Text(welcome.to_string().into()))
        .await
        .is_ok()
    {
        while let Some(Ok(_msg)) = socket.recv().await {
            // Keep connection alive for status updates
        }
    }
}
