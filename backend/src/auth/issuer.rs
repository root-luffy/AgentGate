use chrono::Utc;
use jsonwebtoken::{Algorithm, Header};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{
    claims::{AdminClaims, AdminRole, AgentClaims},
    key_ring::KeyRing,
};
use crate::error::AppError;

pub struct TokenIssuer {
    pub key_ring: Arc<KeyRing>,
    pub pool: PgPool,
    pub jwt_issuer: String,
    pub jwt_default_ttl_secs: i64,
}

impl TokenIssuer {
    pub async fn issue_agent_token(
        &self,
        agent_id: Uuid,
        ttl_secs: Option<i64>,
        label: Option<String>,
        servers: Vec<Uuid>,
    ) -> Result<String, AppError> {
        let key = self
            .key_ring
            .active_key()
            .await
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no active signing key")))?;

        let now = Utc::now().timestamp();
        let ttl = ttl_secs.unwrap_or(self.jwt_default_ttl_secs);
        let jti = format!("tok_{}", Uuid::new_v4().simple());

        let claims = AgentClaims {
            iss: self.jwt_issuer.clone(),
            sub: agent_id,
            jti: jti.clone(),
            iat: now,
            exp: now + ttl,
            servers,
            label: label.clone(),
        };

        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());

        let token = jsonwebtoken::encode(&header, &claims, &key.encoding_key)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("token encode failed: {e}")))?;

        let expires_at = chrono::DateTime::from_timestamp(now + ttl, 0)
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("invalid timestamp")))?;

        sqlx::query(
            "INSERT INTO issued_tokens (agent_id, jti, kid, expires_at, label) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(agent_id)
        .bind(&jti)
        .bind(&key.kid)
        .bind(expires_at)
        .bind(&label)
        .execute(&self.pool)
        .await?;

        Ok(token)
    }

    pub async fn issue_admin_token(
        &self,
        username: &str,
        role: AdminRole,
    ) -> Result<String, AppError> {
        let key = self
            .key_ring
            .active_key()
            .await
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no active signing key")))?;

        let now = Utc::now().timestamp();
        let jti = format!("adm_{}", Uuid::new_v4().simple());

        let claims = AdminClaims {
            iss: format!("{}-admin", self.jwt_issuer),
            sub: username.to_string(),
            jti,
            iat: now,
            exp: now + 8 * 3600,
            role,
        };

        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());

        let token = jsonwebtoken::encode(&header, &claims, &key.encoding_key)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("token encode failed: {e}")))?;

        Ok(token)
    }
}
