import { useCallback, useEffect, useRef } from 'react';

const DEBOUNCE_MS = 700;

/**
 * Debounces LOCAL edits (user typing) ~700 ms before calling `onCommit`, while
 * letting EXTERNAL changes (generation / regeneration) skip the debounce entirely.
 *
 * Contract:
 * - Call `scheduleCommit(out, value)` on every local-edit onChange — the (out,
 *   value) PAIR is captured at call time so a tab switch cannot misroute the edit.
 * - `flush()` commits the captured pair immediately and cancels any pending
 *   debounce — call on blur and on doc/tab switch.
 * - `cancel()` drops any pending debounce without committing — call on unmount or
 *   when the caller switches away and wants to discard the pending edit.
 *
 * The hook is stateless — it does not hold the committed value itself; the caller
 * owns state and passes `onCommit` to receive updates.
 */
export function useDebouncedCommit<TOut extends string>(
  onCommit: (out: TOut, value: string) => void
) {
  // Keep a stable ref so the timeout closure always sees the latest callback
  // without re-creating the timer functions when onCommit changes identity.
  const onCommitRef = useRef(onCommit);
  useEffect(() => {
    onCommitRef.current = onCommit;
  });

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // The pending (out, value) pair captured at scheduleCommit time — the timer and
  // flush both commit THIS pair, never a later activeOut from the closure.
  const pendingRef = useRef<{ out: TOut; value: string } | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  /** Schedule a debounced commit for a LOCAL edit (user typing). */
  const scheduleCommit = useCallback(
    (out: TOut, value: string) => {
      clearTimer();
      // Capture the pair NOW — the timer closure reads pendingRef, not the
      // closure-captured `out`/`value`, so rapid re-schedules always use the latest.
      pendingRef.current = { out, value };
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        const pending = pendingRef.current;
        if (pending) {
          pendingRef.current = null;
          onCommitRef.current(pending.out, pending.value);
        }
      }, DEBOUNCE_MS);
    },
    [clearTimer]
  );

  /** Flush any pending debounce immediately (blur / tab switch). */
  const flush = useCallback(() => {
    clearTimer();
    const pending = pendingRef.current;
    if (pending) {
      pendingRef.current = null;
      onCommitRef.current(pending.out, pending.value);
    }
  }, [clearTimer]);

  /** Cancel pending debounce without committing (unmount / discard). */
  const cancel = useCallback(() => {
    clearTimer();
    pendingRef.current = null;
  }, [clearTimer]);

  // Cancel on unmount to avoid calling stale callbacks.
  useEffect(() => cancel, [cancel]);

  return { scheduleCommit, flush, cancel };
}
