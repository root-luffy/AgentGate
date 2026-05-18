use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub proxy_port: u16,
    pub admin_port: u16,
    pub jwt_issuer: String,
    pub jwt_default_ttl_secs: i64,
    pub key_encryption_secret: String,
    pub admin_initial_username: String,
    pub admin_initial_password: String,
    pub upstream_request_timeout_ms: u64,
    pub max_request_body_bytes: usize,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: require("DATABASE_URL")?,
            proxy_port: env::var("PROXY_PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()?,
            admin_port: env::var("ADMIN_PORT")
                .unwrap_or_else(|_| "8081".into())
                .parse()?,
            jwt_issuer: env::var("JWT_ISSUER").unwrap_or_else(|_| "agentgate".into()),
            jwt_default_ttl_secs: env::var("JWT_DEFAULT_TTL_SECS")
                .unwrap_or_else(|_| "3600".into())
                .parse()?,
            key_encryption_secret: require("KEY_ENCRYPTION_SECRET")?,
            admin_initial_username: env::var("ADMIN_INITIAL_USERNAME")
                .unwrap_or_else(|_| "admin".into()),
            admin_initial_password: require("ADMIN_INITIAL_PASSWORD")?,
            upstream_request_timeout_ms: env::var("UPSTREAM_REQUEST_TIMEOUT_MS")
                .unwrap_or_else(|_| "30000".into())
                .parse()?,
            max_request_body_bytes: env::var("MAX_REQUEST_BODY_BYTES")
                .unwrap_or_else(|_| "1048576".into())
                .parse()?,
        })
    }
}

fn require(key: &str) -> anyhow::Result<String> {
    env::var(key).map_err(|_| anyhow::anyhow!("missing required env var: {key}"))
}
