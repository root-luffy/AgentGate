use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::{
    auth::validator::ValidatedAgent,
    error::AppError,
    proxy::inspector,
    AppState,
};

pub async fn proxy_handler(
    State(state): State<AppState>,
    ValidatedAgent(claims): ValidatedAgent,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    if body.len() > state.config.max_request_body_bytes {
        return Err(AppError::BadRequest("request body too large".into()));
    }

    let inspected = inspector::inspect(&body)?;

    // Only enforce RBAC on tools/call
    if let Some(tool_name) = &inspected.tool_name {
        // Pick the target server from the agent's claims, fall back to first active server
        let server_id = state
            .find_server_for_agent(&claims)
            .await
            .ok_or(AppError::Forbidden)?;

        let allowed = state
            .policy_engine
            .is_allowed(claims.sub, server_id, tool_name)
            .await?;

        if !allowed {
            return Err(AppError::Forbidden);
        }

        let server_url = state
            .get_server_url(server_id)
            .await
            .ok_or(AppError::BadGateway)?;

        let result = state
            .forwarder
            .forward_json(&server_url, body, claims.sub)
            .await?;

        return Ok(Json(result).into_response());
    }

    // Non-tools/call methods: acknowledge with a null result
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "jsonrpc": "2.0",
            "result": null,
            "id": inspected.rpc.id
        })),
    )
        .into_response())
}
