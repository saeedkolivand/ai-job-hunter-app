import { AlertTriangle, CheckCircle2, XCircle } from 'lucide-react';

import { cn } from '@ajh/ui';

interface HealthCardProps {
  name: string;
  status: 'healthy' | 'warning' | 'error' | 'disabled';
  description: string;
}

export function HealthCard({ name, status, description }: HealthCardProps) {
  const statusConfig = {
    healthy: { icon: CheckCircle2, color: 'text-emerald-400', glow: 'ring-1 ring-emerald-400/20' },
    warning: { icon: AlertTriangle, color: 'text-amber-400', glow: '' },
    error: { icon: XCircle, color: 'text-red-400', glow: '' },
    disabled: { icon: XCircle, color: 'text-foreground/40', glow: '' },
  };

  const config = statusConfig[status];
  const Icon = config.icon;

  return (
    <div className={cn('glass-card rounded-2xl p-5', config.glow)}>
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-2">
          <Icon size={16} className={config.color} />
          <span className="text-sm font-medium text-foreground/90">{name}</span>
        </div>
        <span className={cn('text-[10px] uppercase tracking-wider', config.color)}>{status}</span>
      </div>
      <p className="text-xs text-foreground/55">{description}</p>
    </div>
  );
}
