/**
 * Heuristic extractor for a recipient email address + nearby contact name from
 * a job-description string. Used ONLY to prefill the "Apply by email" recipient
 * fields — the user always reviews and edits before anything is sent.
 *
 * Returns `{ email?, name? }`. Both are optional; an absent email means no
 * address was found; an absent name means an address was found but no
 * nearby capitalized-name pattern.
 */
export function extractRecipient(text: string): { name?: string; email?: string } {
  // Cap scan at 20 KB — recipient contacts appear in the first paragraph;
  // scanning the full job description (up to 200 KB) risks ReDoS on adversarial input.
  const bounded = text.slice(0, 20_000);
  const emailMatch = /[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/.exec(bounded);
  if (!emailMatch) return {};

  const email = emailMatch[0];

  // Scan the 150 chars before the email for the last sequence of 2–3 Title-Case
  // words (likely a person name). `[A-Z][a-z]+` avoids ALL-CAPS acronyms.
  const window = bounded.slice(Math.max(0, emailMatch.index - 150), emailMatch.index);
  const nameMatches = [...window.matchAll(/\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+){1,2})\b/g)];
  const name = nameMatches.at(-1)?.[1];

  return { email, ...(name ? { name } : {}) };
}
