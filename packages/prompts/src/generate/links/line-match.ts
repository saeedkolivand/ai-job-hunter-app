import type { ResumeSection } from '../../context-manager/sections.js';

/**
 * Bullets/plain whitespace (`•`, `-`, `*`, space) OR a genuine ordered-list
 * marker (digits immediately followed by `.`/`)`, then whitespace) stripped
 * before body-title matching. Deliberately does NOT treat a bare leading
 * digit run as a marker unless it is actually followed by `.`/`)` +
 * whitespace — otherwise a title that starts with a digit ("3D Printing
 * Pipeline", "2048 Game Engine", "500px Clone Gallery") loses its leading
 * digit(s) to the strip and can never title-match (#M1).
 */
export function stripLeadingMarker(line: string): number {
  let i = 0;
  while (i < line.length && /[\s•*-]/.test(line[i] ?? '')) i++;
  const marker = /^\d+[.)]\s+/.exec(line.slice(i));
  if (marker) {
    i += marker[0].length;
    while (i < line.length && /[\s•*-]/.test(line[i] ?? '')) i++;
  }
  return i;
}
/**
 * Characters normalizeKey / title matching treat as insignificant separators
 * — anything that is not a Unicode letter or digit, so punctuation
 * (apostrophes, colons, parens, commas, …) doesn't break a match, not just
 * hyphen/underscore/whitespace (#M2 — "Jane's Portfolio"/"janes-portfolio",
 * "CrossKit (v2)"/"crosskit-v2", "CrossKit: The Toolkit"/"crosskit-the-toolkit").
 */
const SEPARATOR_CHAR_RE = /[^\p{L}\p{N}]/u;
/** A Unicode letter or digit — used to require a real word boundary. */
const WORD_CHAR_RE = /[\p{L}\p{N}]/u;
/** Anchored check: does `s` begin with an existing `[label](url)` markdown span? */
const LEADING_MD_LINK_RE = /^\[[^\]]{1,200}\]\([^)]{1,2000}\)/;
/**
 * Prefix-match floor for body-title matching (#C) — below this, short labels
 * collide too easily with unrelated text (e.g. "Goth" inside "Gotham City Guide").
 */
export const MIN_TITLE_KEY_LEN = 6;

/** True for a single whitespace character. One char in, so no backtracking. */
function isWs(ch: string | undefined): boolean {
  return ch !== undefined && /\s/.test(ch);
}

/**
 * Index of the whitespace run preceding the first ` — `/` – ` inline separator,
 * or -1 when the line has none.
 *
 * Deliberately NOT `/\s+[—–]\s+/`: that pattern has two unbounded whitespace
 * runs, so on a line with many spaces and no dash the engine retries from every
 * start position and degrades to O(n²) — CodeQL `js/polynomial-redos`, on text
 * extracted from a user-supplied PDF. This is a single left-to-right pass that
 * remembers where the current whitespace run began, so each character is
 * examined once.
 */
export function findInlineSeparator(s: string): number {
  let wsStart = -1;
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (isWs(ch)) {
      if (wsStart === -1) wsStart = i;
      continue;
    }
    if ((ch === '—' || ch === '–') && wsStart !== -1 && isWs(s[i + 1])) return wsStart;
    wsStart = -1;
  }
  return -1;
}

/**
 * Lowercase, accent-folded key with every non-letter/non-digit character
 * stripped (#M2, symmetric with `SEPARATOR_CHAR_RE`) — "ai-job-hunter-app",
 * "ai job hunter app", and "AI Job Hunter" all normalize to the same value, so
 * a body label survives whichever spelling the PDF extractor or the model
 * happened to produce. NFD + combining-mark strip folds accents ("Café" /
 * "cafe") so real-name titles in accented languages still match their slug.
 */
export function normalizeKey(s: string): string {
  return s
    .normalize('NFD')
    .replace(/\p{M}+/gu, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, '');
}

/**
 * Fold ONE character's accents (NFD decompose + strip its combining marks),
 * lowercased — "é" → "e", "ü" → "u" — so the leading-title WALK below (which
 * must stay one-original-character-per-step to keep slicing correct) matches
 * an accented real-name title against an ASCII slug label symmetrically with
 * `normalizeKey` above. Folding a single precomposed letter is 1:1 for every
 * realistic case; falls back to the plain lowercase on the rare empty result.
 */
function foldChar(ch: string): string {
  const folded = ch
    .normalize('NFD')
    .replace(/\p{M}+/gu, '')
    .toLowerCase();
  return folded.charAt(0) || ch.toLowerCase();
}

