import i18n from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next } from 'react-i18next';

import { getClient } from '@/lib/app-client';

import de from './locales/de.json';
import en from './locales/en.json';

const SUPPORTED = ['en', 'de'];

function getInitialLanguage(): string | undefined {
  // 1. Persisted user preference takes priority
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

// Sync renderer locale -> main process on change (one-way to keep AI locale-aware).
// getClient() may not be ready yet if this fires during i18n init, so we swallow errors.
i18n.on('languageChanged', (lng) => {
  try {
    void getClient().system.setLocale(lng as import('@ajh/shared/types').Locale);
  } catch {
    // AppClient not initialized — fired during i18n init before AppClientProvider mounts.
  }
});

export default i18n;
