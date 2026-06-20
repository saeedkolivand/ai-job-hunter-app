/**
 * makeMultiSelectKeyHandler — unit tests.
 *
 * Verifies:
 *  - Arrow keys move focus (update ref + call .focus()) without toggling
 *  - Home / End jump to boundaries without toggling
 *  - Space / Enter toggle the currently-focused item without moving focus
 *  - Unhandled keys are ignored
 *  - Wraps at boundaries (first ← = last; last → = first)
 */
import type React from 'react';
import { describe, expect, it, vi } from 'vitest';

import { makeMultiSelectKeyHandler } from './use-roving-tabindex';

function makeKey(key: string): KeyboardEvent {
  return { key, preventDefault: vi.fn() } as unknown as KeyboardEvent;
}

function setup(initial = 0, count = 3) {
  const focusedIdxRef = { current: initial };
  const focusFns = Array.from({ length: count }, () => vi.fn());
  const refs = {
    current: focusFns.map((fn) => ({ focus: fn }) as unknown as HTMLButtonElement),
  };
  const onToggle = vi.fn();
  const handler = makeMultiSelectKeyHandler(count, focusedIdxRef, refs, onToggle);
  return { focusedIdxRef, focusFns, refs, onToggle, handler };
}

// Helper: cast to KeyboardEvent<HTMLElement> for the handler signature
function fire(handler: ReturnType<typeof makeMultiSelectKeyHandler>, e: KeyboardEvent) {
  handler(e as unknown as React.KeyboardEvent<HTMLElement>);
}

describe('makeMultiSelectKeyHandler — arrow navigation', () => {
  it('ArrowRight moves focus forward without toggling', () => {
    const { focusedIdxRef, focusFns, onToggle, handler } = setup(0, 3);
    const e = makeKey('ArrowRight');
    fire(handler, e);
    expect(focusedIdxRef.current).toBe(1);
    expect(focusFns[1]).toHaveBeenCalledOnce();
    expect(onToggle).not.toHaveBeenCalled();
    expect(e.preventDefault).toHaveBeenCalled();
  });

  it('ArrowDown moves focus forward without toggling', () => {
    const { focusedIdxRef, focusFns, onToggle, handler } = setup(1, 3);
    const e = makeKey('ArrowDown');
    fire(handler, e);
    expect(focusedIdxRef.current).toBe(2);
    expect(focusFns[2]).toHaveBeenCalledOnce();
    expect(onToggle).not.toHaveBeenCalled();
  });

  it('ArrowLeft moves focus backward without toggling', () => {
    const { focusedIdxRef, focusFns, onToggle, handler } = setup(2, 3);
    const e = makeKey('ArrowLeft');
    fire(handler, e);
    expect(focusedIdxRef.current).toBe(1);
    expect(focusFns[1]).toHaveBeenCalledOnce();
    expect(onToggle).not.toHaveBeenCalled();
  });

  it('ArrowUp moves focus backward without toggling', () => {
    const { focusedIdxRef, focusFns, onToggle, handler } = setup(1, 3);
    const e = makeKey('ArrowUp');
    fire(handler, e);
    expect(focusedIdxRef.current).toBe(0);
    expect(focusFns[0]).toHaveBeenCalledOnce();
    expect(onToggle).not.toHaveBeenCalled();
  });

  it('ArrowRight wraps from last to first', () => {
    const { focusedIdxRef, focusFns, handler } = setup(2, 3);
    const e = makeKey('ArrowRight');
    fire(handler, e);
    expect(focusedIdxRef.current).toBe(0);
    expect(focusFns[0]).toHaveBeenCalledOnce();
  });

  it('ArrowLeft wraps from first to last', () => {
    const { focusedIdxRef, focusFns, handler } = setup(0, 3);
    const e = makeKey('ArrowLeft');
    fire(handler, e);
    expect(focusedIdxRef.current).toBe(2);
    expect(focusFns[2]).toHaveBeenCalledOnce();
  });
});

describe('makeMultiSelectKeyHandler — Home / End', () => {
  it('Home jumps to index 0', () => {
    const { focusedIdxRef, focusFns, onToggle, handler } = setup(2, 3);
    const e = makeKey('Home');
    fire(handler, e);
    expect(focusedIdxRef.current).toBe(0);
    expect(focusFns[0]).toHaveBeenCalledOnce();
    expect(onToggle).not.toHaveBeenCalled();
  });

  it('End jumps to last index', () => {
    const { focusedIdxRef, focusFns, onToggle, handler } = setup(0, 3);
    const e = makeKey('End');
    fire(handler, e);
    expect(focusedIdxRef.current).toBe(2);
    expect(focusFns[2]).toHaveBeenCalledOnce();
    expect(onToggle).not.toHaveBeenCalled();
  });
});

describe('makeMultiSelectKeyHandler — Space / Enter toggle', () => {
  it('Space toggles the focused item without moving focus', () => {
    const { focusedIdxRef, focusFns, onToggle, handler } = setup(1, 3);
    const e = makeKey(' ');
    fire(handler, e);
    expect(onToggle).toHaveBeenCalledWith(1);
    expect(focusedIdxRef.current).toBe(1);
    // No extra .focus() calls
    expect(focusFns[0]).not.toHaveBeenCalled();
    expect(focusFns[1]).not.toHaveBeenCalled();
    expect(e.preventDefault).toHaveBeenCalled();
  });

  it('Enter toggles the focused item without moving focus', () => {
    const { focusedIdxRef, onToggle, handler } = setup(0, 3);
    const e = makeKey('Enter');
    fire(handler, e);
    expect(onToggle).toHaveBeenCalledWith(0);
    expect(focusedIdxRef.current).toBe(0);
  });
});

describe('makeMultiSelectKeyHandler — unhandled keys', () => {
  it('ignores Tab without side-effects', () => {
    const { focusedIdxRef, onToggle, handler } = setup(0, 3);
    const e = makeKey('Tab');
    fire(handler, e);
    expect(focusedIdxRef.current).toBe(0);
    expect(onToggle).not.toHaveBeenCalled();
    expect(e.preventDefault).not.toHaveBeenCalled();
  });
});
