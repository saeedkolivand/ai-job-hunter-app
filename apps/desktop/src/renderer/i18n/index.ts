/**
 * Renderer i18n init shim.
 *
 * The generic i18n instance + resources live in `@ajh/translations`.
 * Importing that package initializes i18next as a side-effect (once).
 *
 * This shim adds the renderer-coupled pieces: a `languageChanged` listener
 * that mirrors the renderer locale to the main process so the AI stays
 * locale-aware, and the same mirror onto `<html lang>`. Both need something
 * the generic package must not assume it has — `@/lib/app-client` +
 * `@ajh/shared` for the first, a `document` for the second — so they stay
 * here rather than leaking into a package imported by non-DOM callers.
 *
 * `main.tsx` imports this module for its side-effects (init + listeners).
 */
import type { Locale } from '@ajh/shared/types';
import i18n from '@ajh/translations';

import { getClient } from '@/lib/app-client';

/**
 * Mirror the active locale onto the document root.
 *
 * `<html lang>` is what picks the screen reader's VOICE. It ships as the
 * build-time `en` and i18next never touches the DOM, so before this a user who
 * switched the UI to German had German text read out by an English voice —
 * phonetically wrong end to end, and invisible to anyone not using a reader.
 *
 * `resolvedLanguage` rather than the requested tag: a locale with no bundle
 * falls back to English CONTENT, and announcing that content as German would
 * be the same defect with the languages swapped.
 */
const syncDocumentLang = () => {
  const lang = i18n.resolvedLanguage ?? i18n.language;
  if (lang) document.documentElement.lang = lang;
};

// Sync renderer locale -> main process on change (one-way to keep AI locale-aware).
// getClient() may not be ready yet if this fires during i18n init, so we swallow errors.
i18n.on('languageChanged', (lng) => {
  syncDocumentLang();
  try {
    void getClient().system.setLocale(lng as Locale);
  } catch {
    // AppClient not initialized — fired during i18n init before AppClientProvider mounts.
  }
});

// …and once for the language i18next resolved at init: `languageChanged` has
// already fired by the time this module is imported, so a listener alone would
// leave the very first render (the common case — the user never switches)
// announcing the stored locale in the wrong voice.
syncDocumentLang();

export default i18n;
