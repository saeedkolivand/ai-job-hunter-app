/**
 * Résumé header line classification — mirrors two Rust parser predicates
 * (`apps/desktop/src-tauri/src/export/parser/mod.rs`) so both sides agree on
 * (a) what counts as a document's header contact line, and (b) what counts as
 * a section-heading boundary that ends the header block. Kept in parity by
 * shared fixtures (`../../fixtures/header-contact-line.json`,
 * `../../fixtures/section-names.json`), read by both this file's tests and
 * `cargo test export::parser` — a doc comment claiming parity is not parity.
 *
 * Consumed by the renderer's header-seeding pass (H — the editor is the
 * source of truth: `apps/desktop/src/renderer/lib/generate/generation/generation.ts`)
 * to find the model-generated résumé's own contact line so it can be
 * replaced with the Contact Profile's header line, without running past a
 * real section heading into the document body.
 */

import sectionNames from '../../fixtures/section-names.json';

/** Mirrors the Rust parser's `PHONE_RE`: an optional leading `+`, a digit,
 *  then ≥7 more digits/space/`-`/`(`/`)`/`.`. */
const HEADER_PHONE_RE = /\+?\d[\d\s\-().]{7,}/;

/** Mirrors the Rust parser's `URL_RE` — a known contact-platform keyword or a
 *  bare `http(s)://` URL, case-insensitive (matches Rust's `(?i)` flag). */
const HEADER_URL_RE = /linkedin\.com|github\.com|portfolio|website|^https?:\/\//i;

/** Combined `|`/`·`/`•` separator count — mirrors the Rust parser's
 *  `separator_count` (shared by its job-entry pipe check and its Contact
 *  arm), which is what the `>= 2` check below actually counts. */
function headerSeparatorCount(line: string): number {
  return (line.match(/[|·•]/g) ?? []).length;
}

/**
 * A pre-section line carrying contact info — a mirror of the Rust parser's
 * `is_contact_shaped`: `clean.contains('@') || PHONE_RE.is_match(clean) ||
 * pipe_count >= 2 || URL_RE.is_match(clean)`. `strip_md` does not strip
 * `[label](url)` markdown, so testing the raw line (no stripping here either)
 * matches what Rust's `clean` actually contains.
 */
export function isHeaderContactLine(line: string): boolean {
  return (
    line.includes('@') ||
    HEADER_PHONE_RE.test(line) ||
    headerSeparatorCount(line) >= 2 ||
    HEADER_URL_RE.test(line)
  );
}

/**
 * The line-0-ONLY Contact test — narrower than {@link isHeaderContactLine}: an
 * `@` or a phone shape only, no pipe/URL arms. Mirrors the Rust parser's
 * `is_first_line_contact_shaped`, which is what decides Name vs Contact for a
 * résumé's first line — a combined "Jane Doe | jane@example.com" classifies
 * as `Contact`, not `Name`, there, so a caller with no `fullName` to write
 * over that line must still recognize it as (already) the contact line, not
 * as an untouchable name line to scan past.
 */
export function isFirstLineContactShaped(line: string): boolean {
  return line.includes('@') || HEADER_PHONE_RE.test(line);
}

/** Rust's `SECTION_NAMES` const, read live from the shared fixture rather
 *  than duplicated as a second hardcoded list — the only copy of this data
 *  lives in the JSON file, so this side of the parity pair can never drift. */
const KNOWN_SECTION_NAMES = new Set<string>(sectionNames);

/**
 * True when `line` (after trimming and stripping `**bold**` markers, mirroring
 * Rust's `strip_md`) is an EXACT, case-insensitive match for one of the known
 * multilingual section headings Rust's parser recognizes
 * (`SECTION_NAMES.contains(&lower.as_str())`) — the check that actually flips
 * Rust's `seen_section` in the common case (an ATX `#` heading is the other).
 * Deliberately narrower than "looks like a heading": a job-title line such as
 * "SENIOR SOFTWARE ENGINEER" is NOT a known section name, so it correctly
 * does not end the header-seeding scan — the ALL-CAPS heuristic this replaces
 * used to false-trigger on exactly that (real, prompt-mandated) shape.
 */
export function isKnownSectionName(line: string): boolean {
  return KNOWN_SECTION_NAMES.has(line.replace(/\*\*/g, '').trim().toLowerCase());
}
