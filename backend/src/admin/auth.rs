use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use jsonwebtoken::{Algorithm, Validation};
use std::sync::Arc;

use crate::{
    auth::{claims::AdminClaims, key_ring::KeyRing, validator::extract_bearer},
    error::AppError,
};

pub struct AdminAuth(pub AdminClaims);

#[async_trait]
impl<S> FromRequestParts<S> for AdminAuth
where
    S: Send + Sync,
    Arc<KeyRing>: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key_ring = Arc::<KeyRing>::from_ref(state);
        let token = extract_bearer(&parts.headers)?;

        let header =
            jsonwebtoken::decode_header(&token).map_err(|_| AppError::Unauthorized)?;
        let kid = header.kid.as_deref().unwrap_or("default");
        let decoding_key = key_ring
            .decoding_key_for(kid)
            .await
            .ok_or(AppError::Unauthorized)?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&["agentgate-admin"]);

        let claims = jsonwebtoken::decode::<AdminClaims>(&token, &decoding_key, &validation)
            .map(|d| d.claims)
            .map_err(|_| AppError::Unauthorized)?;

        Ok(AdminAuth(claims))
    }
}
