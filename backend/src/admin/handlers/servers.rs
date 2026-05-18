use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{admin::auth::AdminAuth, error::AppError, AppState};

#[derive(Serialize)]
pub struct ServerResponse {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateServerRequest {
    pub name: String,
    pub base_url: String,
    pub description: Option<String>,
    pub timeout_ms: Option<i32>,
}

pub async fn list(
    State(state): State<AppState>,
    _auth: AdminAuth,
) -> Result<Json<Vec<ServerResponse>>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>, bool, DateTime<Utc>)>(
        "SELECT id, name, base_url, description, is_active, created_at \
         FROM mcp_servers ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, name, base_url, description, is_active, created_at)| ServerResponse {
                    id,
                    name,
                    base_url,
                    description,
                    is_active,
                    created_at,
                },
            )
            .collect(),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    _auth: AdminAuth,
    Json(body): Json<CreateServerRequest>,
) -> Result<(StatusCode, Json<ServerResponse>), AppError> {
    let row = sqlx::query_as::<_, (Uuid, String, String, Option<String>, bool, DateTime<Utc>)>(
        "INSERT INTO mcp_servers (name, base_url, description, timeout_ms) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, name, base_url, description, is_active, created_at",
    )
    .bind(&body.name)
    .bind(&body.base_url)
    .bind(&body.description)
    .bind(body.timeout_ms.unwrap_or(30000))
    .fetch_one(&state.pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ServerResponse {
            id: row.0,
            name: row.1,
            base_url: row.2,
            description: row.3,
            is_active: row.4,
            created_at: row.5,
        }),
    ))
}

pub async fn delete(
    State(state): State<AppState>,
    _auth: AdminAuth,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