/**
 * English section-header words the prompt can produce even outside ALL CAPS
 * (only PROJECTS/PUBLICATIONS are guaranteed always-English per resume.ts;
 * the rest are extra safety) — checked case-insensitively as the WHOLE
 * trimmed line (optionally colon-terminated), never as part of a longer
 * title (#M3 — the ALL-CAPS check alone misses Title-Case "Projects").
 */
const KNOWN_SECTION_HEADER_WORDS = new Set([
  'projects',
  'publications',
  'summary',
  'professional summary',
  'work experience',
  'experience',
  'education',
  'skills',
  'certifications',
]);

/**
 * A bare section-header line ("PROJECTS", "SUMMARY", "ZUSAMMENFASSUNG", …) —
 * ALL CAPS (locale-agnostic), or a known English header word in any casing
 * (#M3) — so it is never itself a candidate item title.
 */
function isSectionHeaderLine(line: string): boolean {
  const t = line.trim();
  if (!t) return false;
  if (/\p{Lu}/u.test(t) && !/\p{Ll}/u.test(t)) return true;
  return KNOWN_SECTION_HEADER_WORDS.has(t.replace(/:$/, '').trim().toLowerCase());
}

/**
 * If the match stopped right before a closing bracket/paren whose opener was
 * already consumed inside the matched span (skipped as an insignificant
 * separator, #M2's widened class), extend `end` by one to include it — so
 * "CrossKit (v2)" wraps as a clean, bracket-balanced title instead of
 * leaving a dangling `)` outside the link (`[CrossKit (v2](url))`).
 */
function extendPastDanglingCloser(line: string, start: number, end: number): number {
  const closer = line[end];
  if (closer !== ')' && closer !== ']' && closer !== '}') return end;
  const opener = closer === ')' ? '(' : closer === ']' ? '[' : '{';
  const span = line.slice(start, end);
  const opens = span.split(opener).length - 1;
  const closes = span.split(closer).length - 1;
  return opens > closes ? end + 1 : end;
}

/**
 * If `end` lands between a UTF-16 surrogate pair's two halves, back it off by
 * one unit so a slice never emits an unpaired surrogate — invalid for
 * JSON/serde, and the plausible blast radius is an IPC/save failure.
 */
function backOffSurrogateSplit(line: string, end: number): number {
  if (end <= 0 || end >= line.length) return end;
  const hi = line.charCodeAt(end - 1);
  const lo = line.charCodeAt(end);
  const isHigh = hi >= 0xd800 && hi <= 0xdbff;
  const isLow = lo >= 0xdc00 && lo <= 0xdfff;
  return isHigh && isLow ? end - 1 : end;
}

export interface TitleSpan {
  start: number;
  end: number;
  /** Higher wins the greedy assignment — see injectLinksIntoGeneratedText. */
  score: number;
}

/**
 * Test whether `label`'s normalized key matches the leading title of `line`
 * (#B/#C) — the model now writes the item's real name ("AI Job Hunter"), not
 * the machine label, which may be a URL-derived slug ("ai-job-hunter-app") or
 * its humanised PDF-extraction form ("ai job hunter app"). Walks `line`'s
 * significant characters (skipping a leading bullet/number marker and any
 * non-letter/non-digit separator) alongside the label's normalized key.
 *
 * A character MISMATCH is always rejected outright — never accepted just
 * because enough characters happened to match first. Only two endings count
 * as a real match: the full label key is consumed (and the line, if it
 * continues, does so at a genuine word boundary — "toolkit" must not match
 * inside "toolkits"), or the line's own title is fully consumed as a genuine
 * (>= MIN_TITLE_KEY_LEN) prefix of the label. This is what stops a
 * coincidental overlap ("gotham city guide" inside "Gothamburg Transit Map")
 * from cross-linking the wrong item.
 *
 * Returns null for a bare section-header line, a line already carrying a
 * markdown link at the title position (idempotency), a span that would
 * cross a `[`/`]` (the widened separator class, #M2, would otherwise let a
 * match skip straight across them — see the bracket check below), or no
 * match.
 */
