# AgentGate

A security gateway that sits between AI agents (MCP clients) and upstream MCP servers, providing JWT-based authentication, fine-grained tool-level RBAC, and a full admin API.

## Architecture

```
                        ┌─────────────────────────────────────────┐
                        │              AgentGate                   │
                        │                                          │
  AI Agent              │  :8080 (Proxy)         :8081 (Admin)    │
  (MCP Client)  ──JWT──►│  ┌──────────────┐    ┌──────────────┐  │
                        │  │ JWT Validator│    │ Admin API    │  │
                        │  │ RBAC Engine  │    │ Login        │  │
                        │  │ Forwarder    │    │ Agents CRUD  │  │
                        │  └──────┬───────┘    │ Servers CRUD │  │
                        │         │             │ Permissions  │  │
                        │         │             │ Token Issue  │  │
                        └─────────┼─────────────┴──────────────┘──┘
                                  │
                                  ▼
                        ┌──────────────────┐
                        │  Upstream MCP    │
                        │  Server(s)       │
                        └──────────────────┘

                        ┌──────────────────┐
                        │   PostgreSQL     │
                        │  (persistence)   │
                        └──────────────────┘
```

## Features

- **RS256 JWT Authentication** — asymmetric key pairs stored encrypted in Postgres; keys auto-generated on first boot
- **Tool-level RBAC** — every `tools/call` is checked against `agent_tool_permissions`; deny-by-default
- **In-memory policy cache** — 60-second TTL with instant invalidation on permission updates
- **Admin REST API** — full CRUD for agents, MCP servers, and permissions; issue scoped tokens per agent
- **Audit trail** — every issued token is recorded with JTI, expiry, and label
- **JSON-RPC inspection** — parses incoming MCP requests; only `tools/call` triggers RBAC, other methods pass through
- **Docker Compose** — single command brings up Postgres and AgentGate with auto-migration

## Quick Start

### 1. Copy and configure environment

```bash
cp .env.example .env
# Edit .env — set KEY_ENCRYPTION_SECRET and ADMIN_INITIAL_PASSWORD at minimum
# Generate a secret: openssl rand -hex 32
```

### 2. Start with Docker Compose

```bash
docker compose up --build
```

AgentGate will:
- Start the proxy on port **8080**
- Start the admin API on port **8081**
- Run database migrations automatically
- Create the initial admin user from `ADMIN_INITIAL_USERNAME` / `ADMIN_INITIAL_PASSWORD`

### 3. Login and get an admin token

```bash
curl -s -X POST http://localhost:8081/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"changeme_min16chars!!"}' | jq .
```

### 4. Create an MCP server

```bash
TOKEN="<admin token from step 3>"

curl -s -X POST http://localhost:8081/api/mcp-servers \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"my-mcp","base_url":"http://my-mcp-server:3000"}' | jq .
```

### 5. Create an agent

```bash
curl -s -X POST http://localhost:8081/api/agents \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"my-agent","description":"Test agent"}' | jq .
```

### 6. Grant a tool permission

```bash
curl -s -X PUT http://localhost:8081/api/permissions \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "agent_id": "<agent-uuid>",
    "mcp_server_id": "<server-uuid>",
    "tool_name": "read_file",
    "is_allowed": true
  }' | jq .
```

### 7. Issue an agent token

```bash
curl -s -X POST http://localhost:8081/api/agents/<agent-uuid>/tokens \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"label":"ci-run-1","ttl_secs":3600,"servers":["<server-uuid>"]}' | jq .
```

### 8. Use the proxy

```bash
AGENT_TOKEN="<token from step 7>"

curl -s -X POST http://localhost:8080/mcp \
  -H "Authorization: Bearer $AGENT_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {"name": "read_file", "arguments": {"path": "/etc/hostname"}}
  }' | jq .
```

## API Reference

### Admin API (port 8081)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/auth/login` | Obtain admin JWT |
| GET | `/api/agents` | List all agents |
| POST | `/api/agents` | Create agent |
| GET | `/api/agents/:id` | Get agent by ID |
| DELETE | `/api/agents/:id` | Delete agent |
| POST | `/api/agents/:id/tokens` | Issue agent token |
| GET | `/api/mcp-servers` | List MCP servers |
| POST | `/api/mcp-servers` | Register MCP server |
| DELETE | `/api/mcp-servers/:id` | Remove MCP server |
| GET | `/api/permissions` | List all permissions |
| PUT | `/api/permissions` | Create or update permission |
| DELETE | `/api/permissions/:id` | Delete permission |
| GET | `/api/health` | Health check |

### Proxy API (port 8080)

All routes: `ANY /*path`

Requires `Authorization: Bearer <agent-jwt>` header.

- For `tools/call` method: validates JWT, checks RBAC, forwards to upstream MCP server
- For all other methods: validates JWT and returns a null result passthrough

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | — | Postgres connection string |
| `KEY_ENCRYPTION_SECRET` | Yes | — | 64-char hex (32 bytes) for AES-256-GCM key encryption |
| `ADMIN_INITIAL_PASSWORD` | Yes | — | Password for the bootstrap admin account |
| `PROXY_PORT` | No | `8080` | Port for the MCP proxy |
| `ADMIN_PORT` | No | `8081` | Port for the admin API |
| `JWT_ISSUER` | No | `agentgate` | JWT `iss` claim |
| `JWT_DEFAULT_TTL_SECS` | No | `3600` | Default agent token lifetime |
| `ADMIN_INITIAL_USERNAME` | No | `admin` | Username for the bootstrap admin account |
| `UPSTREAM_REQUEST_TIMEOUT_MS` | No | `30000` | Timeout for upstream MCP calls |
| `MAX_REQUEST_BODY_BYTES` | No | `1048576` | Maximum proxy request body size (1 MiB) |

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust 2021 edition |
| HTTP framework | Axum 0.7 |
| Async runtime | Tokio |
| Database | PostgreSQL 16 via sqlx 0.7 |
| Migrations | sqlx built-in migrations |
| Authentication | RS256 JWT via `jsonwebtoken` |
| Key storage | AES-256-GCM encrypted via `aes-gcm` |
| RSA key gen | `rsa` crate (PKCS#8, 2048-bit) |
| Password hashing | Argon2id via `argon2` |
| Policy cache | In-process `DashMap` with TTL |
| Upstream HTTP | `reqwest` with rustls |
| Logging | `tracing` + JSON output |
| Containerization | Docker + Docker Compose |

## Development

### Running locally (without Docker)

```bash
# Start Postgres
docker compose up postgres -d

# Copy and edit env
cp .env.example .env
# Set DATABASE_URL=postgres://agentgate:secret@localhost:5432/agentgate

cd backend
cargo run
```

### Generating a KEY_ENCRYPTION_SECRET

```bash
openssl rand -hex 32
```
