/**
 * `<html lang>` follows the UI language.
 *
 * The defect this pins was invisible on screen: switching the app to German
 * left the document root at `en`, so a screen reader read German text with an
 * English voice. Nothing in `@ajh/translations` touches the DOM, so the shim is
 * the only thing that can set it.
 *
 * The shim is imported for its SIDE EFFECTS, exactly as `main.tsx` imports it.
 */
import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

import '@/i18n';

describe('renderer i18n shim — document language', () => {
  it('sets the language at init, not only on the first change', () => {
    // jsdom's document starts with an EMPTY `lang`, and nothing else in the
    // app writes it — so a language tag being there at all can only have come
    // from the import above. Asserted against an absolute ("starts with en"),
    // not against `i18n.language`, which would agree with itself even if both
    // were wrong.
    expect(document.documentElement.lang).toMatch(/^en\b/);
  });

  it('follows every language change, so the voice changes with the text', async () => {
    await i18n.changeLanguage('de');
    expect(document.documentElement.lang).toMatch(/^de\b/);

    await i18n.changeLanguage('en-US');
    expect(document.documentElement.lang).toMatch(/^en\b/);
  });

  it('announces the bundle the text came from, not the requested region subtag', async () => {
    // The active locale carries a region — `en-US` at first launch, `de-AT`
    // for an Austrian user — but the bundles are plain `en`/`de`. `lang`
    // describes the language of the CONTENT, so it follows what i18next
    // resolved rather than what was asked for.
    await i18n.changeLanguage('de-AT');
    expect(document.documentElement.lang).toBe('de');

    await i18n.changeLanguage('en-US');
    expect(document.documentElement.lang).toBe('en');
  });
});
