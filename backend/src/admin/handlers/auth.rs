use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::{
    auth::claims::AdminRole,
    error::AppError,
    AppState,
};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT password_hash FROM admin_users WHERE username = $1",
    )
    .bind(&body.username)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let hash = row.0;
    let parsed_hash =
        PasswordHash::new(&hash).map_err(|_| AppError::Unauthorized)?;
    Argon2::default()
        .verify_password(body.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized)?;

    sqlx::query("UPDATE admin_users SET last_login = now() WHERE username = $1")
        .bind(&body.username)
        .execute(&state.pool)
        .await?;

    let token = state
        .issuer
        .issue_admin_token(&body.username, AdminRole::Admin)
        .await?;

    Ok(Json(LoginResponse { token }))
}
