import { INTL_LETTER_CONVENTIONS, LETTER_MARKET_CONVENTIONS } from './letter-market-data.js';

export type LetterFormality = 'formal' | 'warm' | 'direct';
export type DatePosition = 'top-right' | 'below-header' | 'above-salutation';
export type SenderPosition = 'top-left' | 'top-right';
export type RecipientPosition = 'left' | 'right' | 'top-right';

/** Cover-letter conventions for one market (country/region). */
export interface LetterMarketConventions {
  /** English country/region name (for UI + the prompt). */
  country: string;
  /** ISO-639-1 of the market's native language. */
  nativeLanguage: string;
  formality: LetterFormality;
  lengthWords: { min: number; max: number };
  /** Physical page — `letter` only for the US; everyone else A4. */
  page: 'a4' | 'letter';
  dateFormat: string;
  datePosition: DatePosition;
  senderPosition: SenderPosition;
  recipientPosition: RecipientPosition;
  /** Whether the market uses a subject line, plus its localized label. */
  subjectLine: { use: boolean; label: string };
  /** Native-language salutations; the prompt translates them to the letter language when it differs. */
  salutations: { named: string; generic: string };
  /** Native-language sign-offs (first = most formal/default). */
  signoffs: string[];
  /** Market-expected content (e.g. DACH → salary expectation + start date). User-supplied only. */
  inclusions: string[];
  notes: string;
}

/** All known market ids (for UI pickers + tests). */
export const LETTER_MARKET_IDS = Object.keys(LETTER_MARKET_CONVENTIONS);

/** Letter conventions for a market id, falling back to the international baseline. */
export function letterConventions(market?: string): LetterMarketConventions {
  const key = (market ?? 'intl').trim().toLowerCase();
  return LETTER_MARKET_CONVENTIONS[key] ?? INTL_LETTER_CONVENTIONS;
}

/** True when we have explicit conventions for the market (vs. the intl fallback). */
export function hasLetterConventions(market?: string): boolean {
  const key = (market ?? '').trim().toLowerCase();
  return key in LETTER_MARKET_CONVENTIONS;
}

/**
 * How a market's native-language conventions map onto the language a document is
 * actually written in. See {@link marketLanguageFit}.
 */
export interface MarketLanguageFit {
  /** Resolved conventions for the market (intl baseline for unknown ids). */
  conventions: LetterMarketConventions;
  /** True when the output language IS this market's native language. */
  sameLanguage: boolean;
  /**
   * Phrase one native-language convention string for the output language:
   * quoted verbatim when the languages match, otherwise `the formal <language>
   * equivalent of "<native>"`. Callers append their own surrounding guidance
   * (formality note, named-vs-generic wording), so each surface keeps its voice.
   */
  inOutputLanguage: (native: string) => string;
  /** {@link inOutputLanguage} applied to the market's default (most formal) sign-off. */
  signoff: string;
}

/**
 * Resolve a market plus the "write it in the market's own words, or ask for the
 * formal equivalent in the output language" rule — the decision every surface
 * that renders a salutation/sign-off has to make (letter language, market
 * etiquette).
 *
 * Shared by the cover-letter and application-email builders on purpose: those
 * two derived the same rule independently, and the drift between them is
 * exactly how an English "Dear Hiring Manager," survived on German jobs.
 */
export function marketLanguageFit(market: string | undefined, language: string): MarketLanguageFit {
  const conventions = letterConventions(market);
  const sameLanguage = conventions.nativeLanguage === (language || 'en').slice(0, 2).toLowerCase();
  const inOutputLanguage = (native: string): string =>
    sameLanguage ? `"${native}"` : `the formal ${language} equivalent of "${native}"`;
  return {
    conventions,
    sameLanguage,
    inOutputLanguage,
    signoff: inOutputLanguage(conventions.signoffs[0] ?? ''),
  };
}
