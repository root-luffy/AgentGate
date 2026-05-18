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
pub struct PermissionResponse {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub mcp_server_id: Uuid,
    pub tool_name: String,
    pub is_allowed: bool,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct UpsertPermissionRequest {
    pub agent_id: Uuid,
    pub mcp_server_id: Uuid,
    pub tool_name: String,
    pub is_allowed: bool,
    pub expires_at: Option<DateTime<Utc>>,
}

pub async fn list(
    State(state): State<AppState>,
    _auth: AdminAuth,
) -> Result<Json<Vec<PermissionResponse>>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, bool, Option<DateTime<Utc>>)>(
        "SELECT id, agent_id, mcp_server_id, tool_name, is_allowed, expires_at \
         FROM agent_tool_permissions ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, agent_id, mcp_server_id, tool_name, is_allowed, expires_at)| {
                    PermissionResponse {
                        id,
                        agent_id,
                        mcp_server_id,
                        tool_name,
                        is_allowed,
                        expires_at,
                    }
                },
            )
            .collect(),
    ))
}

pub async fn upsert(
    State(state): State<AppState>,
    _auth: AdminAuth,
    Json(body): Json<UpsertPermissionRequest>,
) -> Result<(StatusCode, Json<PermissionResponse>), AppError> {
    let row = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, bool, Option<DateTime<Utc>>)>(
        "INSERT INTO agent_tool_permissions \
             (agent_id, mcp_server_id, tool_name, is_allowed, expires_at) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (agent_id, mcp_server_id, tool_name) \
         DO UPDATE SET \
             is_allowed = EXCLUDED.is_allowed, \
             expires_at = EXCLUDED.expires_at, \
             updated_at = now() \
         RETURNING id, agent_id, mcp_server_id, tool_name, is_allowed, expires_at",
    )
    .bind(body.agent_id)
    .bind(body.mcp_server_id)
    .bind(&body.tool_name)
    .bind(body.is_allowed)
    .bind(body.expires_at)
    .fetch_one(&state.pool)
    .await?;

    state.policy_engine.invalidate_agent(body.agent_id);

    Ok((
        StatusCode::OK,
        Json(PermissionResponse {
            id: row.0,
            agent_id: row.1,
            mcp_server_id: row.2,
            tool_name: row.3,
            is_allowed: row.4,
            expires_at: row.5,
        }),
    ))
}

pub async fn delete_permission(
    State(state): State<AppState>,
    _auth: AdminAuth,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM agent_tool_permissions WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
