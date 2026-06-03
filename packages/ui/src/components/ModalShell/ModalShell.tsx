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
  /** id of the element labelling the dialog (wired to aria-labelledby). */
  ariaLabelledby?: string;
  /** Accessible name when there is no visible title element to reference. */
  ariaLabel?: string;
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
  ariaLabelledby,
  ariaLabel,
}: ModalShellProps) {
  const trapRef = useFocusTrap(open);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  // The blur + dim behind the modal comes from GlassOverlay (fixed-overlay
  // backdrop-blur), which only composites the area behind it — far cheaper than
  // filtering the whole app subtree, so we no longer toggle a body attribute.

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
              aria-labelledby={ariaLabelledby}
              aria-label={ariaLabel}
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
