import { AppLayout } from '@/components/layout/AppLayout'
import { ScrollText } from 'lucide-react'

export default function AuditPage() {
  return (
    <AppLayout>
      <div className="flex flex-col items-center justify-center h-64 gap-3 text-muted-foreground">
        <ScrollText className="h-10 w-10" />
        <p className="text-lg font-medium">Audit Log</p>
        <p className="text-sm">Coming in the next milestone</p>
      </div>
    </AppLayout>
  )
}
