import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';

import { useFocusTrap } from '../../hooks/use-focus-trap';
import { cn } from '../../lib/cn';
import { setModalBlur } from '../../lib/modal-blur';
import { prefersReducedMotion, transition, variants } from '../../lib/motion';
import { GlassOverlay } from '../GlassOverlay';

/**
 * Entrance/exit variants for the panel, as a pure function of the user's motion
 * preference — exported so the reduced-motion branch is unit-testable without
 * driving `matchMedia` through a rendered tree.
 *
 * Under `reduce` the horizontal travel is dropped ENTIRELY (opacity only)
 * rather than merely shortened, so there is no positional jump.
 */
export function drawerVariants(reduced: boolean) {
  return reduced ? variants.overlay : variants.slideOverRight;
}

/** Panel transition, likewise a pure function of the motion preference. */
export function drawerTransition(reduced: boolean) {
  // `relaxed` (220ms), not `normal` (180ms) — the panel travels its full width,
  // and a short duration over a long distance reads as a snap, not a slide.
  return reduced ? transition.instant : transition.relaxed;
}

export interface DrawerProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  /** Accessible name when there is no visible title element to reference. */
  ariaLabel?: string;
  /** id of the element labelling the dialog (wired to `aria-labelledby`). */
  ariaLabelledby?: string;
  /**
   * Where focus goes on close when the control that OPENED the drawer no longer
   * exists — e.g. an empty-state CTA that the drawer's own action replaced.
   * Point this at a control that is always mounted (typically the persistent
   * trigger for the same drawer); without it focus would fall to `<body>`.
   */
  returnFocusTo?: React.RefObject<HTMLElement | null>;
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
 * `ModalShell`: same portal + scrim + app-shell frosting + focus trap + Escape
 * contract, but the panel is pinned full-height to the right edge instead of
 * centred.
 *
 * Use it for a task surface the user edits and then applies (a filter/search
 * form), where the content behind should stay legible; use `ModalShell` for a
 * decision that must be answered before continuing.
 *
 * The panel is a flex column the height of the viewport, so content that needs
 * pinned chrome renders its own `shrink-0` header/footer around a
 * `min-h-0 flex-1 overflow-y-auto` body (see `ScrapeForm`) — nothing scrolls
 * out of reach.
 */
export function Drawer({
  open,
  onClose,
  children,
  ariaLabel,
  ariaLabelledby,
  returnFocusTo,
  widthClass = 'w-[30rem] max-w-[calc(100vw-5rem)]',
  className,
  zIndex = 600,
  closeOnBackdrop = true,
}: DrawerProps) {
  // Focus return (WCAG 2.4.3 Focus Order). MUST be declared BEFORE useFocusTrap:
  // effects run in hook-call order, so this one still sees the opener as
  // `activeElement`; the trap's effect moves focus into the panel right after.
  const openerRef = useRef<HTMLElement | null>(null);
  // Mirrored so the cleanup below reads the CURRENT fallback without making the
  // effect re-run (and re-capture the opener) whenever the prop identity changes.
  const fallbackRef = useRef(returnFocusTo);
  fallbackRef.current = returnFocusTo;
  useEffect(() => {
    if (!open) return;
    openerRef.current = document.activeElement as HTMLElement | null;
    return () => {
      const opener = openerRef.current;
      openerRef.current = null;
      // The opener can be gone by the time the drawer closes — a route change, or
      // a conditionally-rendered trigger the drawer's own action replaced. Only
      // refocus something still in the document, else fall back to the caller's
      // always-mounted trigger; never leave focus on <body>.
      // `<body>` passes `contains` but is not a focus target — it is what
      // `activeElement` degrades to when the focused node is removed, so
      // accepting it would silently swallow the fallback.
      const live = (el: HTMLElement | null | undefined) =>
        el && el !== document.body && document.contains(el) ? el : null;
      (live(opener) ?? live(fallbackRef.current?.current))?.focus();
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

  // Frost the app shell behind the drawer — see `lib/modal-blur` for why the
  // blur can't live on the portaled overlay itself under WebView2.
  useEffect(() => {
    if (!open) return;
    setModalBlur(true);
    return () => setModalBlur(false);
  }, [open]);

  const reduced = prefersReducedMotion();

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
            {...drawerVariants(reduced)}
            transition={drawerTransition(reduced)}
          >
            {children}
          </motion.div>
        </>
      )}
    </AnimatePresence>,
    document.body
  );
}
