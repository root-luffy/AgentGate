use std::sync::Arc;
use tracing::info;

mod admin;
mod auth;
mod config;
mod db;
mod error;
mod policy;
mod proxy;

use auth::{issuer::TokenIssuer, key_ring::KeyRing};
use policy::engine::PolicyEngine;
use proxy::forwarder::Forwarder;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<config::AppConfig>,
    pub pool: sqlx::PgPool,
    pub key_ring: Arc<KeyRing>,
    pub issuer: Arc<TokenIssuer>,
    pub policy_engine: Arc<PolicyEngine>,
    pub forwarder: Arc<Forwarder>,
}

impl axum::extract::FromRef<AppState> for Arc<KeyRing> {
    fn from_ref(state: &AppState) -> Self {
        state.key_ring.clone()
    }
}

impl AppState {
    pub async fn find_server_for_agent(
        &self,
        claims: &auth::claims::AgentClaims,
    ) -> Option<uuid::Uuid> {
        if !claims.servers.is_empty() {
            return Some(claims.servers[0]);
        }
        // Fall back to first active server
        sqlx::query_as::<_, (uuid::Uuid,)>(
            "SELECT id FROM mcp_servers WHERE is_active = true ORDER BY created_at LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .map(|(id,)| id)
    }

    pub async fn get_server_url(&self, server_id: uuid::Uuid) -> Option<String> {
        sqlx::query_as::<_, (String,)>(
            "SELECT base_url FROM mcp_servers WHERE id = $1 AND is_active = true",
        )
        .bind(server_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .map(|(url,)| url)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    info!("starting agentgate");

    let config = Arc::new(config::AppConfig::from_env()?);

    let pool = db::init_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    bootstrap_admin(&pool, &config).await?;

    let key_ring = KeyRing::new(pool.clone(), &config.key_encryption_secret)?;
    key_ring.load_or_generate().await?;

    let issuer = Arc::new(TokenIssuer {
        key_ring: key_ring.clone(),
        pool: pool.clone(),
        jwt_issuer: config.jwt_issuer.clone(),
        jwt_default_ttl_secs: config.jwt_default_ttl_secs,
    });

    let policy_engine = PolicyEngine::new(pool.clone());
    let forwarder = Arc::new(Forwarder::new(config.upstream_request_timeout_ms));

    let state = AppState {
        config: config.clone(),
        pool,
        key_ring,
        issuer,
        policy_engine,
        forwarder,
    };

    let admin_app = admin::router::build(state.clone());
    let proxy_app = proxy::router::build(state.clone());

    let admin_addr = format!("0.0.0.0:{}", config.admin_port);
    let proxy_addr = format!("0.0.0.0:{}", config.proxy_port);

    info!(admin_addr = %admin_addr, proxy_addr = %proxy_addr, "listening");

    let admin_listener = tokio::net::TcpListener::bind(&admin_addr).await?;
    let proxy_listener = tokio::net::TcpListener::bind(&proxy_addr).await?;

    tokio::try_join!(
        axum::serve(admin_listener, admin_app),
        axum::serve(proxy_listener, proxy_app),
    )?;

    Ok(())
}

async fn bootstrap_admin(
    pool: &sqlx::PgPool,
    config: &config::AppConfig,
) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_users")
        .fetch_one(pool)
        .await?;

    if count == 0 {
        let hash = hash_password(&config.admin_initial_password)?;
        sqlx::query("INSERT INTO admin_users (username, password_hash) VALUES ($1, $2)")
            .bind(&config.admin_initial_username)
            .bind(&hash)
            .execute(pool)
            .await?;
        info!(username = %config.admin_initial_username, "created initial admin user");
    }
    Ok(())
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
        Argon2,
    };
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash failed: {e}"))?
        .to_string();
    Ok(hash)
}
