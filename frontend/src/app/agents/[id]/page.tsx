'use client'
import { useState } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import { useParams, useRouter } from 'next/navigation'
import { ArrowLeft, Copy, Check, AlertTriangle } from 'lucide-react'
import { toast } from 'sonner'
import { AppLayout } from '@/components/layout/AppLayout'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { agentsApi } from '@/lib/api/agents'
import type { IssuedToken } from '@/lib/types'

export default function AgentDetailPage() {
  const params = useParams()
  const router = useRouter()
  const id = params.id as string

  const [tokenLabel, setTokenLabel] = useState('')
  const [ttlSecs, setTtlSecs] = useState('')
  const [issuedToken, setIssuedToken] = useState<IssuedToken | null>(null)
  const [copied, setCopied] = useState(false)

  const { data: agent, isLoading, isError } = useQuery({
    queryKey: ['agents', id],
    queryFn: () => agentsApi.get(id),
    enabled: !!id,
  })

  const issueTokenMutation = useMutation({
    mutationFn: (data: { label?: string; ttl_secs?: number }) =>
      agentsApi.issueToken(id, data),
    onSuccess: (data) => {
      setIssuedToken(data)
      toast.success('Token issued successfully')
    },
    onError: (err: Error) => {
      toast.error(err.message || 'Failed to issue token')
    },
  })

  function handleIssueToken(e: React.FormEvent) {
    e.preventDefault()
    const data: { label?: string; ttl_secs?: number } = {}
    if (tokenLabel.trim()) data.label = tokenLabel.trim()
    if (ttlSecs.trim()) {
      const parsed = parseInt(ttlSecs, 10)
      if (!isNaN(parsed) && parsed > 0) data.ttl_secs = parsed
    }
    setIssuedToken(null)
    issueTokenMutation.mutate(data)
  }

  async function handleCopy() {
    if (!issuedToken) return
    try {
      await navigator.clipboard.writeText(issuedToken.token)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
      toast.success('Token copied to clipboard')
    } catch {
      toast.error('Failed to copy token')
    }
  }

  return (
    <AppLayout>
      <div className="space-y-6 max-w-3xl">
        <div className="flex items-center gap-3">
          <Button variant="ghost" size="icon" onClick={() => router.push('/agents')}>
            <ArrowLeft className="h-4 w-4" />
          </Button>
          <div>
            <h1 className="text-2xl font-bold text-foreground">
              {isLoading ? 'Loading…' : agent?.name ?? 'Agent'}
            </h1>
            <p className="text-sm text-muted-foreground mt-0.5">Agent Details</p>
          </div>
        </div>

        {isLoading && (
          <p className="text-muted-foreground text-sm">Loading agent…</p>
        )}

        {isError && (
          <p className="text-destructive text-sm">Failed to load agent.</p>
        )}

        {agent && (
          <>
            {/* Agent Info */}
            <Card>
              <CardHeader>
                <CardTitle className="text-base">Agent Information</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <p className="text-xs text-muted-foreground mb-1">Name</p>
                    <p className="text-sm font-medium text-foreground">{agent.name}</p>
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground mb-1">Status</p>
                    <Badge variant={agent.is_active ? 'success' : 'secondary'}>
                      {agent.is_active ? 'Active' : 'Inactive'}
                    </Badge>
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground mb-1">Description</p>
                    <p className="text-sm text-foreground">{agent.description || '—'}</p>
                  </div>
                  <div>
                    <p className="text-xs text-muted-foreground mb-1">Created</p>
                    <p className="text-sm text-foreground">
                      {new Date(agent.created_at).toLocaleString()}
                    </p>
                  </div>
                  <div className="col-span-2">
                    <p className="text-xs text-muted-foreground mb-1">Agent ID</p>
                    <p className="text-xs font-mono text-muted-foreground break-all">{agent.id}</p>
                  </div>
                </div>
              </CardContent>
            </Card>

            {/* Issue Token */}
            <Card>
              <CardHeader>
                <CardTitle className="text-base">Issue Access Token</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <form onSubmit={handleIssueToken} className="space-y-4">
                  <div className="grid grid-cols-2 gap-4">
                    <div className="space-y-1.5">
                      <Label htmlFor="token-label">Label (optional)</Label>
                      <Input
                        id="token-label"
                        placeholder="e.g. production"
                        value={tokenLabel}
                        onChange={(e) => setTokenLabel(e.target.value)}
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="ttl-secs">TTL in seconds (optional)</Label>
                      <Input
                        id="ttl-secs"
                        type="number"
                        min="1"
                        placeholder="e.g. 86400"
                        value={ttlSecs}
                        onChange={(e) => setTtlSecs(e.target.value)}
                      />
                    </div>
                  </div>
                  <Button type="submit" disabled={issueTokenMutation.isPending}>
                    {issueTokenMutation.isPending ? 'Issuing…' : 'Issue Token'}
                  </Button>
                </form>

                {issuedToken && (
                  <div className="mt-4 space-y-3">
                    <div className="flex items-center gap-2 text-yellow-400 text-sm">
                      <AlertTriangle className="h-4 w-4 shrink-0" />
                      <span>
                        Copy this token now. It will not be shown again.
                      </span>
                    </div>
                    <div className="relative">
                      <textarea
                        readOnly
                        value={issuedToken.token}
                        className="w-full rounded-md border border-border bg-muted p-3 text-xs font-mono text-foreground resize-none h-24 focus:outline-none"
                      />
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="absolute right-2 top-2"
                        onClick={handleCopy}
                      >
                        {copied ? (
                          <Check className="h-3.5 w-3.5 text-green-400" />
                        ) : (
                          <Copy className="h-3.5 w-3.5" />
                        )}
                      </Button>
                    </div>
                    <div className="text-xs text-muted-foreground">
                      <span className="font-medium">Token ID (JTI):</span>{' '}
                      <span className="font-mono">{issuedToken.jti}</span>
                    </div>
                  </div>
                )}
              </CardContent>
            </Card>
          </>
        )}
      </div>
    </AppLayout>
  )
}
