import { useEffect, useRef } from 'react';

const FOCUSABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

/**
 * Traps keyboard focus inside `containerRef` while `active` is true.
 * Returns the ref to attach to the container element.
 */
export function useFocusTrap(active: boolean) {
  const containerRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!active || !containerRef.current) return;

    const container = containerRef.current;

    // Queried fresh on every Tab/Shift-Tab below (not captured once here) —
    // the trapped content can change shape while open (a loading state
    // swapping for content, a streaming panel adding/removing rows, …), so a
    // focusable-element snapshot taken once at open time can go stale and let
    // Tab escape the dialog once the originally first/last element unmounts.
    const getFocusable = () => Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE));

    // Auto-focus first focusable element
    getFocusable()[0]?.focus();

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      const focusable = getFocusable();
      if (focusable.length === 0) {
        e.preventDefault();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];

      if (e.shiftKey) {
        if (document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        }
      } else {
        if (document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };

    container.addEventListener('keydown', onKeyDown);
    return () => container.removeEventListener('keydown', onKeyDown);
  }, [active]);

  return containerRef;
}
