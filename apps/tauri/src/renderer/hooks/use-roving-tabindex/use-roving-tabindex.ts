import type { KeyboardEvent, MutableRefObject } from 'react';

/**
 * Returns an `onKeyDown` handler implementing the APG roving-tabindex pattern
 * for a radiogroup:
 *  - ArrowRight / ArrowDown — advance (wraps)
 *  - ArrowLeft  / ArrowUp   — retreat (wraps)
 *  - Home                   — first item
 *  - End                    — last item
 *
 * After moving selection the handler BOTH calls `onChange(newValue)` AND
 * imperatively focuses `refs.current[newIdx]` so the active DOM element always
 * matches the selected item (WCAG 2.4.7 / APG roving-tabindex contract).
 *
 * This is a plain factory (no React hooks inside) — call it at render time or
 * inside a `useCallback`. Named with a `make` prefix deliberately: the `use`
 * prefix is reserved for actual React hooks.
 */
export function makeRovingTabindex<T>(
  items: readonly T[],
  currentValue: T,
  onChange: (v: T) => void,
  refs: MutableRefObject<(HTMLButtonElement | null)[]>
) {
  return (e: KeyboardEvent<HTMLElement>) => {
    const idx = items.indexOf(currentValue);
    let nextIdx: number | null = null;

    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      nextIdx = (idx + 1) % items.length;
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      nextIdx = (idx - 1 + items.length) % items.length;
    } else if (e.key === 'Home') {
      nextIdx = 0;
    } else if (e.key === 'End') {
      nextIdx = items.length - 1;
    }

    if (nextIdx !== null) {
      e.preventDefault();
      const next = items[nextIdx];
      if (next !== undefined) {
        onChange(next);
        refs.current[nextIdx]?.focus();
      }
    }
  };
}
