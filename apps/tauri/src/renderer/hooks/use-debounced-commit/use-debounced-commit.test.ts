import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useDebouncedCommit } from './use-debounced-commit';

describe('useDebouncedCommit', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('scheduleCommit does NOT call onCommit before 700 ms', () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useDebouncedCommit(onCommit));

    void act(() => result.current.scheduleCommit('hello'));
    void act(() => vi.advanceTimersByTime(699));

    expect(onCommit).not.toHaveBeenCalled();
  });

  it('scheduleCommit calls onCommit after 700 ms', () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useDebouncedCommit(onCommit));

    void act(() => result.current.scheduleCommit('hello'));
    void act(() => vi.advanceTimersByTime(700));

    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('hello');
  });

  it('rapid scheduleCommit calls debounce — only the LAST value commits', () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useDebouncedCommit(onCommit));

    void act(() => {
      result.current.scheduleCommit('a');
      result.current.scheduleCommit('ab');
      result.current.scheduleCommit('abc');
    });
    void act(() => vi.advanceTimersByTime(700));

    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('abc');
  });

  it('flush fires immediately and cancels any pending debounce', () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useDebouncedCommit(onCommit));

    void act(() => result.current.scheduleCommit('local'));
    void act(() => result.current.flush('local'));
    void act(() => vi.advanceTimersByTime(700));

    // flush fires once; no second call from the cancelled timer.
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('local');
  });

  it('cancel drops the pending debounce — onCommit never called', () => {
    const onCommit = vi.fn();
    const { result } = renderHook(() => useDebouncedCommit(onCommit));

    void act(() => result.current.scheduleCommit('local'));
    void act(() => result.current.cancel());
    void act(() => vi.advanceTimersByTime(700));

    expect(onCommit).not.toHaveBeenCalled();
  });

  it('cancel on unmount prevents stale callback after 700 ms', () => {
    const onCommit = vi.fn();
    const { result, unmount } = renderHook(() => useDebouncedCommit(onCommit));

    void act(() => result.current.scheduleCommit('ghost'));
    unmount();
    void act(() => vi.advanceTimersByTime(700));

    expect(onCommit).not.toHaveBeenCalled();
  });

  it('onCommit identity change between schedule and fire uses the latest callback', () => {
    const first = vi.fn();
    const second = vi.fn();
    let cb = first;
    const { result, rerender } = renderHook(() => useDebouncedCommit(cb));

    void act(() => result.current.scheduleCommit('value'));

    // Swap callback before the timer fires.
    cb = second;
    rerender();

    void act(() => vi.advanceTimersByTime(700));

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith('value');
  });
});
