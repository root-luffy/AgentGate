# AgentGate — Architecture & Implementation Plan

## Context

AgentGate is a security gateway that sits between AI agents (MCP clients) and upstream MCP servers. Without a gateway, any agent with a server URL gets unrestricted tool access, no audit trail, and no throttling. AgentGate adds: RS256 JWT authentication, RBAC-based tool access control stored in Postgres and managed via an admin UI, Redis token-bucket rate limiting, and an async dual-sink audit log (Postgres + Kafka). It acts as a transparent JSON-RPC reverse proxy over HTTP with SSE streaming.

**Stack decisions locked in:**
- Backend: Rust — axum 0.7, tower 0.4, tokio 1, jsonwebtoken 9 (RS256), deadpool-redis, sqlx 0.7, rdkafka 0.36, serde_json
- Frontend: Next.js 14 (App Router) + TypeScript + shadcn/ui + Tailwind CSS
- Deployment: Docker Compose (single node, local-first MVP)

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              EXTERNAL WORLD                                  │
│                                                                              │
│  ┌─────────────┐          ┌──────────────────────────────────────────────┐  │
│  │  AI Agent   │          │         Admin Dashboard (Next.js :3000)      │  │
│  │ (MCP client)│          │  Agents │ MCP Servers │ Permissions │ Audit  │  │
│  └──────┬──────┘          └───────────────────┬──────────────────────────┘  │
│         │ HTTP + SSE                           │ REST (admin JWT / API key)  │
│         │ Authorization: Bearer <JWT>          │                             │
└─────────┼───────────────────────────────────────────────────────────────────┘
          │                                      │
          ▼                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      AGENTGATE  (Single Rust Binary)                         │
│                                                                              │
│  ┌──────────────────────────────┐   ┌────────────────────────────────────┐  │
│  │  Proxy Port :8080             │   │       Admin Port :8081             │  │
│  │  Tower middleware stack:      │   │  axum Router:                      │  │
│  │  1. JWT validation            │   │  POST /api/auth/login              │  │
│  │  2. Rate limiter              │   │  POST /api/agents                  │  │
│  │  3. → handler                 │   │  POST /api/agents/{id}/tokens      │  │
│  │     a. RBAC check             │   │  PUT  /api/permissions             │  │
│  │     b. Forward to upstream    │   │  GET  /api/audit                   │  │
│  │     c. Audit log (async)      │   │  GET  /api/mcp-servers             │  │
│  └──────────────┬────────────────┘   └────────────────────────────────────┘  │
│                 │                                                              │
│  ┌──────────────▼──────────────────────────────────────────────────────────┐  │
│  │  Internal Modules: auth:: │ policy:: │ rate_limit:: │ audit:: │ proxy:: │  │
│  └──────────────┬────────────────────────────────────────────────────────┘  │
└─────────────────┼────────────────────────────────────────────────────────────┘
                  │
       ┌──────────┴────────────────────────────────────────┐
       │                         │                          │
  ┌────▼───────┐         ┌───────▼──────┐         ┌────────▼────────┐
  │ MCP Server │         │  Data Layer   │         │  MCP Server N   │
  │  :3001     │         │  Postgres     │         │  :300N          │
  │ (upstream) │         │  Redis        │         │  (upstream)     │
  └────────────┘         │  Kafka        │         └─────────────────┘
                         └───────────────┘
