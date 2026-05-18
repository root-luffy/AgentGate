CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE signing_keys (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    kid         TEXT        NOT NULL UNIQUE,
    public_key  TEXT        NOT NULL,
    private_key TEXT        NOT NULL,
    algorithm   TEXT        NOT NULL DEFAULT 'RS256',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    retired_at  TIMESTAMPTZ
);

CREATE TABLE agents (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL,
    description TEXT,
    is_active   BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE mcp_servers (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL UNIQUE,
    base_url    TEXT        NOT NULL,
    description TEXT,
    is_active   BOOLEAN     NOT NULL DEFAULT TRUE,
    timeout_ms  INT         NOT NULL DEFAULT 30000,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tools (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    mcp_server_id UUID        NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    description   TEXT,
    input_schema  JSONB,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (mcp_server_id, name)
);

CREATE TABLE agent_tool_permissions (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id      UUID        NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    mcp_server_id UUID        NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    tool_name     TEXT        NOT NULL,
    is_allowed    BOOLEAN     NOT NULL DEFAULT FALSE,
    expires_at    TIMESTAMPTZ,
    created_by    TEXT        NOT NULL DEFAULT 'system',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (agent_id, mcp_server_id, tool_name)
);

CREATE INDEX idx_atp_lookup ON agent_tool_permissions(agent_id, mcp_server_id, tool_name);

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

CREATE INDEX idx_tokens_jti   ON issued_tokens(jti);
CREATE INDEX idx_tokens_agent ON issued_tokens(agent_id);

CREATE TABLE admin_users (
    id            UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    username      TEXT    NOT NULL UNIQUE,
    password_hash TEXT    NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login    TIMESTAMPTZ
);
