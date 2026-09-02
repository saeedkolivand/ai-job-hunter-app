/**
 * Help-page search (ADR-041).
 *
 * Deliberately not fuzzy, not stemmed and not indexed: the help corpus is a few
 * dozen already-translated entries, so a plain `String.includes` scan is the
 * right tool. What it is NOT is plain lowercasing — both sides are diacritic-
 * folded first, which is the only reason an ASCII-keyboard query finds accented
 * copy: `resume` matches an entry spelling it `résumé`, and `prufen` matches
 * `prüfen`. Without the fold the de locale is largely unreachable from a
 * keyboard that has no umlaut keys.
 */

/**
 * Lowercase and strip combining marks: `PRÜFEN`, `Prüfen` and `prufen` all
 * normalize to `prufen`. NFD splits a precomposed `ü` into `u` + U+0308
 * COMBINING DIAERESIS, which the `\p{Diacritic}` class then removes.
 */
function fold(s: string): string {
  return s
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase();
}

/**
 * True when every whitespace-separated token of `query` appears somewhere in
 * `text`, case- and diacritic-insensitively and in any order (word-AND).
 *
 * An empty or whitespace-only query matches everything, so an untouched search
 * box leaves the whole page visible.
 */
export function matchesHelpQuery(query: string, text: string): boolean {
  const tokens = fold(query).split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return true;

  const haystack = fold(text);
  return tokens.every((token) => haystack.includes(token));
}