```

---

## 2. Request Lifecycle (tools/call)

| Step | Module | Action | Failure → Response |
|------|--------|---------|-------------------|
| 1 | `auth::validator` | Extract + verify RS256 JWT from `Authorization: Bearer` header; check `exp`, `iat`, `iss`; check `revoked:jti:{jti}` in Redis | `401 {"error":"unauthorized"}` |
| 2 | `rate_limit::limiter` | Atomic Redis Lua: decrement `rl:agent:{id}` and `rl:tool:{id}:{tool}` buckets | `429 {"error":"rate_limit_exceeded","retry_after":N}` |
| 3 | `proxy::inspector` | Buffer body (≤1 MB); parse as `JsonRpcRequest`; extract `params.name` if `method == "tools/call"` | `413` / `400` |
| 4 | `policy::engine` | Check `agent_tool_permissions` (DashMap cache → Postgres); verify `is_allowed = true` and not expired | `403 {"error":"tool_not_permitted"}` |
| 5 | `proxy::forwarder` | Forward request to upstream MCP server URL (reqwest); strip `Authorization` header; add `X-Forwarded-For`, `X-AgentGate-Agent-Id` | `502` / `504` |
| 6 | `audit::logger` | `tokio::spawn` fire-and-forget write to Postgres `audit_log` + Kafka `agentgate.audit` | Never fails the request; logs + metrics on sink failure |

**Failure matrix for infrastructure issues:**

| Dependency Down | Behavior |
|-----------------|----------|
| Redis | Rate limiting fails **open** (allow), alert fired, in-process fallback counter |
| Kafka | Audit to Postgres only, `kafka_audit_failures_total` incremented |
| Postgres (audit write) | Log error, request succeeds |
| Postgres (policy lookup) | `503` — cannot enforce policy safely |
| Upstream MCP server | `502 Bad Gateway` |

---

## 3. Data Models

### 3.1 Postgres Schema

```sql
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- RSA key pairs (AgentGate manages its own keys, private key AES-GCM encrypted)
CREATE TABLE signing_keys (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    kid         TEXT        NOT NULL UNIQUE,
    public_key  TEXT        NOT NULL,          -- PEM
    private_key TEXT        NOT NULL,          -- PEM, AES-256-GCM encrypted
    algorithm   TEXT        NOT NULL DEFAULT 'RS256',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    retired_at  TIMESTAMPTZ                    -- NULL = active
);

