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
pub struct AgentResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub description: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    _auth: AdminAuth,
) -> Result<Json<Vec<AgentResponse>>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, bool, DateTime<Utc>)>(
        "SELECT id, name, description, is_active, created_at \
         FROM agents ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, description, is_active, created_at)| AgentResponse {
                id,
                name,
                description,
                is_active,
                created_at,
            })
            .collect(),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    _auth: AdminAuth,
    Json(body): Json<CreateAgentRequest>,
) -> Result<(StatusCode, Json<AgentResponse>), AppError> {
    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, bool, DateTime<Utc>)>(
        "INSERT INTO agents (name, description) VALUES ($1, $2) \
         RETURNING id, name, description, is_active, created_at",
    )
    .bind(&body.name)
    .bind(&body.description)
    .fetch_one(&state.pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(AgentResponse {
            id: row.0,
            name: row.1,
            description: row.2,
            is_active: row.3,
            created_at: row.4,
        }),
    ))
}

pub async fn get(
    State(state): State<AppState>,
    _auth: AdminAuth,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentResponse>, AppError> {
    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, bool, DateTime<Utc>)>(
        "SELECT id, name, description, is_active, created_at FROM agents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(AgentResponse {
        id: row.0,
        name: row.1,
        description: row.2,
        is_active: row.3,
        created_at: row.4,
    }))
}

pub async fn delete(
    State(state): State<AppState>,
    _auth: AdminAuth,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM agents WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
