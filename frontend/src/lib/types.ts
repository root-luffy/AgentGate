export interface Agent {
  id: string
  name: string
  description: string | null
  is_active: boolean
  created_at: string
}

export interface McpServer {
  id: string
  name: string
  base_url: string
  description: string | null
  is_active: boolean
  created_at: string
}

export interface Permission {
  id: string
  agent_id: string
  mcp_server_id: string
  tool_name: string
  is_allowed: boolean
  expires_at: string | null
}

export interface IssuedToken {
  token: string
  jti: string
}
