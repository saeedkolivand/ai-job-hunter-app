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
    ? {
        position: 'fixed',
        left: rect.left,
        width: rect.width,
        zIndex: 9999,
        ...(dropUp ? { bottom: window.innerHeight - rect.top + 4 } : { top: rect.bottom + 4 }),
      }
    : { display: 'none' };

  return { rect, dropUp, dropdownStyle };
}
