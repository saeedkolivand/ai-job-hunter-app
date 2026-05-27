import { AlertTriangle, CheckCircle2, XCircle } from 'lucide-react';
import { useState } from 'react';

import { Button, cn } from '@ajh/ui';

interface DocumentIssueCardProps {
  filename: string;
  issue: string;
  status: 'healthy' | 'warning' | 'error';
  actions: string[];
}

export function DocumentIssueCard({ filename, issue, status, actions }: DocumentIssueCardProps) {
  const [expanded, setExpanded] = useState(false);

  const statusConfig = {
    healthy: { icon: CheckCircle2, color: 'text-emerald-400' },
    warning: { icon: AlertTriangle, color: 'text-amber-400' },
    error: { icon: XCircle, color: 'text-red-400' },
  };

  const config = statusConfig[status];
  const Icon = config.icon;

  return (
    <div className="glass-card rounded-xl p-4">
      <div className="flex items-start gap-3">
        <Icon size={16} className={cn('mt-0.5 shrink-0', config.color)} />
        <div className="flex-1">
          <div className="text-sm font-medium text-foreground/90">{filename}</div>
          <div className="text-xs text-foreground/55 mt-0.5">{issue}</div>
        </div>
        <Button
          onClick={() => setExpanded(!expanded)}
          className="text-xs text-brand-soft hover:text-brand h-auto bg-transparent border-transparent"
        >
          {expanded ? 'Hide' : 'Actions'}
        </Button>
      </div>
      {expanded && (
        <div className="mt-3 ml-7 space-y-1">
          {actions.map((action, i) => (
            <Button
              key={i}
              className="block text-xs text-foreground/70 hover:text-foreground transition-colors h-auto bg-transparent border-transparent"
            >
              • {action}
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}
