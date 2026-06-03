import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { useKeyboardShortcuts } from './use-keyboard-shortcuts';

function press(key: string, opts: KeyboardEventInit = {}, target?: HTMLElement) {
  const ev = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...opts });
  (target ?? window).dispatchEvent(ev);
  return ev;
}

describe('useKeyboardShortcuts', () => {
  it('jumps to a route on the g-then-letter chord', () => {
    const onNavigate = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onNavigate, onToggleHelp: vi.fn() }));
    press('g');
    press('j');
    expect(onNavigate).toHaveBeenCalledExactlyOnceWith('/jobs');
  });

  it('maps Ctrl/Cmd+K to search and Ctrl/Cmd+, to settings', () => {
    const onNavigate = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onNavigate, onToggleHelp: vi.fn() }));
    press('k', { ctrlKey: true });
    expect(onNavigate).toHaveBeenLastCalledWith('/search');
    press(',', { metaKey: true });
    expect(onNavigate).toHaveBeenLastCalledWith('/settings');
  });

  it('toggles the cheat-sheet on ?', () => {
    const onToggleHelp = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onNavigate: vi.fn(), onToggleHelp }));
    press('?');
    expect(onToggleHelp).toHaveBeenCalledTimes(1);
  });

  it('does not fire single-key shortcuts while typing in a field', () => {
    const onNavigate = vi.fn();
    const onToggleHelp = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onNavigate, onToggleHelp }));
    const input = document.createElement('input');
    document.body.appendChild(input);
    press('g', {}, input);
    press('j', {}, input);
    press('?', {}, input);
    expect(onNavigate).not.toHaveBeenCalled();
    expect(onToggleHelp).not.toHaveBeenCalled();
    input.remove();
  });

  it('still allows Cmd+K from within a field', () => {
    const onNavigate = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onNavigate, onToggleHelp: vi.fn() }));
    const input = document.createElement('input');
    document.body.appendChild(input);
    press('k', { metaKey: true }, input);
    expect(onNavigate).toHaveBeenCalledWith('/search');
    input.remove();
  });
});
