import { useEffect, useRef } from 'react';

import { ROUTES } from '@/constants/routes/routes';

export type AppRoute = (typeof ROUTES)[keyof typeof ROUTES];

/** `g`-prefixed route jumps: press `g`, then the letter, within ~1.2s. */
const GO_TO: Record<string, AppRoute> = {
  d: ROUTES.DASHBOARD,
  a: ROUTES.ANALYZE,
  g: ROUTES.GENERATE,
  j: ROUTES.JOBS,
  p: ROUTES.AUTOPILOT,
  r: ROUTES.RESUMES,
  m: ROUTES.MONITORING,
  i: ROUTES.AI,
  s: ROUTES.SETTINGS,
};

/** Don't hijack keystrokes the user is typing into a field. */
function isTypingTarget(el: EventTarget | null): boolean {
  return (
    el instanceof HTMLElement &&
    (el instanceof HTMLInputElement ||
      el instanceof HTMLTextAreaElement ||
      el.isContentEditable ||
      el.getAttribute('role') === 'textbox')
  );
}

interface Options {
  onNavigate: (to: AppRoute) => void;
  onToggleHelp: () => void;
}

/**
 * Global keyboard shortcuts (mounted once, app-wide):
 *   - ⌘/Ctrl+K → Search, ⌘/Ctrl+, → Settings (work even from a field)
 *   - `?` → toggle the shortcuts cheat-sheet
 *   - `g` then d/a/g/j/p/r/m/i/s → jump to that route
 * Intentionally NOT a command palette (out of scope).
 */
export function useKeyboardShortcuts({ onNavigate, onToggleHelp }: Options) {
  // Keep the latest callbacks in a ref so the listener registers exactly once.
  const cbs = useRef({ onNavigate, onToggleHelp });
  cbs.current = { onNavigate, onToggleHelp };

  const pendingG = useRef(false);
  const gTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const clearG = () => {
      pendingG.current = false;
      if (gTimer.current) clearTimeout(gTimer.current);
      gTimer.current = null;
    };

    const handler = (e: KeyboardEvent) => {
      const { onNavigate, onToggleHelp } = cbs.current;
      const mod = e.ctrlKey || e.metaKey;

      // Modifier combos fire from anywhere, including inputs.
      if (mod && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        onNavigate(ROUTES.SEARCH);
        return;
      }
      if (mod && e.key === ',') {
        e.preventDefault();
        onNavigate(ROUTES.SETTINGS);
        return;
      }

      // Single-key shortcuts never fire while typing or with a modifier held.
      if (mod || e.altKey || isTypingTarget(e.target)) return;

      if (e.key === '?') {
        e.preventDefault();
        onToggleHelp();
        return;
      }

      if (pendingG.current) {
        const to = GO_TO[e.key.toLowerCase()];
        clearG();
        if (to) {
          e.preventDefault();
          onNavigate(to);
        }
        return;
      }
      if (e.key.toLowerCase() === 'g') {
        pendingG.current = true;
        gTimer.current = setTimeout(clearG, 1200);
      }
    };

    window.addEventListener('keydown', handler);
    return () => {
      window.removeEventListener('keydown', handler);
      clearG();
    };
  }, []);
}
