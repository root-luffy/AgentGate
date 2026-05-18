use dashmap::DashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

struct CacheEntry {
    allowed: bool,
    expires: Instant,
}

pub struct PolicyCache {
    inner: DashMap<String, CacheEntry>,
    ttl: Duration,
}

impl PolicyCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            inner: DashMap::new(),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    fn key(agent_id: Uuid, server_id: Uuid, tool: &str) -> String {
        format!("{agent_id}:{server_id}:{tool}")
    }

    pub fn get(&self, agent_id: Uuid, server_id: Uuid, tool: &str) -> Option<bool> {
        let k = Self::key(agent_id, server_id, tool);
        let entry = self.inner.get(&k)?;
        if entry.expires < Instant::now() {
            drop(entry);
            self.inner.remove(&k);
            return None;
        }
        Some(entry.allowed)
    }

    pub fn set(&self, agent_id: Uuid, server_id: Uuid, tool: &str, allowed: bool) {
        self.inner.insert(
            Self::key(agent_id, server_id, tool),
            CacheEntry {
                allowed,
                expires: Instant::now() + self.ttl,
            },
        );
    }

    pub fn invalidate_agent(&self, agent_id: Uuid) {
        let prefix = agent_id.to_string();
        self.inner.retain(|k, _| !k.starts_with(&prefix));
    }
}
