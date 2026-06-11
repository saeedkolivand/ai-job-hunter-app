/**
 * Renderer i18n init shim.
 *
 * The generic i18n instance + resources live in `@ajh/translations`.
 * Importing that package initializes i18next as a side-effect (once).
 *
 * This shim adds the only renderer-coupled piece: a `languageChanged`
 * listener that mirrors the renderer locale to the main process so the
 * AI stays locale-aware. That listener needs `@/lib/app-client` +
 * `@ajh/shared`, which must NOT leak into the generic package — so it
 * stays here.
 *
 * `main.tsx` imports this module for its side-effects (init + listener).
 */
import type { Locale } from '@ajh/shared/types';
import i18n from '@ajh/translations';

import { getClient } from '@/lib/app-client';

// Sync renderer locale -> main process on change (one-way to keep AI locale-aware).
// getClient() may not be ready yet if this fires during i18n init, so we swallow errors.
i18n.on('languageChanged', (lng) => {
  try {
    void getClient().system.setLocale(lng as Locale);
  } catch {
    // AppClient not initialized — fired during i18n init before AppClientProvider mounts.
  }
});

export type { TFunction } from '@ajh/translations';
export { useTranslation } from '@ajh/translations';
export default i18n;
