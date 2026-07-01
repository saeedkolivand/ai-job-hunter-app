import { beforeEach, describe, expect, it } from 'vitest';

import { useUiStore } from './ui-store';

describe('useUiStore', () => {
  beforeEach(() => {
    // Reset to initial state between tests.
    useUiStore.setState({
      shortcutsOpen: false,
      notificationsOpen: false,
      extensionTokenFocus: false,
    });
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

  it('extensionTokenFocus defaults to false', () => {
    expect(useUiStore.getState().extensionTokenFocus).toBe(false);
  });

  it('setExtensionTokenFocus(true) sets the flag', () => {
    useUiStore.getState().setExtensionTokenFocus(true);
    expect(useUiStore.getState().extensionTokenFocus).toBe(true);
  });

  it('setExtensionTokenFocus(false) clears the flag', () => {
    useUiStore.getState().setExtensionTokenFocus(true);
    useUiStore.getState().setExtensionTokenFocus(false);
    expect(useUiStore.getState().extensionTokenFocus).toBe(false);
  });
});
