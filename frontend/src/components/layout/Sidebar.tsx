'use client'
import Link from 'next/link'
import { usePathname, useRouter } from 'next/navigation'
import { Bot, Server, Shield, ScrollText, LogOut, Zap } from 'lucide-react'
import { cn } from '@/lib/utils'
import { clearToken } from '@/lib/auth'

const links = [
  { href: '/agents', label: 'Agents', icon: Bot },
  { href: '/servers', label: 'MCP Servers', icon: Server },
  { href: '/permissions', label: 'Permissions', icon: Shield },
  { href: '/audit', label: 'Audit Log', icon: ScrollText },
]

export function Sidebar() {
  const pathname = usePathname()
  const router = useRouter()

  function handleLogout() {
    clearToken()
    router.push('/login')
  }

  return (
    <aside className="flex h-screen w-56 flex-col border-r border-border bg-card">
      <div className="flex items-center gap-2 px-6 py-5 border-b border-border">
        <Zap className="h-5 w-5 text-blue-400" />
        <span className="font-bold text-foreground">AgentGate</span>
      </div>
      <nav className="flex-1 space-y-1 p-3">
        {links.map(({ href, label, icon: Icon }) => (
          <Link
            key={href}
            href={href}
            className={cn(
              'flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors',
              pathname.startsWith(href)
                ? 'bg-blue-600/20 text-blue-400'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'
            )}
          >
            <Icon className="h-4 w-4" />
            {label}
          </Link>
        ))}
      </nav>
      <div className="border-t border-border p-3">
        <button
          onClick={handleLogout}
          className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
        >
          <LogOut className="h-4 w-4" />
          Logout
        </button>
      </div>
    </aside>
  )
}
