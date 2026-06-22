import { useCallback, useEffect, useRef } from 'react';

const DEBOUNCE_MS = 700;

/**
 * Debounces LOCAL edits (user typing) ~700 ms before calling `onCommit`, while
 * letting EXTERNAL changes (generation / regeneration) skip the debounce entirely.
 *
 * Contract:
 * - Call `scheduleCommit(value)` on every local-edit onChange.
 * - `flush(value)` commits immediately and cancels any pending debounce — call on
 *   blur and on doc/tab switch.
 * - `cancel()` drops any pending debounce without committing — call on unmount or
 *   when the caller switches away and wants to discard the pending edit.
 *
 * The hook is stateless — it does not hold the committed value itself; the caller
 * owns state and passes `onCommit` to receive updates.
 */
export function useDebouncedCommit(onCommit: (value: string) => void) {
  // Keep a stable ref so the timeout closure always sees the latest callback
  // without re-creating the timer functions when onCommit changes identity.
  const onCommitRef = useRef(onCommit);
  useEffect(() => {
    onCommitRef.current = onCommit;
  });

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  /** Schedule a debounced commit for a LOCAL edit (user typing). */
  const scheduleCommit = useCallback(
    (value: string) => {
      clearTimer();
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        onCommitRef.current(value);
      }, DEBOUNCE_MS);
    },
    [clearTimer]
  );

  /** Flush any pending debounce immediately (blur / tab switch). */
  const flush = useCallback(
    (value: string) => {
      clearTimer();
      onCommitRef.current(value);
    },
    [clearTimer]
  );

  /** Cancel pending debounce without committing (unmount / discard). */
  const cancel = useCallback(() => {
    clearTimer();
  }, [clearTimer]);

  // Cancel on unmount to avoid calling stale callbacks.
  useEffect(() => cancel, [cancel]);

  return { scheduleCommit, flush, cancel };
}
