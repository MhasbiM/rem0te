use actix_web::{web, HttpRequest, HttpResponse};

use super::auth::extract_claims;
use super::{ApiResponse, AppState};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/connections")
            .route("", web::get().to(list_connections))
            .route("/peers", web::get().to(list_peers))
            .route("/relay-sessions", web::get().to(list_relay_sessions)),
    );
}

async fn list_connections(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    if extract_claims(&req, &state.jwt_secret).is_none() {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Unauthorized"));
    }

    let connections: Vec<serde_json::Value> = state
        .signaling
        .peers
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "peer_id": p.peer_id,
                "os": p.os,
                "hostname": p.hostname,
                "online": p.online,
                "addr": p.addr,
            })
        })
        .collect();

    HttpResponse::Ok().json(ApiResponse::ok(connections))
}

async fn list_peers(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    if extract_claims(&req, &state.jwt_secret).is_none() {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Unauthorized"));
    }

    let peers = state.signaling.get_peer_list();
    HttpResponse::Ok().json(ApiResponse::ok(peers))
}

async fn list_relay_sessions(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
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
