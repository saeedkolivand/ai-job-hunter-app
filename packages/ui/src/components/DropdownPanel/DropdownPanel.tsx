import { AnimatePresence, motion } from 'motion/react';
import type { CSSProperties, ReactNode, Ref } from 'react';
import { createPortal } from 'react-dom';

import { transition } from '../../lib/motion';

interface DropdownPanelProps {
  open: boolean;
  /** Computed fixed-position style from useDropdownPosition. */
  style: CSSProperties;
  /** Drop-up flips the entrance offset so it grows from the trigger edge. */
  dropUp?: boolean;
  panelRef?: Ref<HTMLDivElement>;
  role?: 'listbox' | 'menu';
  'aria-labelledby'?: string;
  'aria-label'?: string;
  onKeyDown?: (e: React.KeyboardEvent) => void;
  children: ReactNode;
}

/**
 * Shared portal surface for every dropdown menu — the glass panel, the
 * open/exit animation, and viewport positioning. Consumers render their
 * search box + option list as `children`.
 */
export function DropdownPanel({
  open,
  style,
  dropUp,
  panelRef,
  role = 'listbox',
  'aria-labelledby': ariaLabelledby,
  'aria-label': ariaLabel,
  onKeyDown,
  children,
}: DropdownPanelProps) {
  const offset = dropUp ? 4 : -4;
  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          ref={panelRef}
          role={role}
          aria-labelledby={ariaLabelledby}
          aria-label={ariaLabel}
          initial={{ opacity: 0, y: offset, scale: 0.985 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: offset, scale: 0.985 }}
          transition={transition.fast}
          style={style}
          onKeyDown={onKeyDown}
          className="dropdown-surface overflow-hidden rounded-xl"
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>,
    document.body
  );
}
