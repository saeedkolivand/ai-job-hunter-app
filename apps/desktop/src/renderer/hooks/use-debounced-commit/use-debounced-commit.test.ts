import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useDebouncedCommit } from './use-debounced-commit';

// Convenience type alias used throughout the tests.
type Doc = 'resume' | 'cover';

describe('useDebouncedCommit', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('scheduleCommit does NOT call onCommit before 700 ms', () => {
    const onCommit = vi.fn<(out: Doc, value: string) => void>();
    const { result } = renderHook(() => useDebouncedCommit<Doc>(onCommit));

    void act(() => result.current.scheduleCommit('resume', 'hello'));
    void act(() => vi.advanceTimersByTime(699));

    expect(onCommit).not.toHaveBeenCalled();
  });

  it('scheduleCommit calls onCommit(out, value) after 700 ms', () => {
    const onCommit = vi.fn<(out: Doc, value: string) => void>();
    const { result } = renderHook(() => useDebouncedCommit<Doc>(onCommit));

    void act(() => result.current.scheduleCommit('resume', 'hello'));
    void act(() => vi.advanceTimersByTime(700));

    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('resume', 'hello');
  });

  it('rapid scheduleCommit calls debounce — only the LAST (out, value) pair commits', () => {
    const onCommit = vi.fn<(out: Doc, value: string) => void>();
    const { result } = renderHook(() => useDebouncedCommit<Doc>(onCommit));

    void act(() => {
      result.current.scheduleCommit('resume', 'a');
      result.current.scheduleCommit('resume', 'ab');
      result.current.scheduleCommit('resume', 'abc');
    });
    void act(() => vi.advanceTimersByTime(700));

    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('resume', 'abc');
  });

  it('flush fires immediately with the stored pair and cancels any pending debounce', () => {
    const onCommit = vi.fn<(out: Doc, value: string) => void>();
    const { result } = renderHook(() => useDebouncedCommit<Doc>(onCommit));

    void act(() => result.current.scheduleCommit('resume', 'local'));
    void act(() => result.current.flush());
    void act(() => vi.advanceTimersByTime(700));

    // flush fires once with the captured pair; no second call from the cancelled timer.
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('resume', 'local');
  });

  it('flush is a no-op when there is no pending commit', () => {
    const onCommit = vi.fn<(out: Doc, value: string) => void>();
    const { result } = renderHook(() => useDebouncedCommit<Doc>(onCommit));

    void act(() => result.current.flush());

    expect(onCommit).not.toHaveBeenCalled();
  });

  it('cancel drops the pending debounce — onCommit never called', () => {
    const onCommit = vi.fn<(out: Doc, value: string) => void>();
    const { result } = renderHook(() => useDebouncedCommit<Doc>(onCommit));

    void act(() => result.current.scheduleCommit('cover', 'local'));
    void act(() => result.current.cancel());
    void act(() => vi.advanceTimersByTime(700));

    expect(onCommit).not.toHaveBeenCalled();
  });

  it('cancel on unmount prevents stale callback after 700 ms', () => {
    const onCommit = vi.fn<(out: Doc, value: string) => void>();
    const { result, unmount } = renderHook(() => useDebouncedCommit<Doc>(onCommit));

    void act(() => result.current.scheduleCommit('resume', 'ghost'));
    unmount();
    void act(() => vi.advanceTimersByTime(700));

    expect(onCommit).not.toHaveBeenCalled();
  });

  it('onCommit identity change between schedule and fire uses the latest callback', () => {
    const first = vi.fn<(out: Doc, value: string) => void>();
    const second = vi.fn<(out: Doc, value: string) => void>();
    let cb = first;
    const { result, rerender } = renderHook(() => useDebouncedCommit<Doc>(cb));

    void act(() => result.current.scheduleCommit('resume', 'value'));

    // Swap callback before the timer fires.
    cb = second;
    rerender();

    void act(() => vi.advanceTimersByTime(700));

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith('resume', 'value');
  });

  // ── BUG 2 regression: tab-switch commit must route to the original doc ────────

  it('tab-switch mid-edit: flush commits to the ORIGINAL doc, not the new activeOut', () => {
    // Simulates the real caller: scheduleCommit captures ('resume', typedText),
    // then the user switches tabs, then flush() is called.
    // onCommit must receive ('resume', typedText) — not ('cover', anything).
    const onCommit = vi.fn<(out: Doc, value: string) => void>();
    const { result } = renderHook(() => useDebouncedCommit<Doc>(onCommit));

    // 1. User types on the resume tab — pair captured as ('resume', 'typed text').
    void act(() => result.current.scheduleCommit('resume', 'typed text'));

    // 2. Before 700 ms, user switches to cover tab — caller calls flush().
    void act(() => vi.advanceTimersByTime(300));
    void act(() => result.current.flush());

    // 3. After the switch, advance past the original debounce window.
    void act(() => vi.advanceTimersByTime(700));

    // flush must have committed ONCE to 'resume' with 'typed text'.
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('resume', 'typed text');
  });

  it('timer fires and commits the captured pair even when the caller has switched docs', () => {
    // The timer always commits the pair set at scheduleCommit time.
    const onCommit = vi.fn<(out: Doc, value: string) => void>();
    const { result } = renderHook(() => useDebouncedCommit<Doc>(onCommit));

    void act(() => result.current.scheduleCommit('cover', 'cover draft'));

    // 700 ms later — no flush, no cancel; the timer fires with the captured pair.
    void act(() => vi.advanceTimersByTime(700));

    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('cover', 'cover draft');
  });
});
