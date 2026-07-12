/**
 * AI Generate prompts — metadata extraction, resume + cover-letter generation,
 * keyword emphasis, contact-link handling, and output cleanup. The
 * `@ajh/prompts/generate` entry point.
 */

export * from './application-email/index.js';
export * from './application-questions/index.js';
export * from './cover-letter/index.js';
export * from './emphasis/index.js';
export * from './github-projects/index.js';
export * from './interview-practice/index.js';
export * from './interview-questions/index.js';
export * from './job-ad-summary/index.js';
export * from './links/index.js';
export * from './metadata/index.js';
export * from './modes/index.js';
export * from './natural-voice/index.js';
export * from './referral/index.js';
export * from './resume/index.js';
export * from './rewrite/index.js';
export * from './text/index.js';

// Market resolution lives in the locale module but is consumed alongside the
// generation builders (cover letter + application answers), so it's re-exported
// here for a single import source.
export {
  countryToCurrency,
  countryToMarket,
  hasLetterConventions,
  LETTER_MARKET_IDS,
  letterConventions,
  type LetterMarketConventions,
  resolveMarket,
  type ResolveMarketInput,
} from '../locale/index.js';
