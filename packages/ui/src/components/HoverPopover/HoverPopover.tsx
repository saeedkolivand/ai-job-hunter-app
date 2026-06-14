import { AnimatePresence, motion } from 'motion/react';
import {
  type KeyboardEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';

import { cn } from '../../lib/cn';
import { transition, variants } from '../../lib/motion';

export interface HoverPopoverProps {
  /** The always-rendered trigger (the hoverable/focusable element). */
  trigger: ReactNode;
  /** Popover panel content. */
  children: ReactNode;
  /** Which side of the trigger the panel opens on. */
  placement?: 'top' | 'bottom';
  /** ms before closing after pointer/focus leaves. */
  closeDelay?: number;
  /** Accessible label for the panel. */
  ariaLabel?: string;
  /** Class for the trigger wrapper. */
  className?: string;
  /** Class for the floating panel. */
  contentClassName?: string;
}

/**
 * Generic hover/focus popover.
 *
 * Mechanics it owns (so call sites don't reimplement them):
 *  - Opens on `mouseenter` / `focus`. While open, hover keep-open/close is
 *    GEOMETRY-BASED: a document `pointermove` listener keeps it open whenever the
 *    pointer is over the trigger OR the panel (each inflated by an 8px bridge pad)
 *    and otherwise schedules close after `closeDelay` ms. Geometry-based because when
 *    `placement='top'` the panel is portalled directly under the descending cursor and
 *    a synthetic `mouseenter` never fires for an element inserted under an
 *    already-present pointer (the insertion-under-cursor race), so mouseenter/leave
 *    on the panel can't be trusted. A document `pointerleave` schedules close when the
 *    cursor leaves the window. Esc / `blur` still close.
 *  - Panel is portalled to `document.body` and positioned against the trigger via
 *    `getBoundingClientRect` (re-measured on scroll/resize), opening upward for
 *    `placement='top'` and downward for `placement='bottom'`.
 *  - Panel gets `role="tooltip"` + a generated id wired to the trigger wrapper's
 *    `aria-describedby`; the wrapper also exposes `aria-expanded`. No focus trap —
 *    the trigger keeps focus, matching tooltip semantics.
 *
 * Content (job list, links, etc.) stays at the call site.
 */
export function HoverPopover({
  trigger,
  children,
  placement = 'top',
  closeDelay = 120,
  ariaLabel,
  className,
  contentClassName,
}: HoverPopoverProps) {
  const [open, setOpen] = useState(false);
  const [rect, setRect] = useState<DOMRect | null>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const rafId = useRef<number | null>(null);
  const popoverId = useId();

  const cancelClose = useCallback(() => {
    if (closeTimer.current) clearTimeout(closeTimer.current);
  }, []);

  const scheduleClose = useCallback(() => {
    cancelClose();
    closeTimer.current = setTimeout(() => setOpen(false), closeDelay);
  }, [cancelClose, closeDelay]);

  const handleOpen = useCallback(() => {
    cancelClose();
    setOpen(true);
  }, [cancelClose]);

  const handleKeyDown = useCallback((e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') setOpen(false);
  }, []);

  // Clear any pending timer on unmount.
  useEffect(() => () => cancelClose(), [cancelClose]);

  // Measure the trigger while open, re-measuring on scroll/resize.
  useEffect(() => {
    if (!open) return;
    const measure = () => {
      if (wrapperRef.current) setRect(wrapperRef.current.getBoundingClientRect());
    };
    measure();
    window.addEventListener('scroll', measure, true);
    window.addEventListener('resize', measure);
    return () => {
      window.removeEventListener('scroll', measure, true);
      window.removeEventListener('resize', measure);
    };
  }, [open]);

  // Geometry-based hover tracking: while open, a document `pointermove` listener is
  // the SOLE authority for hover-close. It keeps the popover open whenever the pointer
  // is over the trigger OR the panel (each inflated by an 8px bridge pad) and otherwise
  // schedules close. This is immune to the synthetic `mouseenter` never firing for the
  // panel when `placement='top'` portals it directly under the descending cursor. A
  // document `pointerleave` schedules close when the cursor leaves the window.
  useEffect(() => {
    if (!open) return;
    const PAD = 8;
    const inside = (bounds: DOMRect | null, x: number, y: number) =>
      bounds !== null &&
      x >= bounds.left - PAD &&
      x <= bounds.right + PAD &&
      y >= bounds.top - PAD &&
      y <= bounds.bottom + PAD;
    const onPointerMove = (e: PointerEvent) => {
      const { clientX, clientY } = e;
      if (rafId.current !== null) cancelAnimationFrame(rafId.current);
      rafId.current = requestAnimationFrame(() => {
        rafId.current = null;
        const triggerRect = wrapperRef.current?.getBoundingClientRect() ?? null;
        const panelRect = panelRef.current?.getBoundingClientRect() ?? null;
        if (inside(triggerRect, clientX, clientY) || inside(panelRect, clientX, clientY)) {
          cancelClose();
        } else {
          scheduleClose();
        }
      });
    };
    const onPointerLeave = () => scheduleClose();
    document.addEventListener('pointermove', onPointerMove);
    document.addEventListener('pointerleave', onPointerLeave);
    return () => {
      document.removeEventListener('pointermove', onPointerMove);
      document.removeEventListener('pointerleave', onPointerLeave);
      if (rafId.current !== null) {
        cancelAnimationFrame(rafId.current);
        rafId.current = null;
      }
    };
  }, [open, cancelClose, scheduleClose]);

  const panelStyle: React.CSSProperties = rect
    ? {
        position: 'fixed',
        left: rect.left,
        zIndex: 9999,
        ...(placement === 'top'
          ? { bottom: window.innerHeight - rect.top, paddingBottom: 8 }
          : { top: rect.bottom, paddingTop: 8 }),
      }
    : { display: 'none' };

  return (
    <div
      ref={wrapperRef}
      className={cn('relative', className)}
      aria-expanded={open}
      aria-describedby={open ? popoverId : undefined}
      onMouseEnter={handleOpen}
      onFocus={handleOpen}
      onBlur={scheduleClose}
      onKeyDown={handleKeyDown}
    >
      {trigger}

      {createPortal(
        <AnimatePresence>
          {open && (
            <motion.div
              ref={panelRef}
              id={popoverId}
              role="tooltip"
              aria-label={ariaLabel}
              {...(placement === 'top' ? variants.fadeSlideUp : variants.fadeSlideDown)}
              transition={transition.fast}
              style={panelStyle}
            >
              <div className={contentClassName}>{children}</div>
            </motion.div>
          )}
        </AnimatePresence>,
        document.body
      )}
    </div>
  );
}
