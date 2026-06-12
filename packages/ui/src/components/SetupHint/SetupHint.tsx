import { Loader2, type LucideIcon } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { ReactNode } from 'react';

import { transition } from '../../lib/motion';
import { Alert } from '../Alert';
import { Button } from '../Button';

type Tone = 'info' | 'amber';

export interface SetupHintProps {
  /** Collapses (with animation) when false. Default true. */
  show?: boolean;
  message: ReactNode;
  /** Label of the inline one-click action. Omit for a message-only hint. */
  actionLabel?: string;
  onAction?: () => void;
  /** Shows a spinner and disables the action while a fix is in flight. */
  pending?: boolean;
  icon?: LucideIcon;
  tone?: Tone;
  className?: string;
}

/**
 * Inline "a prerequisite blocks this flow — here's the one-click fix" banner.
 * Renders via `Alert` so tone, layout, and accessibility are consistent.
 */
export function SetupHint({
  show = true,
  message,
  actionLabel,
  onAction,
  pending = false,
  icon: IconProp,
  tone = 'info',
  className,
}: SetupHintProps) {
  const alertType = tone === 'amber' ? 'warning' : 'info';
  const iconNode = IconProp ? <IconProp size={12} /> : undefined;

  const action =
    actionLabel && onAction ? (
      <Button
        type="button"
        variant="unstyled"
        disabled={pending}
        onClick={onAction}
        className="shrink-0 text-[11px] text-brand-soft underline-offset-2 hover:underline disabled:opacity-50"
      >
        {pending ? <Loader2 size={11} className="animate-spin" /> : actionLabel}
      </Button>
    ) : undefined;

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          initial={{ opacity: 0, height: 0, marginBottom: 0 }}
          animate={{ opacity: 1, height: 'auto', marginBottom: 16 }}
          exit={{ opacity: 0, height: 0, marginBottom: 0 }}
          transition={transition.fast}
          className="overflow-hidden"
        >
          <Alert
            type={alertType}
            message={message}
            showIcon
            icon={iconNode}
            action={action}
            className={className}
            style={{ fontSize: '11px', padding: '6px 10px' }}
          />
        </motion.div>
      )}
    </AnimatePresence>
  );
}
