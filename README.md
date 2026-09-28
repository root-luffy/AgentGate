<p align="center">
  <img src="assets/banner.svg" alt="AgentGate: the security checkpoint between AI agents and their MCP tools" width="100%">
</p>

<p align="center">
  <b>Give every AI agent its own key, and decide exactly which MCP tools it may call.</b><br>
  AgentGate is a self-hosted gateway that sits in front of your MCP servers. It checks a signed token and a<br>
  per-tool permission on every <code>tools/call</code>, and refuses anything you didn't explicitly allow.
</p>

<p align="center">
  <a href="backend/Cargo.toml"><img alt="Rust 2021 + Axum 0.7" src="https://img.shields.io/badge/Rust-Axum%200.7-ce422b?logo=rust&logoColor=white"></a>
  <a href="frontend/package.json"><img alt="Next.js 14" src="https://img.shields.io/badge/Next.js-14-111111?logo=nextdotjs&logoColor=white"></a>
  <a href="docker-compose.yml"><img alt="PostgreSQL 16" src="https://img.shields.io/badge/PostgreSQL-16-336791?logo=postgresql&logoColor=white"></a>
  <a href="https://modelcontextprotocol.io"><img alt="Model Context Protocol" src="https://img.shields.io/badge/protocol-MCP-22d3ee"></a>
  <a href="#quick-start"><img alt="Run with Docker Compose" src="https://img.shields.io/badge/run-docker%20compose-2496ED?logo=docker&logoColor=white"></a>
</p>

<p align="center">
  <a href="#quick-start">Quick start</a> ·
  <a href="#how-it-works">How it works</a> ·
  <a href="#api-reference">API</a> ·
  <a href="#configuration">Configuration</a> ·
  <a href="#roadmap">Roadmap</a>
</p>

---

## Why AgentGate

Point an agent at an MCP server and it can usually call **every** tool that server exposes, with no
identity and no way to say "this agent may read files but never delete them."

AgentGate puts a checkpoint in between:

- **One token per agent.** Agents authenticate with short-lived RS256 JWTs that AgentGate signs itself.
- **Tool-level RBAC.** Permissions are rows of *agent × server × tool*. No row means **deny**.
- **Expiring grants.** A permission can carry an `expires_at` and stops working on its own.
- **Fast checks.** Decisions are cached in memory for 60 s and cleared the moment you change that agent's permissions.
- **Keys at rest are encrypted.** The RSA-2048 signing key is generated on first boot and stored in Postgres, AES-256-GCM encrypted.
- **Admin UI + REST API.** Manage agents, servers and permissions, and issue tokens, from a Next.js dashboard or `curl`.

## How it works

<p align="center">
  <img src="assets/architecture.svg" alt="Architecture: AI agent calls the proxy on :8080 (verify JWT, check RBAC, forward) which reaches upstream MCP servers; the Admin UI on :3000 calls the Admin API on :8081; a shared KeyRing and policy cache sit in between, backed by PostgreSQL" width="100%">
</p>

AgentGate is a single Rust binary that listens on two ports:

| Port | Who talks to it | What it does |
| --- | --- | --- |
| **8080** proxy | AI agents (MCP clients) | Verifies the agent JWT, inspects the JSON-RPC body, enforces RBAC on `tools/call`, forwards allowed calls upstream. |
| **8081** admin | The dashboard, your scripts | Login, CRUD for agents, MCP servers and permissions, token issuing, health check. |

Every proxied request walks the same checklist, and any step can stop it:

<p align="center">
  <img src="assets/request-flow.svg" alt="Request lifecycle: verify JWT (401), inspect body (400), tools/call? (other methods get 200 with a null result), check RBAC (403), forward (502), result back to the agent" width="100%">
</p>

Errors come back as JSON, for example `{"error": "tool_not_permitted", "message": "forbidden: tool not permitted"}`.

## Quick start

**You need:** Docker with Compose, and `openssl` (or any way to make 32 random bytes of hex).

**1. Create your `.env`**

```bash
cp .env.example .env
openssl rand -hex 32   # paste the output into KEY_ENCRYPTION_SECRET
```

Then edit `.env`:

- set `KEY_ENCRYPTION_SECRET` to the 64 hex characters you just generated,
- set `ADMIN_INITIAL_PASSWORD` to something strong,
- change the host in `DATABASE_URL` from `localhost` to **`postgres`** (the Compose service name):

```bash
DATABASE_URL=postgres://agentgate:secret@postgres:5432/agentgate
```

> [!IMPORTANT]
> `.env.example` points at `localhost`, which is right for running the backend on your machine but not
> from inside the Compose network. With `localhost` the `agentgate` container can't reach Postgres.

**2. Start everything**

```bash
docker compose up --build
```

| Service | URL |
| --- | --- |
| Admin dashboard | http://localhost:3000 |
| Admin API | http://localhost:8081 |
| Agent proxy | http://localhost:8080 |
| PostgreSQL | `localhost:5432` |

On boot AgentGate runs the database migrations, creates the first admin user from
`ADMIN_INITIAL_USERNAME` / `ADMIN_INITIAL_PASSWORD` (only when no admin exists yet), and generates its
signing key if there isn't one.

