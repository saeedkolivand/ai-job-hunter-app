/**
 * AI Generate prompts — metadata extraction, resume + cover-letter generation,
 * keyword emphasis, contact-link handling, and output cleanup. The
 * `@ajh/prompts/generate` entry point.
 */

export * from './application-questions.js';
export * from './cover-letter.js';
export * from './emphasis.js';
export * from './links.js';
export * from './metadata.js';
export * from './modes.js';
export * from './resume.js';
export * from './text.js';

// Market resolution lives in the locale module but is consumed alongside the
// generation builders (cover letter + application answers), so it's re-exported
// here for a single import source.
export {
  countryToMarket,
  hasLetterConventions,
  LETTER_MARKET_IDS,
  letterConventions,
  type LetterMarketConventions,
  resolveMarket,
  type ResolveMarketInput,
} from '../locale/index.js';