-- AI agents (each gets a JWT to authenticate with)
CREATE TABLE agents (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL,
    description TEXT,
    is_active   BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Upstream MCP servers registered in the gateway
CREATE TABLE mcp_servers (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name              TEXT        NOT NULL UNIQUE,
    base_url          TEXT        NOT NULL,    -- e.g. http://filesystem-mcp:3001
    description       TEXT,
    is_active         BOOLEAN     NOT NULL DEFAULT TRUE,
    timeout_ms        INT         NOT NULL DEFAULT 30000,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Tools discovered from each MCP server (via tools/list)
CREATE TABLE tools (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    mcp_server_id UUID        NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    description   TEXT,
    input_schema  JSONB,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (mcp_server_id, name)
);

-- RBAC: which tools each agent may call on each server
CREATE TABLE agent_tool_permissions (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id      UUID        NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    mcp_server_id UUID        NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    tool_name     TEXT        NOT NULL,        -- denormalized for fast lookup
    is_allowed    BOOLEAN     NOT NULL DEFAULT FALSE,
    expires_at    TIMESTAMPTZ,                 -- NULL = never
    created_by    TEXT        NOT NULL DEFAULT 'system',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (agent_id, mcp_server_id, tool_name)
);
CREATE INDEX idx_atp_lookup ON agent_tool_permissions(agent_id, mcp_server_id, tool_name);

-- Rate limit policies (per agent, optionally per tool)
CREATE TABLE rate_limit_policies (
    id                  UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id            UUID    REFERENCES agents(id) ON DELETE CASCADE,  -- NULL = default
    tool_name           TEXT,                  -- NULL = agent-global
    requests_per_minute INT     NOT NULL DEFAULT 60,
    burst_capacity      INT     NOT NULL DEFAULT 10,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Issued JWTs (for revocation and audit)
CREATE TABLE issued_tokens (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id   UUID        NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    jti        TEXT        NOT NULL UNIQUE,
    kid        TEXT        NOT NULL,
    issued_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revoked_by TEXT,
    label      TEXT
);
CREATE INDEX idx_tokens_jti     ON issued_tokens(jti);
CREATE INDEX idx_tokens_agent   ON issued_tokens(agent_id);

-- Audit log (append-only, partitioned by month in M6)
CREATE TABLE audit_log (
    id             BIGSERIAL   PRIMARY KEY,
    event_id       UUID        NOT NULL DEFAULT gen_random_uuid(),
    agent_id       UUID        REFERENCES agents(id),
    mcp_server_id  UUID        REFERENCES mcp_servers(id),
    tool_name      TEXT,
    method         TEXT        NOT NULL,
    request_params JSONB,                      -- sanitized
    outcome        TEXT        NOT NULL,
    http_status    INT,
    latency_ms     INT,
    error_message  TEXT,
    client_ip      TEXT,
    jti            TEXT,
    kafka_offset   BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT chk_outcome CHECK (outcome IN (
        'success','auth_failed','policy_denied',
        'rate_limited','upstream_error','upstream_timeout','internal_error'
    ))
);
CREATE INDEX idx_audit_agent   ON audit_log(agent_id, created_at DESC);
CREATE INDEX idx_audit_outcome ON audit_log(outcome, created_at DESC);
CREATE INDEX idx_audit_created ON audit_log(created_at DESC);

-- Admin users (human operators of the dashboard)
CREATE TABLE admin_users (
    id            UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    username      TEXT    NOT NULL UNIQUE,
    password_hash TEXT    NOT NULL,            -- argon2id
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login    TIMESTAMPTZ
);

-- Admin API keys (for CI/CD automation)
CREATE TABLE admin_api_keys (
    id          UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    admin_id    UUID    REFERENCES admin_users(id) ON DELETE CASCADE,
    key_hash    TEXT    NOT NULL UNIQUE,        -- SHA-256 of the raw key
    key_prefix  TEXT    NOT NULL,              -- first 8 chars, for display
    label       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used   TIMESTAMPTZ,
    expires_at  TIMESTAMPTZ
);
```

### 3.2 JWT Claims (Rust)

```rust
// src/auth/claims.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentClaims {
    pub iss: String,          // "agentgate"
    pub sub: Uuid,            // agent ID
    pub jti: String,          // unique token ID, used for revocation
    pub iat: i64,             // issued-at (Unix seconds)
    pub exp: i64,             // expiry (Unix seconds)
    pub servers: Vec<Uuid>,   // allowed MCP server IDs (empty = all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminClaims {
    pub iss: String,          // "agentgate-admin"
    pub sub: String,          // admin username
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
    pub role: AdminRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdminRole { Admin, ReadOnly }
```

### 3.3 Redis Key Conventions

```
# Token bucket (rate limiting)
rl:agent:{agent_id}               → String (remaining tokens)   TTL = window_secs
rl:tool:{agent_id}:{tool_sha16}   → String (remaining tokens)   TTL = window_secs
  where tool_sha16 = SHA-256(tool_name)[0..16]

# JWT revocation (fast path)
revoked:jti:{jti}                 → "1"                          TTL = remaining token lifetime (EXAT {exp})

# Upstream server health cache
health:{mcp_server_id}            → "up" | "down"               TTL = 30s

# Rate limit config shadow (avoids DB round-trip per request)
rlcfg:{agent_id}                  → Hash { rpm, burst, window } TTL = 300s
```

**Token bucket Lua script (executed atomically via EVAL):**
```lua
-- KEYS[1]=bucket, ARGV[1]=capacity, ARGV[2]=window_secs
local cur = redis.call('GET', KEYS[1])
if cur == false then
    redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[2])
    cur = tonumber(ARGV[1])
else
    cur = tonumber(cur)
end
if cur <= 0 then return {0, redis.call('TTL', KEYS[1])} end
redis.call('DECR', KEYS[1])
return {1, cur - 1}   -- {allowed, remaining}
```

### 3.4 Kafka Topic & Event Schema

```
Topic: agentgate.audit
  Partitions: 12   (partition key = agent_id → ordered per-agent reads)
  Retention:  7 days
  Replication: 1 (single-broker MVP)
```

Event JSON (produced on every tool call):
```json
{
  "schema_version": 1,
  "event_id": "018e4a2b-...",
  "event_type": "tool_call",
  "timestamp": "2025-01-15T10:30:00.123Z",
  "agent_id": "a1b2c3d4-...",
  "agent_name": "coding-assistant",
  "mcp_server_id": "e5f6a7b8-...",
  "mcp_server_name": "filesystem-server",
  "tool_name": "read_file",
  "method": "tools/call",
  "request_params": { "path": "/workspace/main.rs" },
  "outcome": "success",
  "http_status": 200,
  "latency_ms": 42,
  "error_message": null,
  "client_ip": "10.0.0.5",
  "jti": "tok_abc123"
}
```

---

## 4. Rust Module Breakdown

Single crate, clean module structure. No workspace for MVP.

```
backend/
├── Cargo.toml
├── migrations/
│   ├── 0001_initial.sql
│   └── 0002_...sql
└── src/
    ├── main.rs               startup, AppState, dual-port axum launch
    ├── config.rs             AppConfig loaded from env vars
    ├── error.rs              AppError enum + IntoResponse impl
    ├── db.rs                 PgPool init + migration runner
    ├── auth/
    │   ├── mod.rs
    │   ├── claims.rs         AgentClaims, AdminClaims, AdminRole
    │   ├── key_ring.rs       In-memory RSA key store, 5-min DB refresh
    │   ├── validator.rs      Tower Layer: JWT validation middleware
    │   └── issuer.rs         Token minting + revocation
    ├── policy/
    │   ├── mod.rs
    │   ├── engine.rs         RBAC evaluation (is_tool_allowed)
    │   ├── cache.rs          DashMap-backed in-process cache, TTL 60s
    │   └── models.rs         PolicyDecision, DenyReason
    ├── rate_limit/
    │   ├── mod.rs
    │   ├── limiter.rs        Tower Layer: token bucket middleware
    │   ├── lua.rs            Lua script constant
    │   └── models.rs         RateLimitPolicy, BucketConfig
    ├── audit/
    │   ├── mod.rs            AuditLogger (dual-sink, fire-and-forget)
    │   ├── event.rs          AuditEvent, AuditOutcome, AuditEventType
    │   ├── postgres_sink.rs  sqlx INSERT into audit_log
    │   └── kafka_sink.rs     rdkafka FutureProducer wrapper
    ├── proxy/
    │   ├── mod.rs
    │   ├── router.rs         axum Router for :8080, Tower stack
    │   ├── handler.rs        json_rpc_handler + sse_handler
    │   ├── inspector.rs      JsonRpcRequest parsing, tool name extraction
    │   └── forwarder.rs      reqwest client, forward_json_rpc + forward_sse
    └── admin/
        ├── mod.rs
        ├── router.rs         axum Router for :8081
        ├── auth.rs           Admin JWT/API key middleware
        └── handlers/
            ├── agents.rs
            ├── servers.rs
            ├── permissions.rs
            ├── tokens.rs
            ├── audit.rs
            └── rate_limits.rs
```

**Key public types and their module homes:**

| Type | Module | Used by |
|------|--------|---------|
| `AppState` | `main.rs` | all handlers |
| `AppConfig` | `config.rs` | `main.rs`, all modules |
| `AppError` | `error.rs` | all modules |
| `AgentClaims` | `auth::claims` | `auth::validator`, `proxy::handler`, `rate_limit::limiter` |
| `KeyRing` | `auth::key_ring` | `auth::validator`, `auth::issuer` |
| `PolicyEngine` | `policy::engine` | `proxy::handler` |
| `RateLimitLayer` | `rate_limit::limiter` | `proxy::router` |
| `AuditLogger` | `audit` | `proxy::handler`, `admin::handlers::*` |
| `AuditEvent` | `audit::event` | `audit` module, `proxy::handler` |

**`AppState` struct:**
```rust
#[derive(Clone)]
pub struct AppState {
    pub config:         Arc<AppConfig>,
    pub pool:           PgPool,
    pub redis:          deadpool_redis::Pool,
    pub key_ring:       Arc<KeyRing>,
    pub policy_engine:  Arc<PolicyEngine>,
    pub audit_logger:   Arc<AuditLogger>,
    pub forwarder:      Arc<Forwarder>,
}
```

**Admin REST API routes (port :8081):**
```
POST   /api/auth/login
POST   /api/auth/api-keys
DELETE /api/auth/api-keys/{id}

GET    /api/agents                   list
POST   /api/agents                   create
GET    /api/agents/{id}
PUT    /api/agents/{id}
DELETE /api/agents/{id}
POST   /api/agents/{id}/tokens       issue JWT
DELETE /api/agents/{id}/tokens/{jti} revoke

GET    /api/mcp-servers
POST   /api/mcp-servers
PUT    /api/mcp-servers/{id}
DELETE /api/mcp-servers/{id}
POST   /api/mcp-servers/{id}/discover-tools   trigger tools/list

GET    /api/permissions              list (filterable by agent, server)
PUT    /api/permissions              upsert
DELETE /api/permissions/{id}

GET    /api/rate-limits
POST   /api/rate-limits
PUT    /api/rate-limits/{id}
DELETE /api/rate-limits/{id}

GET    /api/audit                    paginated, filterable
GET    /api/audit/{event_id}

GET    /api/health
```

---

## 5. Frontend Architecture (Next.js + TypeScript)

**Stack:** Next.js 14 App Router · TypeScript · shadcn/ui · Tailwind CSS · TanStack React Query · TanStack Table

```
frontend/
├── src/
│   ├── app/
│   │   ├── layout.tsx                root layout + sidebar
│   │   ├── login/page.tsx
│   │   ├── dashboard/page.tsx        overview stats
│   │   ├── agents/
│   │   │   ├── page.tsx              agent list
│   │   │   ├── new/page.tsx
│   │   │   └── [id]/
│   │   │       ├── page.tsx          detail + issued tokens
│   │   │       └── permissions/page.tsx  tool permission matrix
│   │   ├── servers/
│   │   │   ├── page.tsx
│   │   │   └── [id]/page.tsx         server detail + tool discovery
│   │   ├── permissions/page.tsx      global permission matrix
│   │   ├── audit/page.tsx            searchable audit log
│   │   └── rate-limits/page.tsx
│   ├── lib/
│   │   ├── api/
│   │   │   ├── client.ts             base fetch wrapper, injects Bearer token
│   │   │   ├── agents.ts
│   │   │   ├── servers.ts
│   │   │   ├── permissions.ts
│   │   │   ├── audit.ts
│   │   │   └── rate-limits.ts
│   │   └── auth.ts                   admin session (cookie/localStorage)
│   ├── components/
│   │   ├── ui/                       shadcn re-exports
│   │   ├── layout/Sidebar.tsx
│   │   ├── permissions/
│   │   │   └── PermissionMatrix.tsx  TanStack Table grid, optimistic toggles
│   │   ├── audit/
│   │   │   ├── AuditTable.tsx        paginated, cursor-based
│   │   │   ├── AuditFilters.tsx      agent/server/outcome/date range
│   │   │   └── AuditDetail.tsx       drawer with full JSON
│   │   └── agents/
│   │       ├── AgentForm.tsx
│   │       └── TokenList.tsx
│   └── types/api.ts                  TypeScript interfaces mirroring Rust DTOs
```

**PermissionMatrix** is the most complex component: rows = tools (grouped by MCP server), columns = agents, cells = on/off toggles. Uses TanStack Table with sticky first column. Optimistic updates via React Query `useMutation` with rollback on error.

**Audit log** uses cursor-based pagination (`id` as cursor), URL-encoded filters for bookmarkable URLs, 30-second polling refresh.

**Admin auth:** POST `/api/auth/login` returns a short-lived admin JWT (8-hour TTL). Stored in `localStorage`, sent as `Authorization: Bearer` on all admin API calls. Login form on unauthenticated redirect.

---

## 6. Config Design

### Environment variables (`.env`)

```bash
# Required
DATABASE_URL=postgres://agentgate:secret@postgres:5432/agentgate
REDIS_URL=redis://redis:6379
KAFKA_BROKERS=kafka:9092
KEY_ENCRYPTION_SECRET=<32-byte hex string, AES-256-GCM for PEM storage>

# Ports
PROXY_PORT=8080
ADMIN_PORT=8081

# JWT
JWT_ISSUER=agentgate
JWT_DEFAULT_TTL_SECS=3600

# Admin bootstrap (only read when admin_users table is empty)
ADMIN_INITIAL_USERNAME=admin
ADMIN_INITIAL_PASSWORD=<min 16 chars>

# Timeouts and limits
UPSTREAM_CONNECT_TIMEOUT_MS=5000
UPSTREAM_REQUEST_TIMEOUT_MS=30000
MAX_REQUEST_BODY_BYTES=1048576
RATE_LIMIT_REDIS_TIMEOUT_MS=50   # fail-open if exceeded
KAFKA_PRODUCE_TIMEOUT_MS=5000

# Caching
POLICY_CACHE_TTL_SECS=60
KEY_RING_REFRESH_INTERVAL_SECS=300

# Observability
LOG_LEVEL=info
LOG_FORMAT=json
```

### What lives where

| Config item | Location | Notes |
|------------|----------|-------|
| DB/Redis/Kafka URLs | `.env` | Deployment secrets |
| JWT issuer, TTL | `.env` | Deployment-specific |
| RBAC permissions | Postgres | Source of truth, admin-editable via UI |
| Rate limit policies | Postgres | Source of truth, admin-editable via UI |
| MCP server definitions | Postgres | Source of truth |
| Agent definitions | Postgres | Source of truth |
| RSA signing keys | Postgres (PEM, AES-GCM encrypted) | Needs persistence + rotation |
| JWT revocation list | Redis (TTL-auto-expiring) | Fast O(1) lookup |
| Policy decisions | DashMap in-process (TTL 60s) | Performance cache, invalidated by admin writes |

### Docker Compose (MVP)

```yaml
services:
  agentgate:
    build: ./backend
    ports: ["8080:8080", "8081:8081"]
    env_file: .env
    depends_on:
      postgres: { condition: service_healthy }
      redis:    { condition: service_healthy }
      kafka:    { condition: service_healthy }

  frontend:
    build: ./frontend
    ports: ["3000:3000"]
    environment:
      NEXT_PUBLIC_ADMIN_API_URL: http://localhost:8081

  postgres:
    image: postgres:16-alpine
    environment: { POSTGRES_DB: agentgate, POSTGRES_USER: agentgate, POSTGRES_PASSWORD: secret }
    volumes: [pgdata:/var/lib/postgresql/data]
    healthcheck: { test: ["CMD","pg_isready","-U","agentgate"], interval: 5s, retries: 5 }

  redis:
    image: redis:7-alpine
    command: redis-server --appendonly yes
    volumes: [redisdata:/data]
    healthcheck: { test: ["CMD","redis-cli","ping"], interval: 5s }

  kafka:
    image: confluentinc/cp-kafka:7.6.0
    environment:
      KAFKA_NODE_ID: 1
      KAFKA_PROCESS_ROLES: broker,controller
      KAFKA_LISTENERS: PLAINTEXT://0.0.0.0:9092,CONTROLLER://0.0.0.0:9093
      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://kafka:9092
      KAFKA_CONTROLLER_QUORUM_VOTERS: 1@kafka:9093
      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 1
    healthcheck: { test: ["CMD","kafka-broker-api-versions","--bootstrap-server","localhost:9092"], interval: 10s, retries: 10 }

volumes: { pgdata: {}, redisdata: {} }
```

---

## 7. MVP → v1 Milestones

### M1 — Auth Skeleton (Week 1)
**Goal:** Binary starts, connects to Postgres, issues and validates RS256 JWTs.

Delivers:
- `config.rs`, `error.rs`, `db.rs`
- `auth/key_ring.rs`, `auth/claims.rs`, `auth/issuer.rs`, `auth/validator.rs`
- Admin API: `/api/auth/login`, `POST /api/agents`, `POST /api/agents/{id}/tokens`
- Postgres tables: `signing_keys`, `agents`, `issued_tokens`, `admin_users`
- sqlx migrations infrastructure
- Structured JSON tracing
- Docker Compose: agentgate + postgres only

**Acceptance:** `POST /api/agents` creates an agent. `POST /api/agents/{id}/tokens` returns a verifiable RS256 JWT. Proxy port returns `401` for requests without a valid token.

---

### M2 — Proxy Core + RBAC (Week 2)
**Goal:** Forward `tools/call` to real upstream MCP servers with access control.

Delivers:
- `proxy/router.rs`, `proxy/handler.rs`, `proxy/inspector.rs`, `proxy/forwarder.rs`
- `policy/engine.rs`, `policy/cache.rs`
- Admin API: `mcp-servers` CRUD, `permissions` CRUD, `discover-tools` endpoint
- Postgres tables: `mcp_servers`, `tools`, `agent_tool_permissions`
- Cache invalidation on permission writes
- Docker Compose adds: example MCP server container

**Acceptance:** Valid JWT can call an allowed tool and get a real response. Calling a disallowed tool returns `403`.

---

### M3 — Rate Limiting + Audit (Week 3)
**Goal:** All tool calls are rate-limited and both audit sinks are live.

Delivers:
- `rate_limit/limiter.rs`, `rate_limit/lua.rs`
- `audit/event.rs`, `audit/postgres_sink.rs`, `audit/kafka_sink.rs`
- Admin API: `/api/audit`, `/api/rate-limits` CRUD
- Postgres tables: `rate_limit_policies`, `audit_log`
- Redis Lua token bucket
- Kafka topic auto-creation at startup (rdkafka admin client)
- Docker Compose adds: redis, kafka (KRaft single-broker)

**Acceptance:** 100 requests in 10 seconds triggers `429`. Every request appears in `audit_log`. Kafka consumer on `agentgate.audit` receives events with correct partition key.

---

### M4 — SSE Streaming + Admin UI MVP (Week 4)
**Goal:** Full MCP SSE transport. Basic frontend is usable.

Delivers:
- `proxy/handler.rs` SSE handler (async stream forwarding via `tokio-stream`)
- JWT in query param (`?token=`) support for SSE clients that can't send headers
- Next.js frontend: login, agent management, server management, permission matrix, audit log
- Docker Compose adds: frontend service

**Acceptance:** An MCP client (Claude Desktop or test script) can open SSE channel through AgentGate to a real MCP server and execute tool calls end-to-end.

---

### M5 — Hardening + Observability (Week 5)
**Goal:** Production-ready robustness and monitoring.

Delivers:
- Prometheus metrics: request count, error count, latency histograms, rate limit hits, audit failures
- Graceful shutdown with in-flight request draining
- `X-Request-Id` header propagated end-to-end
- Admin API key support (alternative to session JWT)
- SSRF protection: block RFC-1918 + link-local upstream URLs
- Health check with dependency status
- Structured error codes (`"code"` field in all error responses)
- Grafana + Prometheus in Docker Compose with pre-built dashboard

**Acceptance:** Prometheus scrape returns all metrics. Health endpoint shows each dependency's status. Killing Redis shows rate-limit fail-open behavior and increments the alert counter.

---

### M6 — Audit Scale + v1 Polish (v1)
**Goal:** Scale audit log, improve multi-agent UX.

Delivers:
- `audit_log` partitioned by month (`PARTITION BY RANGE (created_at)`)
- Audit log CSV export (`GET /api/audit/export?format=csv`)
- Cursor-based pagination for all list endpoints
- Bulk permission management (set multiple tools at once)
- Permission expiry UI
- `docker-compose.prod.yml` with resource limits and restart policies

---

## 8. Open Questions (Decisions Needed Before Coding)

**Q1: SSE JWT delivery**
MCP SSE clients open `GET /sse` — HTTP GET can't have a body. Options:
- (a) `?token=<jwt>` query param — leaks token into access logs of any reverse proxy
- (b) `HttpOnly` cookie (`SameSite=None` for cross-origin)
- (c) Require `Authorization` header, document it for clients

Recommendation: (a) for MVP with short-TTL SSE-specific tokens (15 min). Decision needed before M4.

**Q2: Kafka failure policy**
If Kafka broker is down, should AgentGate fail-open (allow request, audit to Postgres only) or fail-closed (return `502`)? Recommendation: fail-open — Postgres is the authoritative audit store.

**Q3: Admin session TTL**
Current plan: admin session JWT = 8 hours, minimum password = 16 chars. Confirm these values.

**Q4: MCP spec version pinning**
Transparent proxy (inspect only `method` and `params.name`) vs. strict spec validation. Recommendation: transparent proxy to avoid coupling to spec churn.

**Q5: RSA key size**
2048-bit (fast, standard) vs. 4096-bit (slower, longer-lived). Recommendation: 2048-bit for MVP, key rotation built from day one.

**Q6: Tool discovery refresh**
Manual only (button in UI) vs. background re-scan every N minutes. Recommendation: manual only for MVP.

**Q7: Audit log retention**
Recommendation: 90 days in Postgres, enforced by scheduled DELETE or pg_cron.

**Q8: Kafka optional**
If `KAFKA_BROKERS` is unset, skip Kafka sink silently and rely on Postgres. Reduces local dev friction significantly.

---

## 9. Risks

### Security

| Risk | Severity | Mitigation |
|------|----------|-----------|
| Private key exposure (DB + env var both compromised) | Critical | Separate secrets; KMS adapter interface in v1 |
| JWT in SSE query param logged by reverse proxies | High | Short-TTL (15 min) SSE tokens; HTTPS enforced; log scrubbing |
| SSRF via malicious MCP server URL | High | Block RFC-1918 + link-local IPs in `forwarder.rs` before any HTTP call |
| Audit log tampering via DB access | Medium | Restrict agentgate DB user to INSERT-only on `audit_log`; Kafka as secondary record |
| Rate limit bypass via Redis outage | Medium | In-process `DashMap` sliding window as degraded fallback |

### Performance

| Risk | Severity | Mitigation |
|------|----------|-----------|
| 1000+ concurrent SSE connections (FD exhaustion) | High | `ulimit -n` in Docker; `reqwest` connection pool limits; idle SSE timeout |
| Unbounded audit write queue when Postgres is slow | Medium | Bounded `mpsc` channel; drop + increment counter if full |
| Policy cache stampede on startup | Medium | Singleflight pattern (`moka` cache `get_with`) to coalesce concurrent misses |

### Operational

| Risk | Severity | Mitigation |
|------|----------|-----------|
| Kafka ops overhead for local dev | High | Make Kafka optional via `KAFKA_BROKERS` env var |
| Schema migration breaking compile-time sqlx queries | Medium | Additive-only migrations in MVP; `cargo sqlx prepare` in CI |
| MCP spec churn breaking inspector | Low | Transparent proxy inspects only `method == "tools/call"` and `params.name` |

---

## 10. Critical Files (First to Implement)

1. [backend/migrations/0001_initial.sql](backend/migrations/0001_initial.sql) — all other modules depend on correct schema
2. [backend/src/config.rs](backend/src/config.rs) — loaded first in `main.rs`, required by every module
3. [backend/src/auth/key_ring.rs](backend/src/auth/key_ring.rs) + [auth/validator.rs](backend/src/auth/validator.rs) — everything behind the proxy wall depends on these
4. [backend/src/proxy/handler.rs](backend/src/proxy/handler.rs) — where RBAC check, rate limit, audit, and forwarder are composed
5. [backend/src/audit/kafka_sink.rs](backend/src/audit/kafka_sink.rs) — most complex dependency (rdkafka async producer + error handling)
6. [frontend/src/components/permissions/PermissionMatrix.tsx](frontend/src/components/permissions/PermissionMatrix.tsx) — most complex UI component

---

## Dependency Manifest (Cargo.toml)

```toml
[dependencies]
# Web
axum          = { version = "0.7", features = ["ws", "macros"] }
tower         = { version = "0.4", features = ["full"] }
tower-http    = { version = "0.5", features = ["trace", "cors", "request-id"] }
tokio         = { version = "1", features = ["full"] }

# Serialization
serde         = { version = "1", features = ["derive"] }
serde_json    = "1"

# Database
sqlx          = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "json", "migrate"] }

# Redis
deadpool-redis = "0.14"

# Kafka
rdkafka       = { version = "0.36", features = ["cmake-build"] }

# JWT
jsonwebtoken  = "9"

# Errors
thiserror     = "1"
anyhow        = "1"

# Utilities
uuid          = { version = "1", features = ["v4", "v7", "serde"] }
chrono        = { version = "0.4", features = ["serde"] }
dashmap       = "5"
reqwest       = { version = "0.12", features = ["json", "stream", "rustls-tls"] }
tokio-stream  = "0.1"
futures       = "0.3"
bytes         = "1"

# Observability
tracing                    = "0.1"
tracing-subscriber         = { version = "0.3", features = ["json", "env-filter"] }
metrics                    = "0.22"
metrics-exporter-prometheus = "0.13"

# Crypto
aes-gcm   = "0.10"
argon2    = "0.5"
rand      = "0.8"

# Config
dotenvy = "0.15"
```
