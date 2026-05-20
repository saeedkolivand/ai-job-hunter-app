import { createPortal } from 'react-dom';
import { useEffect, type ReactNode } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { GlassOverlay } from './GlassOverlay';
import { useFocusTrap } from '../hooks/use-focus-trap';
import { variants, transition } from '../lib/motion';
import { cn } from '../lib/cn';

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

  return createPortal(
    <AnimatePresence>
      {open && (
        <>
          <GlassOverlay onClick={onClose} zIndex={zIndex - 1} />
          <motion.div
            className="pointer-events-none fixed inset-0 flex items-center justify-center p-4"
            style={{ zIndex }}
            {...variants.overlay}
            transition={transition.overlay}
          >
            <motion.div
              ref={trapRef as React.RefObject<HTMLDivElement>}
              role="dialog"
              aria-modal="true"
              className={cn(
                'glass-modal pointer-events-auto relative w-full overflow-hidden rounded-2xl border shadow-xl',
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
