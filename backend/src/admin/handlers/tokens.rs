use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use jsonwebtoken::{Algorithm, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{admin::auth::AdminAuth, auth::claims::AgentClaims, error::AppError, AppState};

#[derive(Deserialize)]
pub struct IssueTokenRequest {
    pub label: Option<String>,
    pub ttl_secs: Option<i64>,
    pub servers: Option<Vec<Uuid>>,
}

#[derive(Serialize)]
pub struct IssueTokenResponse {
    pub token: String,
    pub jti: String,
}

pub async fn issue(
    State(state): State<AppState>,
    _auth: AdminAuth,
    Path(agent_id): Path<Uuid>,
    Json(body): Json<IssueTokenRequest>,
) -> Result<(StatusCode, Json<IssueTokenResponse>), AppError> {
    // Verify agent exists and is active
    let exists = sqlx::query_as::<_, (bool,)>(
        "SELECT EXISTS(SELECT 1 FROM agents WHERE id = $1 AND is_active = true)",
    )
    .bind(agent_id)
    .fetch_one(&state.pool)
    .await?
    .0;

    if !exists {
        return Err(AppError::NotFound);
    }

    let token = state
        .issuer
        .issue_agent_token(
            agent_id,
            body.ttl_secs,
            body.label,
            body.servers.unwrap_or_default(),
        )
        .await?;

    // Decode the token (no signature verification needed here — we just issued it)
    // to extract the jti from the claims.
    let key_ring = state.key_ring.clone();
    let decoding_key = key_ring
        .active_key()
        .await
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no active signing key")))?
        .decoding_key;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[state.config.jwt_issuer.as_str()]);

    let claims = jsonwebtoken::decode::<AgentClaims>(&token, &decoding_key, &validation)
        .map(|d| d.claims)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to decode issued token: {e}")))?;

    Ok((
        StatusCode::CREATED,
        Json(IssueTokenResponse {
            token,
            jti: claims.jti,
        }),
    ))
}
