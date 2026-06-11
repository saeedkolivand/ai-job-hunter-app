/**
 * @ajh/translations — the generic i18n instance + translation resources.
 *
 * This package owns the provider-agnostic half of the renderer's i18n:
 *   - the i18next instance wired with LanguageDetector + initReactI18next
 *   - the bundled translation resources (en, de)
 *   - the persisted-language read from localStorage
 *
 * Importing this module initializes i18next as a side-effect (once).
 *
 * It is intentionally free of any app/renderer coupling — no AppClient,
 * no IPC, no @ajh/shared. The renderer keeps a thin init shim that imports
 * this package and attaches the renderer-coupled `languageChanged` listener.
 *
 * Components import `useTranslation` / `TFunction` from here (re-exported
 * through the renderer's `@/lib/i18n` → now `@ajh/translations`).
 */
import i18n from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next, useTranslation as useReactI18nextTranslation } from 'react-i18next';

import de from './locales/de/translation.json';
import en from './locales/en/translation.json';

const SUPPORTED = ['en', 'de'];

function getInitialLanguage(): string | undefined {
  // 1. Persisted user preference takes priority
  if (typeof localStorage === 'undefined') return undefined;
  try {
    const raw = localStorage.getItem('ai-job-hunter-preferences');
    if (raw) {
      const parsed = JSON.parse(raw) as { state?: { language?: string } };
      const saved = parsed?.state?.language;
      if (saved && SUPPORTED.includes(saved)) return saved;
    }
  } catch {
    // ignore
  }
  // 2. Fall through to LanguageDetector (system locale)
  return undefined;
}

const persisted = getInitialLanguage();

void i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      en: { translation: en },
      de: { translation: de },
    },
    // If persisted, set it directly and skip detection
    ...(persisted ? { lng: persisted } : {}),
    fallbackLng: 'en',
    supportedLngs: SUPPORTED,
    // Strip region codes (e.g. "de-AT" → "de")
    load: 'languageOnly',
    detection: {
      // Only use navigator (system locale) — no cookies/localStorage managed by i18next itself
      order: ['navigator'],
      caches: [],
    },
    interpolation: { escapeValue: false },
  });

/**
 * App i18n adapter. The single seam for UI translations — components import
 * `useTranslation` from `@ajh/translations`, never from `react-i18next`. Lets us
 * layer app-wide behavior (namespaces, RTL, instrumentation) without touching
 * call sites.
 */
export const useTranslation: typeof useReactI18nextTranslation = (...args) =>
  useReactI18nextTranslation(...args);

// TFunction lives in i18next (the core), not react-i18next
export type { TFunction } from 'i18next';

export default i18n;
