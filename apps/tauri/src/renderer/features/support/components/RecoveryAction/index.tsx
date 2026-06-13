import { AlertTriangle, Loader2 } from 'lucide-react';
import { useState } from 'react';

import { Button, cn, ConfirmModal, useNotification } from '@ajh/ui';

interface RecoveryActionProps {
  title: string;
  description: string;
  destructive: boolean;
  action: string;
  successMessage?: string;
  onAction?: () => void | Promise<void>;
}

export function RecoveryAction({
  title,
  description,
  destructive,
  action,
  successMessage,
  onAction,
}: RecoveryActionProps) {
  const notify = useNotification();
  const [pending, setPending] = useState(false);
  // Destructive recovery tools (reset / clear / unload) run irreversible work,
  // so they go through a confirm step; non-destructive ones fire immediately.
  const [confirmOpen, setConfirmOpen] = useState(false);

  const run = async () => {
    if (!onAction) return;
    setPending(true);
    try {
      await onAction();
      notify.success({ message: successMessage ?? `${title} completed.` });
    } catch (err) {
      notify.error({ message: err instanceof Error ? err.message : `${title} failed.` });
    } finally {
      setPending(false);
    }
  };

  const handleClick = () => {
    if (destructive) {
      setConfirmOpen(true);
      return;
    }
    void run();
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
        disabled={pending}
        onClick={handleClick}
      >
        {pending ? <Loader2 size={12} className="animate-spin" /> : action}
      </Button>

      <ConfirmModal
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        onConfirm={() => {
          setConfirmOpen(false);
          void run();
        }}
        title={`${title}?`}
        description={`${description} This action cannot be undone.`}
        confirmText={action}
        variant="danger"
        isConfirming={pending}
      />
    </div>
  );
}
