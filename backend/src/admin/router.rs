use axum::{
    http::{header, Method},
    routing::{delete, get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::admin::handlers;
use crate::AppState;

pub fn build(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    Router::new()
        // Auth
        .route("/api/auth/login", post(handlers::auth::login))
        // Agents
        .route(
            "/api/agents",
            get(handlers::agents::list).post(handlers::agents::create),
        )
        .route(
            "/api/agents/:id",
            get(handlers::agents::get).delete(handlers::agents::delete),
        )
        .route("/api/agents/:id/tokens", post(handlers::tokens::issue))
        // MCP Servers
        .route(
            "/api/mcp-servers",
            get(handlers::servers::list).post(handlers::servers::create),
        )
        .route("/api/mcp-servers/:id", delete(handlers::servers::delete))
        // Permissions
        .route(
            "/api/permissions",
            get(handlers::permissions::list).put(handlers::permissions::upsert),
        )
        .route(
            "/api/permissions/:id",
            delete(handlers::permissions::delete_permission),
        )
        // Health
        .route(
            "/api/health",
            get(|| async { axum::Json(serde_json::json!({ "status": "ok" })) }),
        )
        .with_state(state)
        .layer(cors)
}
