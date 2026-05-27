import { AlertTriangle, CheckCircle2, XCircle } from 'lucide-react';

import { Button, cn } from '@ajh/ui';

interface DiagnosticItemProps {
  name: string;
  status: 'healthy' | 'warning' | 'error' | 'disabled';
  description: string;
  action?: string;
}

export function DiagnosticItem({ name, status, description, action }: DiagnosticItemProps) {
  const statusConfig = {
    healthy: { icon: CheckCircle2, color: 'text-emerald-400' },
    warning: { icon: AlertTriangle, color: 'text-amber-400' },
    error: { icon: XCircle, color: 'text-red-400' },
    disabled: { icon: XCircle, color: 'text-foreground/40' },
  };

  const config = statusConfig[status];
  const Icon = config.icon;

  return (
    <div className="flex items-start justify-between p-3 rounded-xl bg-white/[0.02]">
      <div className="flex items-start gap-3">
        <Icon size={16} className={cn('mt-0.5 shrink-0', config.color)} />
        <div>
          <div className="text-sm font-medium text-foreground/90">{name}</div>
          <div className="text-xs text-foreground/55 mt-0.5">{description}</div>
        </div>
      </div>
      {action && (
        <Button size="sm" variant="ghost" className="text-xs">
          {action}
        </Button>
      )}
    </div>
  );
}
