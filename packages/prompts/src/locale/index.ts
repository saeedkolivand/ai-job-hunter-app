/**
 * Locale data — section-header lexicons, resume conventions, and token factors.
 *
 * All market/locale behaviour keys off the JOB-AD's detected locale (there is no
 * default-to-German or default-to-English market assumption). Pure data + helpers,
 * no dependencies.
 */

export {
  type DatePosition,
  hasLetterConventions,
  LETTER_MARKET_IDS,
  letterConventions,
  type LetterFormality,
  type LetterMarketConventions,
  type MarketLanguageFit,
  marketLanguageFit,
  type RecipientPosition,
  type SenderPosition,
} from './letter-conventions.js';
export { LETTER_MARKET_CONVENTIONS } from './letter-market-data.js';
export {
  countryFromLocation,
  countryToCurrency,
  countryToMarket,
  resolveMarket,
  type ResolveMarketInput,
} from './market.js';
export {
  hasResumeConventions,
  RESUME_CONVENTION_LOCALES,
  type ResumeConventions,
  resumeConventions,
  type ResumeSectionHeaderId,
} from './resume-conventions.js';
export { SECTION_LEXICON, type SectionLexiconEntry, type SectionName } from './sections.js';

// ─── Token estimation factors ─────────────────────────────────────────────────

/**
 * Characters-per-token by locale. `length / 4` (English) under-counts tokens for
 * languages that tokenizers split more aggressively (German, Dutch, …), so those
 * use a smaller divisor → a higher token estimate.
 */
export const CHARS_PER_TOKEN: Record<string, number> = {
  en: 4,
  de: 3.2,
  nl: 3.4,
  fr: 3.6,
  es: 3.7,
  it: 3.7,
  pt: 3.7,
};

/** Characters-per-token divisor for a locale (default 4). */
export function charsPerToken(locale?: string): number {
  const key = (locale ?? 'en').slice(0, 2).toLowerCase();
  return CHARS_PER_TOKEN[key] ?? 4;
}
