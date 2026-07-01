/**
 * Single source of truth for the supported résumé/cover-letter OUTPUT languages.
 *
 * Used by the generation pipeline (`safeLocale` clamps any incoming locale to one
 * of these) and by the Resume Builder's output-language picker. Add a language
 * here once and both stay in sync — no other literal list of locales should exist.
 */

import type { Locale } from '@ajh/shared';

export interface OutputLanguage {
  code: string;
  /** The language's name in the language itself (e.g. "Deutsch"). */
  endonym: string;
  /** The language's English name (e.g. "German"). */
  englishName: string;
  /**
   * True for languages the bundled PDF/preview fonts can't render — see
   * `apps/desktop/src-tauri/src/export/typst_engine/world.rs` `LoadedFonts::load`.
   * Generation and DOCX work; PDF + live preview tofu until CJK font support
   * lands (a follow-up). The Resume Builder warns when one of these is selected.
   */
  cjk?: boolean;
}

export const OUTPUT_LANGUAGES: readonly OutputLanguage[] = [
  { code: 'en', endonym: 'English', englishName: 'English' },
  { code: 'de', endonym: 'Deutsch', englishName: 'German' },
  { code: 'fr', endonym: 'Français', englishName: 'French' },
  { code: 'es', endonym: 'Español', englishName: 'Spanish' },
  { code: 'it', endonym: 'Italiano', englishName: 'Italian' },
  { code: 'tr', endonym: 'Türkçe', englishName: 'Turkish' },
  { code: 'pt', endonym: 'Português', englishName: 'Portuguese' },
  { code: 'ru', endonym: 'Русский', englishName: 'Russian' },
  { code: 'zh', endonym: '中文', englishName: 'Chinese', cjk: true },
  { code: 'ja', endonym: '日本語', englishName: 'Japanese', cjk: true },
  { code: 'ko', endonym: '한국어', englishName: 'Korean', cjk: true },
] as const;

export const VALID_LOCALES = OUTPUT_LANGUAGES.map((l) => l.code);

export type SupportedLocale = (typeof OUTPUT_LANGUAGES)[number]['code'];

/**
 * Clamp any incoming locale to a supported one (defaults to English). Returns the
 * shared `Locale` union so it slots straight into the typed `locale` field of
 * `AiGenerateRequest` — callers need no cast. `VALID_LOCALES` is derived from
 * `OUTPUT_LANGUAGES` and matches `Locale` exactly, so the membership check is sound.
 */
export function safeLocale(lng: string): Locale {
  return (VALID_LOCALES.includes(lng) ? lng : 'en') as Locale;
}
