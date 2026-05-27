import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useEffect } from 'react';
import { createPortal } from 'react-dom';

import { useFocusTrap } from '../../hooks/use-focus-trap';
import { cn } from '../../lib/cn';
import { transition, variants } from '../../lib/motion';
import { GlassOverlay } from '../GlassOverlay';

export interface ModalShellProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  /** Tailwind max-width class — default max-w-md */
  maxWidth?: string;
  /** Extra classes forwarded to the panel element */
  className?: string;
  /** z-index layer — default 600 (--z-modal) */
  zIndex?: number;
  /** Border color class applied to the panel — e.g. "border-red-500/30" */
  borderClass?: string;
}

/**
 * Shared modal container: overlay + glass panel + focus trap + Escape key.
 * All dialogs (ConfirmModal, etc.) compose from this rather than rebuilding.
 */
export function ModalShell({
  open,
  onClose,
  children,
  maxWidth = 'max-w-md',
  className,
  zIndex = 600,
  borderClass,
}: ModalShellProps) {
  const trapRef = useFocusTrap(open);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  // Blur the app content behind the modal by toggling a body attribute.
  // CSS in globals.css targets [data-modal-open] .app-content to apply filter:blur().
  useEffect(() => {
    if (!open) return;
    const prev = document.body.dataset.modalDepth;
    const depth = (parseInt(prev ?? '0', 10) || 0) + 1;
    document.body.dataset.modalDepth = String(depth);
    document.body.setAttribute('data-modal-open', '');
    return () => {
      const next = (parseInt(document.body.dataset.modalDepth ?? '1', 10) || 1) - 1;
      document.body.dataset.modalDepth = String(next);
      if (next <= 0) document.body.removeAttribute('data-modal-open');
    };
  }, [open]);

  return createPortal(
    <AnimatePresence>
      {open && (
        <>
          {/* Visual backdrop — no click handler; the outer container handles dismissal */}
          <GlassOverlay zIndex={zIndex - 1} />
          {/* Click on the backdrop area (outside the panel) closes the modal.
              Click on the panel calls stopPropagation so it never reaches here. */}
          <motion.div
            className="fixed inset-0 flex items-center justify-center p-4"
            style={{ zIndex }}
            onClick={onClose}
            {...variants.overlay}
            transition={transition.overlay}
          >
            <motion.div
              ref={trapRef as React.RefObject<HTMLDivElement>}
              role="dialog"
              aria-modal="true"
              className={cn(
                'glass-modal relative w-full overflow-hidden rounded-2xl border shadow-xl',
                maxWidth,
                borderClass ?? 'border-white/[0.12]',
                className
              )}
              {...variants.scale}
              transition={transition.modal}
              onClick={(e) => e.stopPropagation()}
            >
              {children}
            </motion.div>
          </motion.div>
        </>
      )}
    </AnimatePresence>,
    document.body
  );
}
