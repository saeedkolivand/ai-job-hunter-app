import { cn } from '@ajh/ui';

interface MemoryBarProps {
  label: string;
  used: string;
  total: string;
  percentage: number;
  status: 'healthy' | 'warning' | 'error';
}

export function MemoryBar({ label, used, total, percentage, status }: MemoryBarProps) {
  const statusConfig = {
    healthy: 'from-emerald-400 to-emerald-500',
    warning: 'from-amber-400 to-amber-500',
    error: 'from-red-400 to-red-500',
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-sm">
        <span className="text-foreground/90">{label}</span>
        <span className="text-foreground/55">
          {used} / {total}
        </span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-white/5">
        <div
          className={cn('h-full rounded-full bg-gradient-to-r', statusConfig[status])}
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  );
}