**3. Sign in** at http://localhost:3000 and add a server, an agent and a permission. Or do it all from the terminal, below.

## Walkthrough with curl

<details open>
<summary><b>From zero to an allowed tool call in six requests</b></summary>

Uses [`jq`](https://jqlang.github.io/jq/) to pull IDs out of the responses.

```bash
# 1. Log in (admin tokens last 8 hours)
ADMIN=$(curl -s -X POST http://localhost:8081/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"<your ADMIN_INITIAL_PASSWORD>"}' | jq -r .token)

# 2. Register an upstream MCP server (reachable from the agentgate container)
SERVER=$(curl -s -X POST http://localhost:8081/api/mcp-servers \
  -H "Authorization: Bearer $ADMIN" -H 'Content-Type: application/json' \
  -d '{"name":"files","base_url":"http://my-mcp-server:3000","description":"Filesystem tools"}' | jq -r .id)

# 3. Create an agent
AGENT=$(curl -s -X POST http://localhost:8081/api/agents \
  -H "Authorization: Bearer $ADMIN" -H 'Content-Type: application/json' \
  -d '{"name":"coding-assistant","description":"Reads the repo"}' | jq -r .id)

# 4. Allow exactly one tool
curl -s -X PUT http://localhost:8081/api/permissions \
  -H "Authorization: Bearer $ADMIN" -H 'Content-Type: application/json' \
  -d "{\"agent_id\":\"$AGENT\",\"mcp_server_id\":\"$SERVER\",\"tool_name\":\"read_file\",\"is_allowed\":true}"

# 5. Issue the agent a 1-hour token scoped to that server
TOKEN=$(curl -s -X POST http://localhost:8081/api/agents/$AGENT/tokens \
  -H "Authorization: Bearer $ADMIN" -H 'Content-Type: application/json' \
  -d "{\"label\":\"ci-run-1\",\"ttl_secs\":3600,\"servers\":[\"$SERVER\"]}" | jq -r .token)

# 6. Call the tool through the gateway
curl -s -X POST http://localhost:8080/mcp \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read_file","arguments":{"path":"README.md"}}}'
```

Swap `read_file` for a tool you didn't allow and the same call returns **403 `tool_not_permitted`**.
</details>

## API reference

### Admin API (`:8081`)

Everything except login and health needs `Authorization: Bearer <admin token>`.

| Method | Path | Body | Returns |
| --- | --- | --- | --- |
| `POST` | `/api/auth/login` | `{username, password}` | `{token}` |
| `GET` | `/api/agents` | | list of agents |
| `POST` | `/api/agents` | `{name, description?}` | `201` agent |
| `GET` | `/api/agents/:id` | | agent |
| `DELETE` | `/api/agents/:id` | | `204` |
| `POST` | `/api/agents/:id/tokens` | `{label?, ttl_secs?, servers?: [uuid]}` | `201` `{token, jti}` |
| `GET` | `/api/mcp-servers` | | list of servers |
| `POST` | `/api/mcp-servers` | `{name, base_url, description?, timeout_ms?}` | `201` server |
| `DELETE` | `/api/mcp-servers/:id` | | `204` |
| `GET` | `/api/permissions` | | list of permissions |
| `PUT` | `/api/permissions` | `{agent_id, mcp_server_id, tool_name, is_allowed, expires_at?}` | upserted permission |
| `DELETE` | `/api/permissions/:id` | | `204` |
| `GET` | `/api/health` | | `{"status":"ok"}` |

### Proxy (`:8080`)

`ANY /*path` with `Authorization: Bearer <agent token>` and a JSON-RPC body.

- **`tools/call`**: the tool name is read from `params.name`, checked against the permission table, and if
  allowed the original body is `POST`ed to the server's `base_url` with an `X-AgentGate-Agent-Id` header.
  The upstream JSON response is returned as-is. Upstream errors return `502`; an upstream timeout returns `500`.
- **Any other method**: authenticated, then answered with `{"jsonrpc":"2.0","result":null,"id":…}`. It is **not** forwarded yet.

**Which server?** The first ID in the token's `servers` claim. If the token has no servers, the oldest active server.

## Configuration

All settings are environment variables, read from `.env` (see [`.env.example`](.env.example)).

| Variable | Required | Default | Description |
| --- | :---: | --- | --- |
| `DATABASE_URL` | yes | | Postgres connection string. Use host `postgres` under Docker Compose. |
| `KEY_ENCRYPTION_SECRET` | yes | | 64 hex chars (32 bytes). AES-256-GCM key that encrypts the stored signing key. Generate with `openssl rand -hex 32`. |
| `ADMIN_INITIAL_PASSWORD` | yes | | Password for the bootstrap admin, used only when no admin exists yet. |
| `ADMIN_INITIAL_USERNAME` | | `admin` | Username for the bootstrap admin. |
| `PROXY_PORT` | | `8080` | Agent-facing proxy port. |
| `ADMIN_PORT` | | `8081` | Admin API port. |
| `JWT_ISSUER` | | `agentgate` | `iss` claim on issued tokens. Keep the default: token validation currently expects `agentgate`. |
| `JWT_DEFAULT_TTL_SECS` | | `3600` | Agent token lifetime when `ttl_secs` isn't given. |
| `UPSTREAM_REQUEST_TIMEOUT_MS` | | `30000` | Timeout for calls to upstream MCP servers. |
| `MAX_REQUEST_BODY_BYTES` | | `1048576` | Largest proxy request body accepted (1 MiB). |
| `RUST_LOG` | | `info` | Log filter. Logs are structured JSON. |

The dashboard reads `NEXT_PUBLIC_ADMIN_API_URL` (default `http://localhost:8081`), which Compose already sets.

> [!CAUTION]
> Changing or losing `KEY_ENCRYPTION_SECRET` makes the stored signing key unreadable, and AgentGate will fail to start
> against that database. Keep it somewhere safe.

## Running without Docker

```bash
# Postgres only, in Docker
docker compose up postgres -d

# Backend: keep DATABASE_URL pointing at localhost in .env
cd backend
cargo run            # finds ../.env, runs migrations, serves :8080 and :8081

# Dashboard, in another terminal
cd frontend
npm install
npm run dev          # http://localhost:3000
```

## FAQ

<details>
<summary><b>Can my agent list tools through AgentGate?</b></summary>

Not yet. Only `tools/call` is forwarded today; other JSON-RPC methods such as `tools/list` are authenticated
and answered with a `null` result. Forwarding the rest of the protocol and SSE streaming are on the
[roadmap](#roadmap).
</details>

<details>
<summary><b>What happens when a permission isn't set?</b></summary>

The call is denied with `403`. A tool is only reachable when a row exists with `is_allowed = true` and an
`expires_at` that is empty or in the future.
</details>

<details>
<summary><b>How fast do permission changes take effect?</b></summary>

Immediately. A `PUT /api/permissions` clears that agent's cached decisions. Other decisions are cached for
at most 60 seconds.
</details>

<details>
<summary><b>Can I revoke a token?</b></summary>

Not through the API yet. Every issued token is recorded (with its `jti`, label and expiry) so revocation can be
added, but today a token stays valid until it expires. Keep TTLs short.
</details>

<details>
<summary><b>Is it production-ready?</b></summary>

It's an early build (phase 1 of the [plan](PLAN.md)). Before exposing it, note that the admin API currently
allows CORS from any origin, rate limiting and a call audit log aren't built yet, and the Compose file uses
a demo Postgres password.
</details>

## Project layout

```
AgentGate/
├── docker-compose.yml        agentgate + frontend + postgres
├── .env.example              settings template
├── PLAN.md                   architecture and milestone plan
├── backend/                  Rust gateway (one binary, two ports)
│   ├── migrations/           sqlx schema, applied on boot
│   └── src/
│       ├── main.rs           startup, admin bootstrap, dual listeners
│       ├── config.rs         env var loading
│       ├── auth/             claims, KeyRing (RSA + AES-GCM), issuer, validator
│       ├── policy/           RBAC engine + 60 s DashMap cache
│       ├── proxy/            :8080 router, JSON-RPC inspector, forwarder
│       └── admin/            :8081 router, admin auth, CRUD handlers
└── frontend/                 Next.js 14 admin dashboard
    └── src/
        ├── app/              login, agents, servers, permissions (audit: placeholder)
        ├── components/       layout + UI primitives
        └── lib/api/          typed admin API client
```

## Tech stack

| Layer | Built with |
| --- | --- |
| Gateway | Rust 2021, Axum 0.7, Tokio, tower-http |
| Auth | `jsonwebtoken` (RS256), `rsa` (2048-bit keys), `aes-gcm`, Argon2id admin passwords |
| Data | PostgreSQL 16, sqlx 0.7 with embedded migrations |
| Policy cache | `DashMap` with TTL |
| Upstream calls | `reqwest` with rustls |
| Logging | `tracing` with JSON output |
| Dashboard | Next.js 14, React 18, TanStack Query, Tailwind CSS, Radix UI |
| Packaging | Docker multi-stage builds, Docker Compose |

## Roadmap

From [PLAN.md](PLAN.md). Checked items are in the code today.

- [x] **M1 Auth skeleton:** RS256 signing keys, agent and admin tokens, Postgres migrations, JSON logs
- [x] **M2 Proxy + RBAC:** `tools/call` forwarding, per-tool permissions, policy cache with invalidation, admin CRUD
- [ ] Tool discovery (`tools/list` against each server) and token revocation
- [ ] **M3 Rate limiting + audit:** Redis token buckets, audit log to Postgres and Kafka
- [ ] **M4 SSE streaming + full admin UI:** SSE transport, permission matrix, searchable audit log
- [ ] **M5 Hardening:** Prometheus metrics, graceful shutdown, admin API keys, SSRF protection, request IDs
- [ ] **M6 Scale:** partitioned audit log, CSV export, bulk permissions, production Compose file

## Contributing

Issues and pull requests are welcome. For larger changes, open an issue first and check [PLAN.md](PLAN.md)
so the work lines up with the planned milestones.

## License

Not yet specified.
