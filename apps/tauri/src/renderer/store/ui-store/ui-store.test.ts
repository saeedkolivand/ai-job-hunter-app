import { beforeEach, describe, expect, it } from 'vitest';

import { useUiStore } from './ui-store';

describe('useUiStore', () => {
  beforeEach(() => {
    // Reset to initial state between tests.
    useUiStore.setState({ shortcutsOpen: false });
  });

  it('starts with shortcuts closed', () => {
    expect(useUiStore.getState().shortcutsOpen).toBe(false);
  });

  it('setShortcutsOpen(true) opens the shortcuts panel', () => {
    useUiStore.getState().setShortcutsOpen(true);
    expect(useUiStore.getState().shortcutsOpen).toBe(true);
  });

  it('setShortcutsOpen(false) closes the shortcuts panel', () => {
    useUiStore.getState().setShortcutsOpen(true);
    useUiStore.getState().setShortcutsOpen(false);
    expect(useUiStore.getState().shortcutsOpen).toBe(false);
  });
});
