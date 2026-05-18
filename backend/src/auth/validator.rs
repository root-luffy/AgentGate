use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use std::sync::Arc;

use crate::auth::claims::AgentClaims;
use crate::auth::key_ring::KeyRing;
use crate::error::AppError;

pub struct ValidatedAgent(pub AgentClaims);

#[async_trait]
impl<S> FromRequestParts<S> for ValidatedAgent
where
    S: Send + Sync,
    Arc<KeyRing>: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key_ring = Arc::<KeyRing>::from_ref(state);
        let token = extract_bearer(&parts.headers)?;
        let claims = validate_agent_jwt(&token, &key_ring).await?;
        Ok(ValidatedAgent(claims))
    }
}

pub fn extract_bearer(headers: &axum::http::HeaderMap) -> Result<String, AppError> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    auth.strip_prefix("Bearer ")
        .map(|t| t.to_string())
        .ok_or(AppError::Unauthorized)
}

pub async fn validate_agent_jwt(
    token: &str,
    key_ring: &Arc<KeyRing>,
) -> Result<AgentClaims, AppError> {
    let header =
        jsonwebtoken::decode_header(token).map_err(|_| AppError::Unauthorized)?;

    let kid = header.kid.as_deref().unwrap_or("default");
    let decoding_key = key_ring
        .decoding_key_for(kid)
        .await
        .ok_or(AppError::Unauthorized)?;

    validate_with_key(token, &decoding_key)
}

pub fn validate_with_key(token: &str, key: &DecodingKey) -> Result<AgentClaims, AppError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["agentgate"]);

    jsonwebtoken::decode::<AgentClaims>(token, key, &validation)
        .map(|data| data.claims)
        .map_err(|_| AppError::Unauthorized)
}
