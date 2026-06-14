import { ChevronDown, type LucideIcon } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import type { ReactNode } from 'react';

import { Button, cn, transition } from '@ajh/ui';

interface SectionProps {
  /** Header label. */
  label: string;
  /** Leading icon shown before the label. */
  icon: LucideIcon;
  /** Whether this section is currently expanded (single-expand group). */
  open: boolean;
  onToggle: () => void;
  /** Optional count pill shown after the label (e.g. number of answers). */
  badge?: number;
  children: ReactNode;
}

/**
 * One collapsible row inside the generation card body. Encapsulates the
 * height-animation boilerplate that used to be hand-rolled per section, while
 * preserving the card's single-expand-group behavior, the leading icon, and the
 * optional count badge.
 *
 * NOTE: this is intentionally NOT the `@ajh/ui` Accordion. That primitive is
 * self-contained (uncontrolled internal `open` state, string-only title, no
 * icon/badge slot, surface-card chrome), so it cannot express this card's
 * single-expand discriminant or its icon+badge headers without a behavior/visual
 * regression. This local section keeps the existing UX exactly while removing the
 * duplicated AnimatePresence + height-animation blocks.
 */
export function Section({ label, icon: Icon, open, onToggle, badge, children }: SectionProps) {
  return (
    <div className="border-t border-white/[0.04]">
      <Button
        variant="unstyled"
        onClick={onToggle}
        aria-expanded={open}
        className="flex w-full items-center justify-between px-5 py-3 text-left text-xs font-medium text-foreground/55 transition-colors hover:text-foreground/80"
      >
        <span className="flex items-center gap-2">
          <Icon size={12} /> {label}
          {badge !== undefined && (
            <span className="rounded-full bg-white/[0.06] px-1.5 py-0.5 text-[9px] text-foreground/45">
              {badge}
            </span>
          )}
        </span>
        <ChevronDown size={13} className={cn('transition-transform', open && 'rotate-180')} />
      </Button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0 }}
            animate={{ height: 'auto' }}
            exit={{ height: 0 }}
            transition={transition.normal}
            className="overflow-hidden"
          >
            {children}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
