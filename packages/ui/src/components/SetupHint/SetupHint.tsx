import { Info, Loader2, type LucideIcon } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { transition } from '../../lib/motion';

type Tone = 'info' | 'amber';

const TONE: Record<Tone, { box: string; icon: string }> = {
  info: { box: 'border-blue-400/15 bg-blue-400/5 text-blue-200/75', icon: 'text-blue-400/60' },
  amber: {
    box: 'border-amber-400/15 bg-amber-400/5 text-amber-200/80',
    icon: 'text-amber-400/60',
  },
};

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
 * Generalized from the jobs ScrapeForm auth hint so every blocked flow (no AI
 * provider, disconnected board, …) gets the same nudge + action affordance.
 */
export function SetupHint({
  show = true,
  message,
  actionLabel,
  onAction,
  pending = false,
  icon: Icon = Info,
  tone = 'info',
  className,
}: SetupHintProps) {
  const c = TONE[tone];
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
          <div
            className={cn(
              'flex items-center gap-2 rounded-lg border px-3 py-2 text-[11px]',
              c.box,
              className
            )}
          >
            <Icon size={12} className={cn('shrink-0', c.icon)} />
            <span>{message}</span>
            {actionLabel && onAction && (
              <button
                type="button"
                disabled={pending}
                onClick={onAction}
                className="ml-auto shrink-0 text-brand-soft underline-offset-2 hover:underline disabled:opacity-50"
              >
                {pending ? <Loader2 size={11} className="animate-spin" /> : actionLabel}
              </button>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
