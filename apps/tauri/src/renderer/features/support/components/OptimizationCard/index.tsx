import { Button, cn } from '@ajh/ui';

interface OptimizationCardProps {
  title: string;
  description: string;
  action: string;
  priority: 'high' | 'medium' | 'low';
}

export function OptimizationCard({ title, description, action, priority }: OptimizationCardProps) {
  const priorityConfig = {
    high: { color: 'text-red-400', border: 'border-red-400/20', bg: 'bg-red-400/5' },
    medium: { color: 'text-amber-400', border: 'border-amber-400/20', bg: 'bg-amber-400/5' },
    low: { color: 'text-emerald-400', border: 'border-emerald-400/20', bg: 'bg-emerald-400/5' },
  };

  const config = priorityConfig[priority];

  return (
    <div
      className={cn(
        'flex items-start justify-between p-4 rounded-xl border',
        config.border,
        config.bg
      )}
    >
      <div>
        <div className="flex items-center gap-2 mb-1">
          <span className="text-xs font-semibold uppercase tracking-wider">
            {priority} priority
          </span>
          <div className="text-sm font-medium text-foreground/90">{title}</div>
        </div>
        <div className="text-xs text-foreground/55">{description}</div>
      </div>
      <Button variant="glass" className="text-xs">
        {action}
      </Button>
    </div>
  );
}
