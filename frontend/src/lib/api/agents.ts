import { apiFetch } from './client'
import type { Agent, IssuedToken } from '../types'

export const agentsApi = {
  list: () => apiFetch<Agent[]>('/api/agents'),
  get: (id: string) => apiFetch<Agent>(`/api/agents/${id}`),
  create: (data: { name: string; description?: string }) =>
    apiFetch<Agent>('/api/agents', { method: 'POST', body: JSON.stringify(data) }),
  delete: (id: string) => apiFetch<void>(`/api/agents/${id}`, { method: 'DELETE' }),
  issueToken: (id: string, data: { label?: string; ttl_secs?: number }) =>
    apiFetch<IssuedToken>(`/api/agents/${id}/tokens`, { method: 'POST', body: JSON.stringify(data) }),
}
