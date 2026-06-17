import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useEffect } from 'react';
import { createPortal } from 'react-dom';

import { useFocusTrap } from '../../hooks/use-focus-trap';
import { cn } from '../../lib/cn';
import { transition, variants } from '../../lib/motion';
import { GlassOverlay } from '../GlassOverlay';

/**
 * Number of currently-open ModalShells. WebView2 does not reliably composite the
 * portaled overlay's `backdrop-filter`, so the frosted-glass effect comes from
 * blurring the in-flow app shell instead (see the `.modal-blur-active` rule in
 * the app CSS). The class is ref-counted so stacked modals keep the blur until
 * the last one closes. Toggling a body class keeps this primitive app-agnostic.
 */
let openModalCount = 0;

function setModalBlur(active: boolean): void {
  if (typeof document === 'undefined') return;
  if (active) {
    openModalCount += 1;
    document.body.classList.add('modal-blur-active');
  } else {
    openModalCount = Math.max(0, openModalCount - 1);
    if (openModalCount === 0) document.body.classList.remove('modal-blur-active');
  }
}

export interface ModalShellProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  /** Optional pinned header region (does not scroll). */
  header?: ReactNode;
  /** Optional pinned footer region (does not scroll). */
  footer?: ReactNode;
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
  header,
  footer,
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

  // Frost the app shell behind the modal. GlassOverlay supplies the dim scrim,
  // but its `backdrop-filter` blur is unreliable across WebView2 portal stacking
  // contexts, so the actual blur is an in-flow `filter` on the app shell driven
  // by this body class (see `.modal-blur-active` in the app CSS). Ref-counted via
  // `setModalBlur` so stacked modals don't clear it early.
  useEffect(() => {
    if (!open) return;
    setModalBlur(true);
    return () => setModalBlur(false);
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
              aria-labelledby={ariaLabelledby}
              aria-label={ariaLabel}
              className={cn(
                'glass-modal relative flex max-h-[calc(100vh-2rem)] w-full flex-col overflow-hidden rounded-2xl border shadow-xl',
                maxWidth,
                borderClass ?? 'border-white/[0.12]',
                className
              )}
              {...variants.scale}
              transition={transition.modal}
              onClick={(e) => e.stopPropagation()}
            >
              {header && <div className="shrink-0">{header}</div>}
              <div className="@container min-h-0 flex-1 overflow-y-auto">{children}</div>
              {footer && <div className="shrink-0">{footer}</div>}
            </motion.div>
          </motion.div>
        </>
      )}
    </AnimatePresence>,
    document.body
  );
}
