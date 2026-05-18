import { cn } from '@/lib/utils'

export function Badge({
  className,
  variant = 'default',
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & {
  variant?: 'default' | 'success' | 'destructive' | 'secondary'
}) {
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium',
        variant === 'default' && 'bg-blue-900 text-blue-300',
        variant === 'success' && 'bg-green-900 text-green-300',
        variant === 'destructive' && 'bg-red-900 text-red-300',
        variant === 'secondary' && 'bg-muted text-muted-foreground',
        className
      )}
      {...props}
    />
  )
}
