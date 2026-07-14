use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::auth::extract_claims;
use super::{ApiResponse, AppState};

#[derive(Deserialize)]
pub struct FileTransferRequest {
    pub from_peer: String,
    pub to_peer: String,
    pub file_path: String,
    pub direction: String, // "upload" or "download"
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/files")
            .route("/request", web::post().to(request_file_transfer))
            .route("/sessions", web::get().to(list_file_sessions)),
    );
}

async fn request_file_transfer(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<FileTransferRequest>,
) -> HttpResponse {
    if extract_claims(&req, &state.jwt_secret).is_none() {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Unauthorized"));
    }

    // Validate peers exist
    let from_online = state.signaling.peers.iter().any(|p| p.peer_id == body.from_peer && p.online);
    let to_online = state.signaling.peers.iter().any(|p| p.peer_id == body.to_peer && p.online);

    if !from_online || !to_online {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::err("One or both peers are offline"));
    }

    // Create a relay session for file transfer
    let session_id = state.relay.create_session(&body.from_peer);
    state.relay.join_session(&session_id, &body.to_peer);

    // Notify the target peer via signaling
    let msg = serde_json::json!({
        "type": "FileTransferRequest",
        "payload": {
            "session_id": session_id,
            "from_peer": body.from_peer,
            "file_path": body.file_path,
            "direction": body.direction,
        }
    });

    let msg_str = msg.to_string();

    // Send to target peer via WS or TCP
    if let Some(ws) = state.signaling.ws_connections.get(&body.to_peer) {
        let _ = ws.send(msg_str.clone());
    }
    // Also try TCP
    for conn in state.signaling.tcp_connections.iter() {
        if conn.key().contains(&body.to_peer) {
            let _ = conn.value().0.send(msg_str.clone());
        }
    }

    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "session_id": session_id,
        "status": "initiated",
    })))
}

async fn list_file_sessions(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    if extract_claims(&req, &state.jwt_secret).is_none() {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Unauthorized"));
    }

    let sessions: Vec<serde_json::Value> = state
        .relay
        .sessions
        .iter()
        .map(|s| {
            serde_json::json!({
                "session_id": s.session_id,
                "peer_a": s.peer_a,
                "peer_b": s.peer_b,
                "created_at": s.created_at.to_rfc3339(),
            })
        })
        .collect();

    HttpResponse::Ok().json(ApiResponse::ok(sessions))
}
