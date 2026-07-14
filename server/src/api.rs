mod auth;
mod connections;
mod file_transfer;
mod users;

use actix_web::web;
use std::sync::Arc;

use crate::relay::RelayState;
use crate::signaling::SignalingState;

#[derive(Clone)]
pub struct AppState {
    pub signaling: Arc<SignalingState>,
    pub relay: Arc<RelayState>,
    pub jwt_secret: String,
    // In-memory user store (use DB in production)
    pub users: Arc<dashmap::DashMap<String, UserRecord>>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String, // "admin" | "user"
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize)]
pub struct ApiResponse<T: serde::Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: serde::Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }
    pub fn err(msg: &str) -> Self {
        Self { success: false, data: None, error: Some(msg.to_string()) }
    }
}

impl AppState {
    pub fn new(signaling: Arc<SignalingState>, relay: Arc<RelayState>, jwt_secret: String) -> Self {
        let users = Arc::new(dashmap::DashMap::new());
        // Add default admin user: admin / admin123
        let hash = bcrypt::hash("admin123", bcrypt::DEFAULT_COST).unwrap();
        users.insert("admin".to_string(), UserRecord {
            id: uuid::Uuid::new_v4().to_string(),
            username: "admin".to_string(),
            password_hash: hash,
            role: "admin".to_string(),
            created_at: chrono::Utc::now(),
        });
        Self { signaling, relay, jwt_secret, users }
    }
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .configure(auth::configure)
            .configure(users::configure)
            .configure(connections::configure)
            .configure(file_transfer::configure)
    )
    .service(
        web::scope("/health")
            .route("", web::get().to(|| async { "OK" }))
    );
}
