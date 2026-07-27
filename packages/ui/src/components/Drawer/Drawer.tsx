import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';

import { useFocusTrap } from '../../hooks/use-focus-trap';
import { cn } from '../../lib/cn';
import { resolveTransition, transition, variants } from '../../lib/motion';
import { GlassOverlay } from '../GlassOverlay';

export interface DrawerProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  /** Accessible name when there is no visible title element to reference. */
  ariaLabel?: string;
  /** id of the element labelling the dialog (wired to `aria-labelledby`). */
  ariaLabelledby?: string;
  /**
   * Width of the panel. Defaults to a clamped sheet that never exceeds the
   * window minus a gutter, so it stays usable at the 900×600 window floor.
   */
  widthClass?: string;
  /** Extra classes forwarded to the panel element. */
  className?: string;
  /** z-index layer — default 600 (`--z-modal`), matching {@link ModalShell}. */
  zIndex?: number;
  /** When false, a backdrop click does NOT close the drawer. Default `true`. */
  closeOnBackdrop?: boolean;
}

/**
 * Right-edge slide-over panel (drawer / sheet) — the lateral sibling of
 * `ModalShell`: same portal + scrim + focus trap + Escape contract, but the
 * panel is pinned full-height to the right edge instead of centred.
 *
 * Use it for a task surface the user edits and then applies (a filter/search
 * form), where the content behind should stay legible; use `ModalShell` for a
 * decision that must be answered before continuing.
 *
 * Motion respects `prefers-reduced-motion`: the horizontal travel is dropped
 * (opacity only) rather than merely shortened, so there is no positional jump.
 */
export function Drawer({
  open,
  onClose,
  children,
  ariaLabel,
  ariaLabelledby,
  widthClass = 'w-[30rem] max-w-[calc(100vw-5rem)]',
  className,
  zIndex = 600,
  closeOnBackdrop = true,
}: DrawerProps) {
  // Focus return (WCAG 2.4.3 Focus Order). MUST be declared BEFORE useFocusTrap:
  // effects run in hook-call order, so this one still sees the opener as
  // `activeElement`; the trap's effect moves focus into the panel right after.
  const openerRef = useRef<HTMLElement | null>(null);
  useEffect(() => {
    if (!open) return;
    openerRef.current = document.activeElement as HTMLElement | null;
    return () => {
      const opener = openerRef.current;
      openerRef.current = null;
      // The opener can be gone by the time the drawer closes (a route change,
      // a conditionally-rendered trigger) — only refocus something still live.
      if (opener && document.contains(opener)) opener.focus();
    };
  }, [open]);

  const trapRef = useFocusTrap(open);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  const resolved = resolveTransition(transition.normal);
  const isInstant = resolved.duration === 0;

  return createPortal(
    <AnimatePresence>
      {open && (
        <>
          <GlassOverlay zIndex={zIndex - 1} onClick={closeOnBackdrop ? onClose : undefined} />
          <motion.div
            ref={trapRef as React.RefObject<HTMLDivElement>}
            role="dialog"
            aria-modal="true"
            aria-label={ariaLabel}
            aria-labelledby={ariaLabelledby}
            className={cn(
              'glass-modal fixed inset-y-0 right-0 flex flex-col overflow-hidden border-l border-white/[0.12] shadow-xl',
              widthClass,
              className
            )}
            style={{ zIndex }}
            {...(isInstant ? variants.overlay : variants.slideOverRight)}
            transition={resolved}
          >
            {children}
          </motion.div>
        </>
      )}
    </AnimatePresence>,
    document.body
  );
}
