'use client'
import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Plus, Trash2 } from 'lucide-react'
import { toast } from 'sonner'
import { AppLayout } from '@/components/layout/AppLayout'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { agentsApi } from '@/lib/api/agents'
import { serversApi } from '@/lib/api/servers'
import { permissionsApi } from '@/lib/api/permissions'
import type { Permission } from '@/lib/types'

export default function PermissionsPage() {
  const queryClient = useQueryClient()

  const [agentId, setAgentId] = useState('')
  const [serverId, setServerId] = useState('')
  const [toolName, setToolName] = useState('')
  const [isAllowed, setIsAllowed] = useState(true)

  const { data: agents, isLoading: agentsLoading } = useQuery({
    queryKey: ['agents'],
    queryFn: agentsApi.list,
  })

  const { data: servers, isLoading: serversLoading } = useQuery({
    queryKey: ['servers'],
    queryFn: serversApi.list,
  })

  const {
    data: permissions,
    isLoading: permsLoading,
    isError: permsError,
  } = useQuery({
    queryKey: ['permissions'],
    queryFn: permissionsApi.list,
  })

  const upsertMutation = useMutation({
    mutationFn: (data: {
      agent_id: string
      mcp_server_id: string
      tool_name: string
      is_allowed: boolean
    }) => permissionsApi.upsert(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['permissions'] })
      toast.success('Permission saved')
      setAgentId('')
      setServerId('')
      setToolName('')
      setIsAllowed(true)
    },
    onError: (err: Error) => {
      toast.error(err.message || 'Failed to save permission')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => permissionsApi.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['permissions'] })
      toast.success('Permission deleted')
    },
    onError: (err: Error) => {
      toast.error(err.message || 'Failed to delete permission')
    },
  })

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!agentId || !serverId || !toolName.trim()) {
      toast.error('Please fill in all required fields')
      return
    }
    upsertMutation.mutate({
      agent_id: agentId,
      mcp_server_id: serverId,
      tool_name: toolName.trim(),
      is_allowed: isAllowed,
    })
  }

  function handleDelete(perm: Permission) {
    if (!window.confirm('Delete this permission?')) return
    deleteMutation.mutate(perm.id)
  }

  function getAgentName(id: string) {
    return agents?.find((a) => a.id === id)?.name ?? id
  }

  function getServerName(id: string) {
    return servers?.find((s) => s.id === id)?.name ?? id
  }

  const selectClass =
    'flex h-9 w-full rounded-md border border-border bg-muted px-3 py-1 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-blue-500 disabled:opacity-50'

  return (
    <AppLayout>
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-bold text-foreground">Permissions</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Control which agents can invoke which tools on which servers
          </p>
        </div>

        {/* Add Permission Form */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Add Permission</CardTitle>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleSubmit} className="space-y-4">
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                <div className="space-y-1.5">
                  <Label htmlFor="perm-agent">Agent</Label>
                  <select
                    id="perm-agent"
                    className={selectClass}
                    value={agentId}
                    onChange={(e) => setAgentId(e.target.value)}
                    required
                    disabled={agentsLoading}
                  >
                    <option value="">Select agent…</option>
                    {agents?.map((agent) => (
                      <option key={agent.id} value={agent.id}>
                        {agent.name}
                      </option>
                    ))}
                  </select>
                </div>

                <div className="space-y-1.5">
                  <Label htmlFor="perm-server">MCP Server</Label>
                  <select
                    id="perm-server"
                    className={selectClass}
                    value={serverId}
                    onChange={(e) => setServerId(e.target.value)}
                    required
                    disabled={serversLoading}
                  >
                    <option value="">Select server…</option>
                    {servers?.map((server) => (
                      <option key={server.id} value={server.id}>
                        {server.name}
                      </option>
                    ))}
                  </select>
                </div>

                <div className="space-y-1.5">
                  <Label htmlFor="perm-tool">Tool Name</Label>
                  <Input
                    id="perm-tool"
                    placeholder="e.g. read_file"
                    value={toolName}
                    onChange={(e) => setToolName(e.target.value)}
                    required
                  />
                </div>

                <div className="space-y-1.5">
                  <Label>Allow / Deny</Label>
                  <div className="flex items-center gap-2 h-9">
                    <Switch
                      checked={isAllowed}
                      onCheckedChange={setIsAllowed}
                      id="perm-allowed"
                    />
                    <Label htmlFor="perm-allowed" className="cursor-pointer">
                      {isAllowed ? (
                        <span className="text-green-400">Allow</span>
                      ) : (
                        <span className="text-red-400">Deny</span>
                      )}
                    </Label>
                  </div>
                </div>
              </div>

              <div className="flex justify-end">
                <Button type="submit" disabled={upsertMutation.isPending}>
                  <Plus className="h-4 w-4 mr-2" />
                  {upsertMutation.isPending ? 'Saving…' : 'Add Permission'}
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>

        {/* Permissions Table */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Current Permissions</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            {permsLoading && (
              <p className="text-muted-foreground text-sm p-6">Loading permissions…</p>
            )}
            {permsError && (
              <p className="text-destructive text-sm p-6">Failed to load permissions.</p>
            )}
            {permissions && permissions.length === 0 && (
              <p className="text-muted-foreground text-sm p-6">
                No permissions defined yet.
              </p>
            )}
            {permissions && permissions.length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Agent</TableHead>
                    <TableHead>Server</TableHead>
                    <TableHead>Tool</TableHead>
                    <TableHead>Allowed</TableHead>
                    <TableHead>Expires</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {permissions.map((perm) => (
                    <TableRow key={perm.id}>
                      <TableCell className="font-medium text-foreground">
                        {getAgentName(perm.agent_id)}
                      </TableCell>
                      <TableCell className="text-muted-foreground">
                        {getServerName(perm.mcp_server_id)}
                      </TableCell>
                      <TableCell className="font-mono text-xs text-foreground">
                        {perm.tool_name}
                      </TableCell>
                      <TableCell>
                        <Badge variant={perm.is_allowed ? 'success' : 'destructive'}>
                          {perm.is_allowed ? 'Allow' : 'Deny'}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-muted-foreground text-xs">
                        {perm.expires_at
                          ? new Date(perm.expires_at).toLocaleDateString()
                          : 'Never'}
                      </TableCell>
                      <TableCell className="text-right">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleDelete(perm)}
                          disabled={deleteMutation.isPending}
                        >
                          <Trash2 className="h-4 w-4 text-red-400" />
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      </div>
    </AppLayout>
  )
}
