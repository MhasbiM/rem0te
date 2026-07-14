use actix_web::{web, HttpRequest, HttpResponse};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use super::{ApiResponse, AppState};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,       // username
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub username: String,
    pub role: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/auth/login", web::post().to(login))
        .route("/auth/me", web::get().to(me));
}

async fn login(
    state: web::Data<AppState>,
    body: web::Json<LoginRequest>,
) -> HttpResponse {
    let user = match state.users.get(&body.username) {
        Some(u) => u.clone(),
        None => {
            return HttpResponse::Unauthorized()
                .json(ApiResponse::<()>::err("Invalid credentials"));
        }
    };

    match bcrypt::verify(&body.password, &user.password_hash) {
        Ok(true) => {}
        _ => {
            return HttpResponse::Unauthorized()
                .json(ApiResponse::<()>::err("Invalid credentials"));
        }
    }

    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user.username.clone(),
        role: user.role.clone(),
        exp: now + 86400 * 7, // 7 days
        iat: now,
    };

    match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ) {
        Ok(token) => HttpResponse::Ok().json(ApiResponse::ok(LoginResponse {
            token,
            username: user.username,
            role: user.role,
        })),
        Err(e) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::err(&format!("Token generation failed: {e}"))),
    }
}

async fn me(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let claims = match extract_claims(&req, &state.jwt_secret) {
        Some(c) => c,
        None => return HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Unauthorized")),
    };
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "username": claims.sub,
        "role": claims.role,
    })))
}

pub fn extract_claims(req: &HttpRequest, secret: &str) -> Option<Claims> {
    let auth_header = req.headers().get("Authorization")?;
    let auth_str = auth_header.to_str().ok()?;
    let token = auth_str.strip_prefix("Bearer ")?;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|data| data.claims)
}

pub fn require_admin(req: &HttpRequest, secret: &str) -> Option<Claims> {
    let claims = extract_claims(req, secret)?;
    if claims.role == "admin" {
        Some(claims)
    } else {
        None
    }
}
