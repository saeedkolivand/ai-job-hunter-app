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

/**
 * Returns an `onKeyDown` handler implementing the APG roving-tabindex pattern
 * for a **multi-select** toolbar/group (e.g. a set of toggle buttons):
 *
 *  - ArrowRight / ArrowDown — move focus forward (wraps), NO toggle
 *  - ArrowLeft  / ArrowUp   — move focus backward (wraps), NO toggle
 *  - Home                   — move focus to first item, NO toggle
 *  - End                    — move focus to last item, NO toggle
 *  - Space / Enter          — toggle membership of the currently-focused item
 *
 * Focus position is tracked in `focusedIdxRef` (a plain `useRef<number>`),
 * independent of the selection set. Exactly ONE button should have `tabIndex=0`
 * (the one at `focusedIdxRef.current`); all others get `tabIndex=-1`.
 *
 * Callers must:
 *   1. Pass a stable `focusedIdxRef` (e.g. `useRef(0)`).
 *   2. Also update `focusedIdxRef.current` on click (so mouse + keyboard stay in sync).
 *   3. Render `tabIndex={i === focusedIdxRef.current ? 0 : -1}` on each button.
 */
export function makeMultiSelectKeyHandler(
  length: number,
  focusedIdxRef: MutableRefObject<number>,
  refs: MutableRefObject<(HTMLButtonElement | null)[]>,
  onToggle: (idx: number) => void
) {
  return (e: KeyboardEvent<HTMLElement>) => {
    if (length <= 0) return;
    const current = focusedIdxRef.current;
    let nextIdx: number | null = null;
    let shouldToggle = false;

    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      nextIdx = (current + 1) % length;
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      nextIdx = (current - 1 + length) % length;
    } else if (e.key === 'Home') {
      nextIdx = 0;
    } else if (e.key === 'End') {
      nextIdx = length - 1;
    } else if (e.key === ' ' || e.key === 'Enter') {
      shouldToggle = true;
    }

    if (nextIdx !== null) {
      e.preventDefault();
      focusedIdxRef.current = nextIdx;
      refs.current[nextIdx]?.focus();
    } else if (shouldToggle) {
      e.preventDefault();
      onToggle(current);
    }
  };
}
