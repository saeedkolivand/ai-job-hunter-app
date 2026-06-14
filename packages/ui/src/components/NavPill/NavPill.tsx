import { motion } from 'motion/react';

import { cn } from '../../lib/cn';
import { transition } from '../../lib/motion';

export interface NavPillProps {
  /**
   * Shared-layout animation id — **must be unique per nav list** so the pill
   * slides between rows of the same list and never animates across lists.
   */
  layoutId: string;
  /** Extra classes merged onto the pill (e.g. a different corner radius). */
  className?: string;
}

/**
 * Animated active-row indicator for vertical nav lists (app sidebar, settings
 * sidebar). Render it conditionally — only for the active row — as the first
 * child of a `relative` wrapper, with the clickable row as its sibling above.
 * The accent glass pill slides between rows via motion's shared-layout
 * animation keyed on {@link NavPillProps.layoutId}. Decorative: the active
 * state is conveyed to assistive tech by `aria-current` on the row itself.
 */
export function NavPill({ layoutId, className }: NavPillProps) {
  return (
    <motion.div
      aria-hidden
      layoutId={layoutId}
      className={cn('pointer-events-none absolute inset-0 rounded-xl', className)}
      style={{
        background:
          'linear-gradient(135deg, color-mix(in srgb, var(--color-brand) 18%, transparent) 0%, color-mix(in srgb, var(--color-brand-2) 10%, transparent) 100%)',
        border: '1px solid color-mix(in srgb, var(--color-brand) 25%, transparent)',
        boxShadow: '0 0 16px color-mix(in srgb, var(--color-brand) 12%, transparent)',
      }}
      transition={transition.spring}
    />
  );
}
