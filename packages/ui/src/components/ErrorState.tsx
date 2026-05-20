import { AlertTriangle, RefreshCw } from 'lucide-react';
import type { ReactNode } from 'react';
import { Button } from './Button';
import { cn } from '../lib/cn';

interface ErrorStateProps {
  title?: string;
  description?: string;
  onRetry?: () => void;
  action?: ReactNode;
  className?: string;
}

/**
 * Consistent error display — used for failed data loads and caught errors.
 */
export function ErrorState({
  title = 'Something went wrong',
  description,
  onRetry,
  action,
  className,
}: ErrorStateProps) {
  return (
    <div
      className={cn('flex flex-col items-center justify-center gap-4 py-16 text-center', className)}
    >
      <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-red-500/10 ring-1 ring-red-500/20">
        <AlertTriangle size={24} className="text-red-400/70" />
      </div>
      <div className="space-y-1">
        <p className="text-sm font-medium text-foreground/70">{title}</p>
        {description && <p className="max-w-xs text-xs text-foreground/40">{description}</p>}
      </div>
      {onRetry && (
        <Button variant="ghost" size="sm" onClick={onRetry}>
          <RefreshCw size={13} />
          Try again
        </Button>
      )}
      {action}
    </div>
  );
}
