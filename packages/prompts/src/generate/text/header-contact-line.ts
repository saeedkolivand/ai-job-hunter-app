/**
 * Résumé header line classification — mirrors several Rust parser predicates
 * (`apps/desktop/src-tauri/src/export/parser/mod.rs`) so both sides agree on
 * (a) what counts as a document's header contact line, and (b) what counts as
 * a section-heading boundary that ends the header block. Kept in parity by
 * shared fixtures (`../../fixtures/header-contact-line.json`,
 * `../../fixtures/section-names.json`, `../../fixtures/all-caps-headings.json`),
 * read by both this file's tests and `cargo test export::parser` — a doc
 * comment claiming parity is not parity.
 *
 * Consumed by the renderer's header-seeding pass (H — the editor is the
 * source of truth: `apps/desktop/src/renderer/lib/generate/generation/generation.ts`)
 * to find the model-generated résumé's own contact line so it can be
 * replaced with the Contact Profile's header line, bounded so it never runs
 * past the header block into the document body (see `looksLikeHeaderBoundary`
 * there — this file supplies the recognizers, not the structural bound).
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
 * (`SECTION_NAMES.contains(&lower.as_str())`) — a Title-Case heading
 * ("Experience", "Perfil") flips `seen_section` here even when it isn't
 * ALL-CAPS. Covers every locale `../../locale/index.ts`'s `CONVENTIONS` ships
 * résumé headers for (en/de/fr/es/it/nl/pt). See {@link isAllCapsSectionHeading}
 * for the shape-based sibling that catches an ALL-CAPS heading not literally
 * in this list.
 */
export function isKnownSectionName(line: string): boolean {
  return KNOWN_SECTION_NAMES.has(line.replace(/\*\*/g, '').trim().toLowerCase());
}

/** Mirrors the Rust parser's `COMPANY_KEYWORDS` — whole-word tokens that make
 *  an otherwise heading-shaped ALL-CAPS line "likely a company/role name"
 *  instead (a job title such as "SENIOR SOFTWARE ENGINEER" must never end the
 *  header-seeding scan). Case-sensitive, matching Rust's `&str` equality —
 *  `is_likely_company_or_role` only ever runs on already-uppercased text, so
 *  `SaaS` (the one mixed-case entry) can never actually match; kept exactly
 *  as Rust has it for byte-for-byte parity rather than "fixed". */
const COMPANY_KEYWORDS = new Set([
  'NASA',
  'IBM',
  'AWS',
  'GCP',
  'USA',
  'UK',
  'EU',
  'CEO',
  'CTO',
  'VP',
  'SVP',
  'ENGINEER',
  'DEVELOPER',
  'MANAGER',
  'DIRECTOR',
  'LEAD',
  'SENIOR',
  'SR',
  'JUNIOR',
  'JR',
  'STAFF',
  'PRINCIPAL',
  'ARCHITECT',
  'ANALYST',
  'CONSULTANT',
  'IT',
  'AI',
  'ML',
  'UI',
  'UX',
  'API',
  'REST',
  'SaaS',
  'B2B',
  'B2C',
  'HR',
]);

function isLikelyCompanyOrRole(text: string): boolean {
  return text.split(/\s+/).some((word) => COMPANY_KEYWORDS.has(word));
}

/** Mirrors the Rust parser's `DATE_RE` — a month name or a bare `19xx`/`20xx`
 *  year, followed within 30 chars by `Present`/`Current`/`Now`/`Heute`/
 *  `Ongoing`/`Actuel` or another year. A job-entry date RANGE, not a section
 *  title, so a line matching this is excluded from
 *  {@link isAllCapsSectionHeading}. */
const DATE_RANGE_RE =
  /\b(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?|19\d{2}|20\d{2})[\s\S]{0,30}?(?:Present|Current|Now|Heute|Ongoing|Actuel|20\d{2}|19\d{2})\b/i;

/** Mirrors Rust's `clean.chars().any(|c| c.is_ascii_digit() &&
 *  clean.matches(c).count() == 4)` — a crude "no years" guard: true when SOME
 *  ASCII digit appears exactly 4 times anywhere in the string (not
 *  necessarily as a contiguous run). */
function hasAsciiDigitRepeatedExactlyFourTimes(s: string): boolean {
  const counts = new Map<string, number>();
  for (const ch of s) {
    if (ch >= '0' && ch <= '9') counts.set(ch, (counts.get(ch) ?? 0) + 1);
  }
  return [...counts.values()].some((n) => n === 4);
}

/** Unicode-alphabetic character count — mirrors Rust's
 *  `c.is_alphabetic()` (the `char` Unicode property), not just ASCII. */
function alphabeticCount(s: string): number {
  return (s.match(/\p{L}/gu) ?? []).length;
}

/**
 * The ALL-CAPS `SectionHeader` shape rule — a mirror of Rust's
 * `is_all_caps_section_heading`: fully uppercase, 4–60 chars, ≥2 alphabetic
 * characters, no ASCII digit repeated exactly 4 times, not a likely
 * company/role/acronym token ({@link isLikelyCompanyOrRole}), no date-range
 * shape ({@link DATE_RANGE_RE}), and no `@`.
 *
 * This is what recognizes a locale's own ALL-CAPS heading (`PERFIL`,
 * `PROFILO`, `WERKERVARING`, …) — the résumé prompt mandates ALL-CAPS section
 * titles — without needing every locale's exact wording in
 * {@link isKnownSectionName}'s list, and what an English heading not
 * literally in that list ("PROFESSIONAL EXPERIENCE", "KEY ACHIEVEMENTS")
 * falls back to. Kept in parity by the shared fixture
 * `../../fixtures/all-caps-headings.json` — a previous version of this
 * predicate was DELETED from the TS mirror without a fixture gate, which
 * silently broke header-seeding for the very locales/headings this exists to
 * catch (see the fixture's own history for the exact repro).
 */
export function isAllCapsSectionHeading(line: string): boolean {
  return (
    line === line.toUpperCase() &&
    line.length >= 4 &&
    line.length <= 60 &&
    alphabeticCount(line) >= 2 &&
    !hasAsciiDigitRepeatedExactlyFourTimes(line) &&
    !isLikelyCompanyOrRole(line) &&
    !DATE_RANGE_RE.test(line) &&
    !line.includes('@')
  );
}
