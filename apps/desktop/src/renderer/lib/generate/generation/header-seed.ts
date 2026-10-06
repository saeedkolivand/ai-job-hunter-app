/**
 * Header seeding (H — the editor is the source of truth).
 *
 * PDF/DOCX export used to rebuild the header (name + contact line) from the
 * Contact Profile every time, discarding whatever the generated/edited text
 * said. The profile's own header is now seeded into the canonical text right
 * after generation, so the string the editor shows IS what exports — the
 * Rust-side overrides (`ContactProfile::apply_to_header`, the `candidate_name`
 * overrides) are fallbacks for a header that has none, not unconditional rewrites.
 *
 * `isHeaderContactLine` (a mirror of the Rust parser's `is_contact_shaped`)
 * lives in `@ajh/prompts/generate` so its cross-language parity fixture test
 * sits alongside it.
 */

import {
  type GenerationMeta,
  isAllCapsSectionHeading,
  isFirstLineContactShaped,
  isHeaderContactLine,
  isKnownSectionName,
} from '@ajh/prompts/generate';
import type { ContactProfile } from '@ajh/shared';
import { toLanguageCode } from '@ajh/shared/language-detection';

import { getClient } from '../../app-client';
import { errorClass } from '../../error-class';

/**
 * True once we're past the header block. Mirrors the Rust parser's
 * `seen_section` flag: only an actual section heading ends the header zone — a
 * BLANK line does not on its own, so the scan must keep looking past it. A
 * markdown ATX heading (`#…`) is always a boundary; otherwise a line is a
 * boundary when it's a known section name (`isKnownSectionName`, every locale's
 * résumé headers) OR has the shape of an ALL-CAPS section title
 * (`isAllCapsSectionHeading` — the résumé prompt mandates ALL-CAPS headers).
 *
 * This predicate is a best-effort recognizer, not a safety mechanism — see
 * `seedHeaderFromProfile`'s STRUCTURAL bound for what actually prevents an
 * unrecognized heading from turning the seeding scan destructive.
 */
