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
 *  - Opens on `mouseenter` / `focus`, closes on `mouseleave` / `blur` after
 *    `closeDelay` ms (debounced so moving the pointer onto the panel keeps it open).
 *  - Esc closes immediately.
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
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
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
      onMouseLeave={scheduleClose}
      onFocus={handleOpen}
      onBlur={scheduleClose}
      onKeyDown={handleKeyDown}
    >
      {trigger}

      {createPortal(
        <AnimatePresence>
          {open && (
            <motion.div
              id={popoverId}
              role="tooltip"
              aria-label={ariaLabel}
              {...(placement === 'top' ? variants.fadeSlideUp : variants.fadeSlideDown)}
              transition={transition.fast}
              style={panelStyle}
              onMouseEnter={cancelClose}
              onMouseLeave={scheduleClose}
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
