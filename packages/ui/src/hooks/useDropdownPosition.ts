import { useCallback, useEffect, useState } from 'react';

export function useDropdownPosition(
  open: boolean,
  triggerRef: React.RefObject<HTMLButtonElement | null>
) {
  const [rect, setRect] = useState<DOMRect | null>(null);

  const measure = useCallback(() => {
    if (triggerRef.current) setRect(triggerRef.current.getBoundingClientRect());
  }, [triggerRef]);

  useEffect(() => {
    if (!open) return;
    measure();
    window.addEventListener('scroll', measure, true);
    window.addEventListener('resize', measure);
    return () => {
      window.removeEventListener('scroll', measure, true);
      window.removeEventListener('resize', measure);
    };
  }, [open, measure]);

  // Flip up if not enough space below
  const spaceBelow = rect ? window.innerHeight - rect.bottom : 999;
  const dropUp = spaceBelow < 220;

  const dropdownStyle: React.CSSProperties = rect
    ? (() => {
        const maxWidth = Math.min(420, window.innerWidth - 16);
        // Anchor the panel under its trigger. The panel is `max-content` wide, so
        // its rendered width is unknown here — when the trigger sits near the right
        // edge, anchor by CSS `right` (to the trigger's right edge) and let the
        // panel grow leftward; otherwise anchor `left`. This keeps the panel under
        // its trigger in both cases without guessing the width. (The old
        // `left = rect.right - maxWidth` assumed a `maxWidth`-wide panel and placed
        // a narrow one far to the left of the trigger.)
        const nearRight = rect.left + maxWidth > window.innerWidth - 8;
        const horizontal = nearRight
          ? { right: Math.max(8, window.innerWidth - rect.right) }
          : { left: rect.left };
        return {
          position: 'fixed',
          ...horizontal,
          width: 'max-content',
          minWidth: Math.min(rect.width, maxWidth),
          maxWidth,
          zIndex: 9999,
          ...(dropUp ? { bottom: window.innerHeight - rect.top + 4 } : { top: rect.bottom + 4 }),
        };
      })()
    : { display: 'none' };

  return { rect, dropUp, dropdownStyle };
}
