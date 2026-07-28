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
 * Traps keyboard focus inside `containerRef` while `active` is true, and hands
 * focus back to whatever had it before the trap opened.
 *
 * Return-focus is a FALLBACK, not an override: a wrapper that owns its own
 * return target (e.g. a Drawer's `returnFocusTo`) runs its cleanup around this
 * one, so we only restore when focus was left orphaned — on `<body>`, on nothing,
 * or still inside the container being torn down. If something already moved focus
 * somewhere meaningful, or the remembered element has left the DOM, we do nothing.
 */
export function useFocusTrap(active: boolean) {
  const containerRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!active || !containerRef.current) return;

    const container = containerRef.current;

    // Captured before the auto-focus below moves it into the trap. Without this,
    // Escape/Cancel dropped focus on <body> and a keyboard user restarted from
    // the top of the document instead of the control they opened the dialog with.
    const previouslyFocused =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;

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
    return () => {
      container.removeEventListener('keydown', onKeyDown);
      const current = document.activeElement;
      const orphaned = current === null || current === document.body || container.contains(current);
      if (orphaned && previouslyFocused?.isConnected) previouslyFocused.focus();
    };
  }, [active]);

  return containerRef;
}