export function matchLineTitle(line: string, label: string): TitleSpan | null {
  if (isSectionHeaderLine(line)) return null;
  const start = stripLeadingMarker(line);
  if (LEADING_MD_LINK_RE.test(line.slice(start))) return null;

  const labelKey = normalizeKey(label);
  if (labelKey.length < MIN_TITLE_KEY_LEN) return null;

  let li = start;
  let matchedLen = 0;
  let end = start;
  while (li < line.length && matchedLen < labelKey.length) {
    const ch = line[li] ?? '';
    if (SEPARATOR_CHAR_RE.test(ch)) {
      li++;
      continue;
    }
    if (foldChar(ch) !== labelKey[matchedLen]) return null;
    matchedLen++;
    li++;
    end = li;
  }
  if (matchedLen < MIN_TITLE_KEY_LEN) return null;

  const lineExhausted = li >= line.length;
  const labelExhausted = matchedLen === labelKey.length;
  if (labelExhausted && !lineExhausted) {
    const next = line[end] ?? '';
    if (WORD_CHAR_RE.test(next)) return null; // mid-word — e.g. "toolkits"
  }

  end = extendPastDanglingCloser(line, start, end);
  end = backOffSurrogateSplit(line, end);
  // Never wrap a span containing `[`/`]` — MD_LINK_SPAN_RE (and the Rust
  // renderer's MD_LINK_RE, model/rich.rs) can't parse nested brackets,
  // and the widened separator class (#M2) would otherwise let a match skip
  // straight across them: "CrossKit [beta] Toolkit" → the broken
  // `[CrossKit [beta] Toolkit](url)` (#MEDIUM).
  const span = line.slice(start, end);
  if (span.includes('[') || span.includes(']')) return null;
  const exact = lineExhausted && labelExhausted;
  const score = matchedLen * 4 + (exact ? 2 : labelExhausted ? 1 : 0);
  return { start, end, score };
}

/** A markdown link span anywhere in a string (non-global, safe for `.test`). */
const HAS_MD_LINK_RE = /\[[^\]]{1,200}\]\([^)]{1,2000}\)/;
/**
 * Is `line` shaped like an item TITLE — not a nested/indented description
 * line, and not a full sentence (#HIGH-1)? The last-resort net's pairing
 * step must never treat a description bullet of an already-linked project,
 * or a prose sentence, as an "open slot" for a different, unrelated label.
 * Also refuses a line containing `[`/`]` (#MEDIUM) — pairing wraps the
 * line's own raw text, so a bracket inside it would produce the same broken
 * nested-bracket markdown the bracket check in `matchLineTitle` exists to
 * prevent.
 *
 * MEDIUM (security re-review): a single top-level bullet marker ("- Fleet
 * Tracker", "• Fleet Tracker") is stripped and the REMAINDER tested — many
 * résumés format project TITLES themselves as a flat bulleted list, not just
 * their descriptions, so flatly rejecting every marked line made this pool
 * unreachable for that (common) shape. Only genuine nesting — leading
 * whitespace/indentation before the marker, the actual textual signal of a
 * sub-point under a parent bullet — is still rejected as a description.
 * `unlinkedItemLineIndices`'s caller already slices on
 * `stripLeadingMarker`'s own index when splicing the link in, so a
 * top-level-bulleted title's marker is preserved untouched either way.
 */
function isItemShapedLine(line: string): boolean {
  if (/^\s/.test(line)) return false; // indented — nested under a parent bullet, a description
  const markerEnd = stripLeadingMarker(line);
  const trimmed = line.slice(markerEnd).trim();
  if (!trimmed || /[.!?]\s*$/.test(trimmed)) return false; // sentence-final punctuation
  if (trimmed.includes('[') || trimmed.includes(']')) return false;
  const words = trimmed.split(/\s+/).filter(Boolean);
  return words.length > 0 && words.length <= 8;
}

/**
 * Line indices, inside a detected PROJECTS/PUBLICATIONS `ResumeSection`, that
 * are item-shaped and carry no link yet — the pool the HIGH-part-2
 * last-resort net draws from when exactly one label is still unmatched after
 * both the title-match and literal-fallback passes (the renamed-item case,
 * e.g. "orbit-sim" written as "Orbital Simulator", which is only knowable
 * after generation — prompt partitioning can't fix it).
 */
export function unlinkedItemLineIndices(lines: string[], sections: ResumeSection[]): number[] {
  const indices: number[] = [];
  for (const section of sections) {
    for (let i = section.startIndex + 1; i <= section.endIndex; i++) {
      const line = lines[i] ?? '';
      if (!line.trim() || HAS_MD_LINK_RE.test(line)) continue;
      if (isItemShapedLine(line)) indices.push(i);
    }
  }
  return indices;
}

/**
 * The line index right after a section's last non-blank content line — or
 * right after its header if the section has no content — the splice point
 * for appending a new item (#HIGH-2, never a bare `lines.push()` at document
 * end with no section context).
 */
export function sectionInsertionPoint(lines: string[], section: ResumeSection): number {
  for (let i = section.endIndex; i > section.startIndex; i--) {
    if ((lines[i] ?? '').trim()) return i + 1;
  }
  return section.startIndex + 1;
}
