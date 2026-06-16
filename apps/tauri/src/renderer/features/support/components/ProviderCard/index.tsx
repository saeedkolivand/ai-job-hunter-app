import { AlertTriangle, CheckCircle2, XCircle } from 'lucide-react';

import { Button, cn } from '@ajh/ui';

interface ProviderCardProps {
  name: string;
  status: 'healthy' | 'warning' | 'error' | 'disabled';
  lastScrape: string;
  successRate: string;
  responseTime: string;
  issue?: string;
}

export function ProviderCard({
  name,
  status,
  lastScrape,
  successRate,
  responseTime,
  issue,
}: ProviderCardProps) {
  const statusConfig = {
    healthy: { icon: CheckCircle2, color: 'text-emerald-400', glow: '' },
    warning: { icon: AlertTriangle, color: 'text-amber-400', glow: '' },
    error: { icon: XCircle, color: 'text-red-400', glow: '' },
    disabled: { icon: XCircle, color: 'text-foreground/40', glow: '' },
  };

  const config = statusConfig[status];
  const Icon = config.icon;

  return (
    <div className="flex items-start justify-between p-4 rounded-xl bg-white/[0.02]">
      <div className="flex items-start gap-3">
        <Icon size={16} className={cn('mt-0.5 shrink-0', config.color)} />
        <div>
          <div className="text-sm font-medium text-foreground/90">{name}</div>
          <div className="text-xs text-foreground/55 mt-0.5">
            Last scrape: {lastScrape} · {successRate} success · {responseTime}
          </div>
          {issue && <div className="text-xs text-amber-400/90 mt-1">{issue}</div>}
        </div>
      </div>
      <div className="flex gap-2">
        <Button variant="ghost" className="text-xs">
          Retry
        </Button>
        <Button variant="ghost" className="text-xs">
          Reset
        </Button>
      </div>
    </div>
  );
}
