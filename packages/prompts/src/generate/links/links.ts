/**
 * Resume contact-link extraction + post-generation hyperlink injection.
 *
 * The Rust PDF/DOCX extractor appends a `\n---\n` markdown reference block of
 * `[anchor](url)` entries. We turn that into (a) a prompt instruction telling the
 * AI to write short labels (LinkedIn, GitHub), and (b) a post-generation injector
 * that replaces those labels with real markdown links.
 */

import { classifyLinks, parseLinkBlock } from './classify.js';
import { MIN_TITLE_KEY_LEN, normalizeKey } from './line-match.js';

export { getBodyLinkMap, getLinkMap, urlToFriendlyLabel } from './classify.js';
export { injectLinksIntoGeneratedText } from './inject.js';

interface ParsedResumeLinks {
  /** Compact block to inject before <candidate_resume> */
  block: string;
  /** Clean email address extracted from mailto annotation, or empty string */
  cleanEmail: string;
}
/**
 * Parse the markdown reference block appended by the Rust PDF/DOCX extractor.
 * Returns a prompt injection block telling the AI to write short labels
 * (LinkedIn, GitHub) — not full URLs. Actual hyperlinks are injected
 * post-generation by injectLinksIntoGeneratedText().
 */
export function parseLinksFromResume(resume: string): ParsedResumeLinks {
  const entries = parseLinkBlock(resume);
  if (!entries.length) return { block: '', cleanEmail: '' };

  const mailto = entries.find((e) => e.url.startsWith('mailto:'));
  const cleanEmail = mailto ? mailto.url.slice('mailto:'.length) : '';

  // Exactly the labels (platform brands + one "Website") getLinkMap() will inject,
  // so the AI is instructed to write the same short labels we later hyperlink.
  const labelEntries = classifyLinks(resume).contact.map((e) => e.label);

  if (!labelEntries.length && !cleanEmail) return { block: '', cleanEmail: '' };

  const parts: string[] = [];
  if (cleanEmail) {
    parts.push(`CANDIDATE EMAIL (use this exact address, no spaces): ${cleanEmail}`);
  }
  if (labelEntries.length) {
    parts.push(
      `CANDIDATE PROFILE LINKS — write ONLY these short labels in the contact line (NOT the full URL):\n` +
        labelEntries.join(', ') +
        `\nExample: Haarlem, Netherlands | name@example.com | +31... | LinkedIn | GitHub | Website`
    );
  }

  return { block: parts.join('\n\n'), cleanEmail };
}

/**
 * Build a prompt instruction for the candidate's BODY links — project, article,
 * publication and portfolio URLs that belong to specific résumé items rather than
 * the contact line (#18). The block regime (PDF/RTF) strips these before the
 * model ever sees them, so without re-surfacing them here they are silently
 * dropped (the original academic-link bug).
 *
 * Partitioned in two (#HIGH part 1): most entries tell the model to name the
 * item with its own real name (#B) — `injectLinksIntoGeneratedText()`
 * matches those by normalized key, not literal text (#C). But a key shorter
 * than the matcher's own floor (`MIN_TITLE_KEY_LEN`) can never title-match no
 * matter what the model writes, so for those SHORT keys only, the old
 * "write this exact label" instruction survives — that's the only way the
 * literal-fallback in `injectLinksIntoGeneratedText()` can still reach them.
 * Telling the model "never write the key" for every entry, unconditionally,
 * made the fallback unreachable and the safety-net claim false.
 *
 * Returns '' when there are no body links.
 */
export function buildBodyLinksBlock(resume: string): string {
  const body = classifyLinks(resume).body;
  if (!body.length) return '';

  const reachable = body.filter((b) => normalizeKey(b.label).length >= MIN_TITLE_KEY_LEN);
  const unreachable = body.filter((b) => normalizeKey(b.label).length < MIN_TITLE_KEY_LEN);

  const parts: string[] = [];
  if (reachable.length) {
    parts.push(
      `CANDIDATE PROJECT / PUBLICATION LINKS — each entry below is a machine-derived reference key ` +
        `for a link that belongs to a specific item in the résumé (a project, publication, or ` +
        `portfolio piece), NOT the contact line. For each entry, write that item using the project's ` +
        `REAL name as it appears in the résumé — never the key itself, never a URL or slug, and never ` +
        `appended to the title (the hyperlink is attached automatically by matching the item name). ` +
        `Every entry must end up with exactly one matching item. If an item has no natural home in ` +
        `Experience or Skills, add a PROJECTS or PUBLICATIONS section and list it there — but never ` +
        `invent a title or context just to force one in:\n` +
        reachable.map((b) => `- ${b.label}`).join('\n')
    );
  }
  if (unreachable.length) {
    parts.push(
      `CANDIDATE PROJECT / PUBLICATION LINKS (SHORT KEYS) — these reference keys are too short to ` +
        `rename safely, so write EACH ONE exactly as shown below, verbatim, as the visible text on ` +
        `its matching item (the hyperlink is attached automatically by matching this exact text). If ` +
        `an item has no natural home in Experience or Skills, add a PROJECTS or PUBLICATIONS section ` +
        `and list it there — but never invent a title or context just to force one in:\n` +
        unreachable.map((b) => `- ${b.label}`).join('\n')
    );
  }
  return parts.join('\n\n');
}

/**
 * Strip the link reference block from resume text before sending to the AI
 * so the body text budget is not wasted on the reference list.
 */
export function stripLinkBlock(resume: string): string {
  const sep = resume.lastIndexOf('\n---\n');
  return sep === -1 ? resume : resume.slice(0, sep);
}