function looksLikeHeaderBoundary(line: string): boolean {
  const t = line.trim();
  if (!t) return false;
  if (/^#{1,6}\s/.test(t)) return true;
  return isKnownSectionName(t) || isAllCapsSectionHeading(t);
}

/**
 * Strip control characters (a `\n` above all) and cap length — mirrors the Rust
 * `sanitize_header_part` treatment `contactLine` already went through.
 * `fullName` reaches this function as a separate, un-sanitized string, so
 * without this a raw `\n` would inject an arbitrary extra physical line,
 * including — if it read as a known section name — a fabricated section Rust's
 * parser would treat as real. Iterates code points (`[...name]`), not UTF-16
 * units, so a surrogate pair straddling the 200 cap isn't split. Strips `\p{Cc}`
 * AND `\p{Cf}` (e.g. the bidi override U+202E, which can visually REVERSE the
 * rendered name; mirrors Rust's `is_format_char` in `contact_profile/header.rs`).
 */
function sanitizeHeaderName(name: string): string {
  return [...name.replace(/[\p{Cc}\p{Cf}]/gu, '')].slice(0, 200).join('');
}

/**
 * Casing/diacritic/punctuation-insensitive key so "SAEED KOLIVAND" and "Saeed
 * Kolivand" — or "François Müller" and "FRANCOIS MULLER" — compare equal. NFKD
 * decomposes an accented character into base letter + combining mark; the mark
 * (`Mn`) is not `\p{L}`/`\p{N}`, so it must be STRIPPED (`\p{M}` → `''`) before
 * the punctuation collapse — collapsing it to a space would insert a spurious
 * word break ("François" → "franc ois"). Then every remaining non-letter/
 * non-number run collapses to a single space.
 */
function nameKey(s: string): string {
  return s
    .normalize('NFKD')
    .replace(/\p{M}/gu, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim()
    .toLowerCase();
}

/**
 * The index of the first non-blank line, or `lines.length` if every line is
 * blank. The header block starts here, not unconditionally at index 0 — Rust's
 * parser accepts the first line with content after ANY number of leading blanks
 * (`export/parser/mod.rs`), and this side must agree with it.
 */
function firstContentLine(lines: string[]): number {
  let i = 0;
  while (i < lines.length && (lines[i] ?? '').trim() === '') i++;
  return i;
}

/**
 * The header block: the first run of content lines — starting at
 * {@link firstContentLine} — up to (not including) the next blank line, or the
 * whole array if there is none. Shared ceiling for the name search in
 * {@link findNameLine} and the contact-line scan in `seedHeaderFromProfile`.
 * Recomputed wherever needed rather than cached across a mutation: an `unshift`
 * shifts every later index by one. The termination scan starts one past
 * `firstContentLine` (not a hardcoded 1) so TWO OR MORE leading blank lines — a
 * shape PDF extraction produces — don't end the block at the second blank.
 */
function headerBlockEnd(lines: string[]): number {
  for (let i = firstContentLine(lines) + 1; i < lines.length; i++) {
    if ((lines[i] ?? '').trim() === '') return i;
  }
  return lines.length;
}

/**
 * The index of the header-block line that's already the profile's own name
 * (compared via {@link nameKey}), or -1. `key` is pre-computed by the caller so
 * an empty-after-normalizing `fullName` short-circuits without scanning — an
 * empty key would otherwise match any other blank-after-normalizing line.
 *
 * Also stops at the first `looksLikeHeaderBoundary` line (the same STRUCTURAL
 * bound as the contact scan): with no blank line anywhere, `headerBlockEnd` is
 * `lines.length`, and an unbounded scan could match the name recurring in the
 * BODY (a sign-off line) instead of ever seeding one at the top. The match check
 * runs BEFORE the boundary check: an ALL-CAPS name is itself boundary-shaped, so
 * it must be allowed to match on the very line that would otherwise break the scan.
 */
function findNameLine(lines: string[], key: string): number {
  if (!key) return -1;
  const blockEnd = headerBlockEnd(lines);
  for (let i = 0; i < blockEnd; i++) {
    const line = lines[i] ?? '';
    if (nameKey(line) === key) return i;
    if (looksLikeHeaderBoundary(line)) break;
  }
  return -1;
}

/** A real email shape (local-part `@` domain `.` tld), not just a bare `@` —
 *  "Software Engineer @ Acme" contains an `@` but no email; only a genuine
 *  email should outrank other candidates in {@link pickReplacementIndex}. */
const EMAIL_SHAPE_RE = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+[.][A-Za-z]{2,}/;

/**
 * Choose which of possibly several contact-shaped `matches` to overwrite with
 * `contactLine`, by a positive signal rather than by position: a genuine email
 * wins; failing that, a match with no `@` but a phone shape; failing that, the
 * FIRST match — not the last, which is closest to the body and so most likely
 * real body content (a skills line with 2+ separators) that a boundary miss let
 * through. Precondition: `matches` is non-empty.
 */
function pickReplacementIndex(lines: string[], matches: number[]): number {
  const withEmail = matches.find((i) => EMAIL_SHAPE_RE.test(lines[i] ?? ''));
  if (withEmail !== undefined) return withEmail;
  const withPhone = matches.find((i) => {
    const line = lines[i] ?? '';
    return !line.includes('@') && isFirstLineContactShaped(line);
  });
  if (withPhone !== undefined) return withPhone;
  return matches[0] ?? 0;
}

/**
 * Seed the generated text's header with the Contact Profile's own values, so
 * the canonical string already carries the exact header PDF/DOCX export
 * renders. Line 1 (the name) is replaced when the profile has a `fullName`
 * (sanitized by {@link sanitizeHeaderName}). `contactLine` carries NO sanitizer
 * on this side: it is always the return value of the `contact_profile_header_line`
 * IPC call (Rust's `ContactProfile::header_markdown()`), which already strips
 * control characters and caps length — a second TS-side pass would only risk
 * drifting from Rust's sanitizer. No-op when the profile has nothing to contribute.
 *
 * Exactly ONE pre-section contact-shaped line — chosen by
 * {@link pickReplacementIndex}, not by position — is overwritten with
 * `contactLine`. This function NEVER removes a line: only replaces one, or
 * (when nothing qualifies) inserts one. A second contact-shaped line in the
 * block therefore SURVIVES (a visible, user-correctable duplicate, since Rust's
 * `model_from_resume_text` joins every pre-section Contact line with `" · "`) —
 * the deliberate trade for never DESTROYING real résumé content, whether by
 * deleting or by silently overwriting it.
 *
 * The first content line ({@link firstContentLine}) is never a candidate for the
 * replacement scan, including when there's no `fullName` and it is already
 * contact-shaped ("Jane Doe | jane@example.com" with no separate name line):
 * overwriting it would erase the name, so it falls to the "insert" branch.
 *
 * Before the unshift/replace fallback, the `fullName` branch searches the header
 * block ({@link findNameLine}) for a line that's ALREADY the profile's own name
 * (casing/punctuation-insensitively) and overwrites just that line in place.
 * This closes two duplicate-header repros: (1) an ALL-CAPS name line, which
 * `looksLikeHeaderBoundary` reads as a section heading, would otherwise get the
 * profile's name inserted above it; (2) a leading blank line — line 0 would be
 * blindly replaced, leaving the model's real name line one row down. `nameKey`
 * can match nothing but the profile's own name, so it's the single line
 * guaranteed redundant with what we're about to write.
 *
 * `sanitizeHeaderName` never changes case, so an ALL-CAPS `fullName` reconciled
 * onto a line at `i > 0` would re-trip `looksLikeHeaderBoundary` where the contact
 * scan starts — the scan is told which index was reconciled and skips it.
 *
 * ponytail: known ceiling, not a bug — a boundary-shaped name line that does NOT
 * match the profile's `fullName` still falls through to unshift and stacks as a
 * visible duplicate; guessing it is "probably the name" would risk destroying a
 * real section heading, which is strictly worse.
 *
 * STRUCTURAL bound (the actual safety mechanism — `looksLikeHeaderBoundary` is
 * best-effort recognition): the scan never looks past the first
 * blank-line-delimited block from the top of the text. A heading-recognition
 * MISS can only degrade to "didn't seed" or "left a duplicate line", never scan
 * into the body and destroy real content.
 */
export function seedHeaderFromProfile(
  text: string,
  profile: ContactProfile,
  contactLine: string
): string {
  const lines = text.split('\n');
  if (!lines.length) return text;

  const fullName = profile.fullName?.trim();
  // Set when the fullName branch reconciles an EXISTING header-block line in
  // place — the contact scan treats that line like the first content line.
  let reconciledNameIndex = -1;
  if (fullName) {
    const nameLineIndex = findNameLine(lines, nameKey(fullName));
    if (nameLineIndex !== -1) {
      lines[nameLineIndex] = sanitizeHeaderName(fullName);
      reconciledNameIndex = nameLineIndex;
    } else {
      // Line 0 is only overwritten when it's actually name-shaped — not a
      // section heading and not already contact-shaped. A model that omits the
      // name line (starts straight with "SUMMARY" or a combined "Jane Doe |
      // jane@example.com" line) must never have that line clobbered — the name
      // is INSERTED ahead of it instead.
      const line0 = lines[0] ?? '';
      if (looksLikeHeaderBoundary(line0) || isFirstLineContactShaped(line0)) {
        lines.unshift(sanitizeHeaderName(fullName));
      } else {
        lines[0] = sanitizeHeaderName(fullName);
      }
    }
  }

  if (contactLine.trim()) {
    // Recomputed here (not reused from the fullName branch) because an
    // `unshift` there shifted every index by one.
    const blockEnd = headerBlockEnd(lines);
    const contentStart = firstContentLine(lines);
    const matches: number[] = [];
    for (let i = 1; i < blockEnd; i++) {
      // Two independent exclusions, either of which can land anywhere in the
      // block: the first content line (never a candidate), and the line the name
      // step reconciled in place (an ALL-CAPS profile name stays ALL-CAPS and
      // would re-trip `looksLikeHeaderBoundary`, breaking the scan before it
      // reaches the real contact line and stacking a second one via insert).
      if (i === contentStart || i === reconciledNameIndex) continue;
      const line = lines[i] ?? '';
      if (looksLikeHeaderBoundary(line)) break;
      if (isHeaderContactLine(line)) matches.push(i);
    }

    if (matches.length > 0) {
      lines[pickReplacementIndex(lines, matches)] = contactLine;
    } else {
      // Assuming line 0 is the name is wrong when there's no fullName to seed AND
      // line 0 is itself a section heading: splicing after it would put the
      // contact line INSIDE that section. Insert ahead of the heading instead,
      // and key off the real first content line, not a hardcoded 0/1, so a run
      // of leading blanks isn't split.
      const insertAt = looksLikeHeaderBoundary(lines[contentStart] ?? '')
        ? contentStart
        : contentStart + 1;
      lines.splice(insertAt, 0, contactLine);
    }
  }

  return lines.join('\n');
}

/**
 * Fetch the Contact Profile + its localized header line and seed them into
 * `text` (H — the editor is the source of truth over whatever header the model
 * wrote). Shared by every résumé-producing path: `generateResume` AND
 * `synthesizeResume` (the Resume Builder, which has no base résumé).
 *
 * Both IPC calls are guarded (`.catch(() => undefined)`): header seeding is
 * cosmetic post-processing on an already-paid-for generation — a transient IPC
 * failure must degrade to "seed nothing", never throw and discard the result.
 *
 * `signal`: this step runs AFTER the model stream finishes, so it can't cancel
 * an in-flight generation, but it honors a cancellation issued WHILE these
 * calls were in flight (or already true): it short-circuits to "seed nothing"
 * before starting AND after they resolve (Tauri's `invoke()` has no abort wiring).
 */
export async function seedHeaderFromContactProfile(
  text: string,
  meta: GenerationMeta,
  locale: string,
  signal?: AbortSignal
): Promise<string> {
  if (signal?.aborted) return text;
  const api = getClient();
  const headerLang = toLanguageCode(meta.targetLanguage || locale);
  // Fired concurrently: headerLine's input doesn't depend on the fetched profile.
  // Each call keeps its own guard so a rejection on either degrades to "seed nothing".
  const [contact, contactLine] = await Promise.all([
    api.contactProfile.get().catch((err: unknown) => {
      console.warn(
        'seedHeaderFromContactProfile: contactProfile.get failed, header not seeded',
        errorClass(err)
      );
      return undefined;
    }),
    api.contactProfile.headerLine(headerLang).catch((err: unknown) => {
      console.warn(
        'seedHeaderFromContactProfile: contactProfile.headerLine failed, header not seeded',
        errorClass(err)
      );
      return undefined;
    }),
  ]);
  if (signal?.aborted) return text;
  if (!contact || contactLine === undefined) return text;
  return seedHeaderFromProfile(text, contact, contactLine);
}
