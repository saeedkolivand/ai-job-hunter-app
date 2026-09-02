/**
 * Help-page search (ADR-041).
 *
 * Deliberately not fuzzy, not stemmed and not indexed: the help corpus is a few
 * dozen already-translated entries, so a plain `String.includes` scan is the
 * right tool and works identically in every locale.
 */

/**
 * True when every whitespace-separated token of `query` appears somewhere in
 * `text`, case-insensitively and in any order (word-AND).
 *
 * An empty or whitespace-only query matches everything, so an untouched search
 * box leaves the whole page visible.
 */
export function matchesHelpQuery(query: string, text: string): boolean {
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return true;

  const haystack = text.toLowerCase();
  return tokens.every((token) => haystack.includes(token));
}
