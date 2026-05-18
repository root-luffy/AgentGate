import { apiFetch } from './client'
import type { Permission } from '../types'

export const permissionsApi = {
  list: () => apiFetch<Permission[]>('/api/permissions'),
  upsert: (data: { agent_id: string; mcp_server_id: string; tool_name: string; is_allowed: boolean }) =>
    apiFetch<Permission>('/api/permissions', { method: 'PUT', body: JSON.stringify(data) }),
  delete: (id: string) => apiFetch<void>(`/api/permissions/${id}`, { method: 'DELETE' }),
}
