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
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { serversApi } from '@/lib/api/servers'
import type { McpServer } from '@/lib/types'

export default function ServersPage() {
  const queryClient = useQueryClient()
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [baseUrl, setBaseUrl] = useState('')
  const [description, setDescription] = useState('')

  const { data: servers, isLoading, isError } = useQuery({
    queryKey: ['servers'],
    queryFn: serversApi.list,
  })

  const createMutation = useMutation({
    mutationFn: (data: { name: string; base_url: string; description?: string }) =>
      serversApi.create(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['servers'] })
      toast.success('Server registered successfully')
      setOpen(false)
      setName('')
      setBaseUrl('')
      setDescription('')
    },
    onError: (err: Error) => {
      toast.error(err.message || 'Failed to register server')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => serversApi.delete(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['servers'] })
      toast.success('Server removed')
    },
    onError: (err: Error) => {
      toast.error(err.message || 'Failed to remove server')
    },
  })

  function handleCreate(e: React.FormEvent) {
    e.preventDefault()
    if (!name.trim() || !baseUrl.trim()) return
    createMutation.mutate({
      name: name.trim(),
      base_url: baseUrl.trim(),
      description: description.trim() || undefined,
    })
  }

  function handleDelete(server: McpServer) {
    if (!window.confirm(`Remove server "${server.name}"? This cannot be undone.`)) return
    deleteMutation.mutate(server.id)
  }

  return (
    <AppLayout>
      <div className="space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground">MCP Servers</h1>
            <p className="text-sm text-muted-foreground mt-1">
              Register and manage MCP tool servers
            </p>
          </div>
          <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger asChild>
              <Button>
                <Plus className="h-4 w-4 mr-2" />
                Register Server
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Register MCP Server</DialogTitle>
              </DialogHeader>
              <form onSubmit={handleCreate} className="space-y-4">
                <div className="space-y-1.5">
                  <Label htmlFor="server-name">Name</Label>
                  <Input
                    id="server-name"
                    placeholder="filesystem-server"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    required
                    autoFocus
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="server-url">Base URL</Label>
                  <Input
                    id="server-url"
                    placeholder="http://localhost:8082"
                    value={baseUrl}
                    onChange={(e) => setBaseUrl(e.target.value)}
                    required
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="server-desc">Description</Label>
                  <Input
                    id="server-desc"
                    placeholder="Optional description"
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                  />
                </div>
                <div className="flex gap-2 justify-end pt-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => setOpen(false)}
                  >
                    Cancel
                  </Button>
                  <Button type="submit" disabled={createMutation.isPending}>
                    {createMutation.isPending ? 'Registering…' : 'Register'}
                  </Button>
                </div>
              </form>
            </DialogContent>
          </Dialog>
        </div>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Registered Servers</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            {isLoading && (
              <p className="text-muted-foreground text-sm p-6">Loading servers…</p>
            )}
            {isError && (
              <p className="text-destructive text-sm p-6">Failed to load servers.</p>
            )}
            {servers && servers.length === 0 && (
              <p className="text-muted-foreground text-sm p-6">
                No servers registered yet.
              </p>
            )}
            {servers && servers.length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Base URL</TableHead>
                    <TableHead>Description</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Created</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {servers.map((server) => (
                    <TableRow key={server.id}>
                      <TableCell className="font-medium text-foreground">
                        {server.name}
                      </TableCell>
                      <TableCell className="text-muted-foreground font-mono text-xs">
                        {server.base_url}
                      </TableCell>
                      <TableCell className="text-muted-foreground">
                        {server.description || '—'}
                      </TableCell>
                      <TableCell>
                        <Badge variant={server.is_active ? 'success' : 'secondary'}>
                          {server.is_active ? 'Active' : 'Inactive'}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-muted-foreground text-xs">
                        {new Date(server.created_at).toLocaleDateString()}
                      </TableCell>
                      <TableCell className="text-right">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleDelete(server)}
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
