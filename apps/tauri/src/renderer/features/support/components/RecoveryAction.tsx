import { AlertTriangle } from 'lucide-react';

import { Button } from '@ajh/ui';

import { cn } from '@/lib/cn';

interface RecoveryActionProps {
  title: string;
  description: string;
  destructive: boolean;
  action: string;
  onAction?: () => void | Promise<void>;
}

export function RecoveryAction({
  title,
  description,
  destructive,
  action,
  onAction,
}: RecoveryActionProps) {
  const handleClick = async () => {
    if (onAction) {
      await onAction();
    }
  };

  return (
    <div
      className={cn(
        'flex items-start justify-between p-4 rounded-xl border',
        destructive ? 'border-red-400/20 bg-red-400/5' : 'border-white/10 bg-white/[0.02]'
      )}
    >
      <div className="flex-1">
        <div className="flex items-center gap-2 mb-1">
          {destructive && <AlertTriangle size={14} className="text-red-400" />}
          <div className="text-sm font-medium text-foreground/90">{title}</div>
        </div>
        <div className="text-xs text-foreground/55">{description}</div>
      </div>
      <Button
        size="sm"
        variant={destructive ? 'ghost' : 'glass'}
        className={cn('text-xs', destructive && 'text-red-400 hover:text-red-300')}
        onClick={handleClick}
      >
        {action}
      </Button>
    </div>
  );
}
