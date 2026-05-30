/** Token + page estimation. */

import { charsPerToken } from '../locale/index.js';

/**
 * Estimate token count (rough approximation: 1 token ≈ N characters, where N is
 * locale-dependent — `length / 4` under-counts languages like German, so pass the
 * job-ad/resume `locale` to use its character-per-token factor).
 */
export function estimateTokens(text: string, locale?: string): number {
  return Math.ceil(text.length / charsPerToken(locale));
}

/**
 * Estimate page count based on character count.
 * Average page: ~3000 characters (500 words × 6 chars/word).
 */
export function estimatePages(text: string): number {
  return Math.ceil(text.length / 3000);
}
