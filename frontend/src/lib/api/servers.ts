import { apiFetch } from './client'
import type { McpServer } from '../types'

export const serversApi = {
  list: () => apiFetch<McpServer[]>('/api/mcp-servers'),
  create: (data: { name: string; base_url: string; description?: string }) =>
    apiFetch<McpServer>('/api/mcp-servers', { method: 'POST', body: JSON.stringify(data) }),
  delete: (id: string) => apiFetch<void>(`/api/mcp-servers/${id}`, { method: 'DELETE' }),
}
