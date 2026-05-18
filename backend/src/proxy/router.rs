use axum::{routing::any, Router};

use crate::{proxy::handler::proxy_handler, AppState};

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/*path", any(proxy_handler))
        .with_state(state)
}
