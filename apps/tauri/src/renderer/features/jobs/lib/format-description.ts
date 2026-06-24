/**
 * Pure, testable job-description formatter.
 *
 * Content shape (verified against the Rust scraping pipeline):
 *  - `strip_html` path (Greenhouse, Lever API plain): one flat line, no HTML,
 *    no entities, all whitespace collapsed.
 *  - `html_to_text` path (LinkedIn, generic HTML): plain text with `\n`
 *    line-breaks; list items already converted to `• item` on their own line;
 *    consecutive blank lines already capped at one (`\n\n`).
 *  - API `descriptionPlain` (Ashby, Lever): raw plain text, may have CRLF
 *    line-endings (Windows clipboard / API responses).
 *
 * Because the Rust layer already decodes every HTML entity and strips all tags,
 * this formatter NEVER needs to handle raw HTML or entities.
 *
 * Round-trip guarantee: all content words (excluding bullet/number markers)
 * are preserved across the block representation.
 */

export type DescriptionBlock =
  | { type: 'heading'; text: string }
  | { type: 'paragraph'; text: string }
  | { type: 'list'; items: string[] };

// Bullet markers produced by `html_to_text` (• is the canonical one from
// the Rust `TT_LI_RE`) plus common plain-text conventions.
const BULLET_RE = /^(\s*)(•|▪|·|‣|–|-|\*)\s+/;

// Numbered list: "1." or "1)" at the start of a line.
const NUMBERED_RE = /^(\s*)\d+[.)]\s+/;

/** Returns true when a line opens with a bullet or number marker. */
function isBulletLine(line: string): boolean {
  return BULLET_RE.test(line) || NUMBERED_RE.test(line);
}

/** Strip the leading marker from a bullet line and return the item text. */
function stripMarker(line: string): string {
  return line.replace(BULLET_RE, '').replace(NUMBERED_RE, '').trim();
}

/**
 * Is this a heading candidate?
 * Conservative rules — all must pass to avoid false positives:
 *  1. Short (≤60 chars after trim).
 *  2. No trailing sentence punctuation (. ? !) — headings don't end sentences.
 *  3. Ends with `:` OR is ALL-CAPS (min 3 chars) OR is Title-Case with ≥3 words.
 *  4. Must be followed by more content (checked at call site).
 *
 * Title-Case requires ≥3 words to avoid false positives on 2-word phrases
 * like "Remote Position" or "Join Google".
 */
function isHeadingCandidate(line: string): boolean {
  const t = line.trim();
  if (t.length === 0 || t.length > 60) return false;
  if (/[.?!]$/.test(t)) return false;

  // Ends with colon — the strongest signal (e.g. "Requirements:", "About us:")
  if (t.endsWith(':')) return true;

  // ALL-CAPS heading (min 3 chars, at least one letter, allow spaces/punctuation)
  if (t.length >= 3 && /[A-Z]/.test(t) && t === t.toUpperCase() && /[A-Za-z]/.test(t)) return true;

  // Title-Case: most words start with uppercase — require ≥3 words to avoid
  // false positives on 2-word phrases like "Remote Position" or "Join Google".
  const words = t.split(/\s+/);
  if (words.length >= 3) {
    const capsWords = words.filter((w) => /^[A-Z]/.test(w));
    if (capsWords.length >= Math.ceil(words.length * 0.6)) return true;
  }

  return false;
}

/**
 * Parse a sequence of lines (already split from a chunk, no blank lines within)
 * into blocks. Used recursively so heading-then-bullets in one chunk is handled.
 */
function parseLines(lines: string[], chunkIsLast: boolean): DescriptionBlock[] {
  if (lines.length === 0) return [];

  // ── All bullet lines → single list block ─────────────────────────────────
  if (lines.every(isBulletLine)) {
    return [{ type: 'list', items: lines.map(stripMarker) }];
  }

  // ── Mixed chunk: interleaved prose + bullets ──────────────────────────────
  if (lines.some(isBulletLine)) {
    const blocks: DescriptionBlock[] = [];
    let i = 0;
    while (i < lines.length) {
      const line = lines[i];
      if (line === undefined) break;

      if (isBulletLine(line)) {
        const items: string[] = [];
        while (i < lines.length) {
          const l = lines[i];
          if (l === undefined || !isBulletLine(l)) break;
          items.push(stripMarker(l));
          i++;
        }
        blocks.push({ type: 'list', items });
      } else {
        // Check if this prose line qualifies as a heading with bullet remainder.
        if (isHeadingCandidate(line) && i + 1 < lines.length && isBulletLine(lines[i + 1] ?? '')) {
          blocks.push({ type: 'heading', text: line });
          i++;
          // Recurse on remaining lines (will hit the bullet branch above).
          blocks.push(...parseLines(lines.slice(i), chunkIsLast));
          return blocks;
        }
        const prose: string[] = [];
        while (i < lines.length) {
          const l = lines[i];
          if (l === undefined || isBulletLine(l)) break;
          prose.push(l);
          i++;
        }
        if (prose.length > 0) {
          blocks.push({ type: 'paragraph', text: prose.join(' ') });
        }
      }
    }
    return blocks;
  }

  // ── Single-line chunk: check for heading ─────────────────────────────────
  if (lines.length === 1) {
    const text = lines[0] ?? '';
    // Only promote to heading when there will be more content after this chunk.
    if (isHeadingCandidate(text) && !chunkIsLast) {
      return [{ type: 'heading', text }];
    }
    return [{ type: 'paragraph', text }];
  }

  // ── Multi-line prose chunk ────────────────────────────────────────────────
  // If the first line is a heading candidate, split it off; recurse on the rest
  // so "Requirements:\n• A\n• B" (no blank line) → heading + list.
  const firstLine = lines[0] ?? '';
  if (isHeadingCandidate(firstLine)) {
    const rest = lines.slice(1);
    const restBlocks = parseLines(rest, chunkIsLast);
    return [{ type: 'heading', text: firstLine }, ...restBlocks];
  }

  return [{ type: 'paragraph', text: lines.join(' ') }];
}

/**
 * Split `raw` into a sequence of `DescriptionBlock`s suitable for rendering.
 * Never drops content — when unsure, everything becomes a paragraph.
 */
export function formatJobDescription(raw: string): DescriptionBlock[] {
  if (!raw || !raw.trim()) return [];

  // Normalise line-endings FIRST (CRLF from Lever/Ashby plain text or pastes).
  const lf = raw.replace(/\r\n/g, '\n').replace(/\r/g, '\n');

  // Collapse 3+ consecutive blank lines to exactly one blank line.
  // Strip trailing horizontal whitespace per line.
  const normalised = lf.replace(/\n{3,}/g, '\n\n').replace(/[ \t]+$/gm, '');

  // Split on blank lines to get coarse chunks.
  const chunks = normalised
    .split(/\n\n+/)
    .map((c) => c.trim())
    .filter(Boolean);

  const blocks: DescriptionBlock[] = [];

  for (let ci = 0; ci < chunks.length; ci++) {
    const chunk = chunks[ci] ?? '';
    const isLast = ci === chunks.length - 1;
    const lines = chunk
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean);
    if (lines.length === 0) continue;
    blocks.push(...parseLines(lines, isLast));
  }

  // Edge case: if nothing parsed, fall back to the original trimmed text.
  if (blocks.length === 0 && raw.trim()) {
    blocks.push({ type: 'paragraph', text: raw.trim() });
  }

  return blocks;
}
