import { describe, expect, it } from 'vitest';

import { resetAppWarningKey } from './reset-message';

describe('resetAppWarningKey', () => {
  it('is null on a full reset', () => {
    expect(resetAppWarningKey({ success: true })).toBeNull();
  });

  it('flags an incomplete reset when a store failed to clear', () => {
    expect(
      resetAppWarningKey({ success: false, error: 'x', failedStores: ['portable_prefs'] })
    ).toBe('settings.privacy.resetAppIncomplete');
  });

  it('prefers the failed-store message over the browser-session one', () => {
    expect(
      resetAppWarningKey({
        success: false,
        failedStores: ['credentials'],
        browserStateRetained: true,
      })
    ).toBe('settings.privacy.resetAppIncomplete');
  });

  it('keeps the board-session message when only browser state remains', () => {
    expect(resetAppWarningKey({ success: false, browserStateRetained: true })).toBe(
      'settings.privacy.resetAppPartial'
    );
  });
});
