use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::policy::cache::PolicyCache;

pub struct PolicyEngine {
    pool: PgPool,
    cache: Arc<PolicyCache>,
}

impl PolicyEngine {
    pub fn new(pool: PgPool) -> Arc<Self> {
        Arc::new(Self {
            pool,
            cache: Arc::new(PolicyCache::new(60)),
        })
    }

    pub async fn is_allowed(
        &self,
        agent_id: Uuid,
        server_id: Uuid,
        tool_name: &str,
    ) -> Result<bool, AppError> {
        if let Some(cached) = self.cache.get(agent_id, server_id, tool_name) {
            return Ok(cached);
        }

        let row = sqlx::query_as::<_, (bool,)>(
            "SELECT is_allowed FROM agent_tool_permissions
             WHERE agent_id = $1
               AND mcp_server_id = $2
               AND tool_name = $3
               AND (expires_at IS NULL OR expires_at > now())",
        )
        .bind(agent_id)
        .bind(server_id)
        .bind(tool_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("policy lookup failed: {e}")))?;

        let allowed = row.map(|(v,)| v).unwrap_or(false);
        self.cache.set(agent_id, server_id, tool_name, allowed);
        Ok(allowed)
    }

    pub fn invalidate_agent(&self, agent_id: Uuid) {
        self.cache.invalidate_agent(agent_id);
    }
}
