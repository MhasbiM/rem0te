use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::auth::require_admin;
use super::{ApiResponse, AppState, UserRecord};

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/users")
            .route("", web::get().to(list_users))
            .route("", web::post().to(create_user))
            .route("/{username}", web::delete().to(delete_user)),
    );
}

async fn list_users(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    if require_admin(&req, &state.jwt_secret).is_none() {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::err("Admin only"));
    }

    let users: Vec<serde_json::Value> = state
        .users
        .iter()
        .map(|u| {
            serde_json::json!({
                "id": u.id,
                "username": u.username,
                "role": u.role,
                "created_at": u.created_at.to_rfc3339(),
            })
        })
        .collect();

    HttpResponse::Ok().json(ApiResponse::ok(users))
}

async fn create_user(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<CreateUserRequest>,
) -> HttpResponse {
    if require_admin(&req, &state.jwt_secret).is_none() {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::err("Admin only"));
    }

    if state.users.contains_key(&body.username) {
        return HttpResponse::Conflict()
            .json(ApiResponse::<()>::err("User already exists"));
    }

    let hash = match bcrypt::hash(&body.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::err(&format!("Hash error: {e}")));
        }
    };

    let user = UserRecord {
        id: uuid::Uuid::new_v4().to_string(),
        username: body.username.clone(),
        password_hash: hash,
        role: body.role.clone().unwrap_or_else(|| "user".to_string()),
        created_at: chrono::Utc::now(),
    };

    state.users.insert(body.username.clone(), user);

    HttpResponse::Created().json(ApiResponse::ok(serde_json::json!({
        "username": body.username,
        "created": true,
    })))
}

async fn delete_user(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    if require_admin(&req, &state.jwt_secret).is_none() {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::err("Admin only"));
    }

    let username = path.into_inner();
    if username == "admin" {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::err("Cannot delete default admin"));
    }

    state.users.remove(&username);
    HttpResponse::Ok().json(ApiResponse::ok("deleted"))
}
