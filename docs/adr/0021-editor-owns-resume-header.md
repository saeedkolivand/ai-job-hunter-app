# ADR-0021: Editor Owns the Résumé Header

**Status:** Accepted (2026-08-04)

**Decision:** The document text is the source of truth for the résumé header contact line at export time. The contact profile seeds generated documents at generation time (replacing the header); the text that results is then preserved by all export formats. Fallback to profile occurs only when the extracted header is empty.

## Context

Previously, the contact profile was the single authoritative source for the header:

- Every export path (PDF, DOCX, plain text) validated the header against the profile
- Document text was parsed but profile values were expected to match
- `apply_to_header()` enforced that name and contact line matched the profile, rejecting exports where they diverged

This meant user edits to a generated document's header were accepted at generation time but rejected at export time. The validation gate caught profile-template misalignment but also prevented intentional customization.

## Decision

Invert the ownership model at export time:

1. **Generation:** `generateResume()` and `synthesizeResume()` (Resume Builder) both call `seedHeaderFromProfile()`, which reconciles the profile's name and exactly one contact-shaped line in the header block, via the `contact_profile_header_line` IPC call. For the name: the function searches the header block (lines 0 up to the first blank) for a line that already IS the profile's `fullName` (compared casing- and punctuation-insensitively via NFKD-normalized key matching), and overwrites that line in place. This closes two duplicate-header defects: (1) an ALL-CAPS name that was mis-classified as a section heading and inserted-above rather than replaced, and (2) a leading blank line from PDF extraction that pushed the real name to index 1, where it wasn't found. For contact: a positive signal (an email shape first, then a phone shape, then position) chooses ONE contact-shaped line to replace — never by removal. A second contact-shaped line in the block (a stale duplicate, a separator-heavy job title the seeder mis-scanned) is left in place, not deleted — replacing it too risked destroying real content (a job title, a skills line) the seeder mistook for a contact line. This happens on every generation, seeding the initial content.

2. **Editing:** Users can edit the generated header in the rich-text editor. These edits are part of the document text.

3. **Export:** `apply_to_header()` now uses the profile as a **fallback only**:
   - If `header.name` is blank, fill it from `profile.full_name`
   - If `header.contact` (extracted from text) is empty, fill it from the profile's `header_markdown()`
   - If both already have content, the text wins — the profile doesn't overwrite

4. **Export validation:** The gate is a **self-consistency check against the header actually rendered**, not a profile-parity check that gets skipped once the text owns the header:
   - `allowed` (the set of URLs the check treats as legitimate) is built by reconstructing the SAME header the renderer produces — `model_from_resume_text` → `apply_to_header`, the exact `prepare_resume_render` pipeline — so it always reflects whichever side (text or profile) actually supplied the header for THIS render, not only the profile.
   - A header-region PDF link that isn't one of `allowed` blocks (`header_url_mismatch`, critical) — **unconditionally, for every résumé**, whichever side owns the header. Narrowed to the header's own expected link COUNT rather than the whole 144pt geometric band: the band is a heuristic and can capture a genuine, correctly-placed body link (an early job entry's own company site) that isn't part of the header at all; the check only considers the topmost N header-band links, where N is the number of links the reconstructed header itself claims.
   - A profile link that never surfaces anywhere in the (full) header band warns (`header_url_missing`) — advisory, and only checked when the profile actually supplied the header (comparing an unrelated profile against a text-owned header would be a false "missing" for every one of its links).
   - A job-board/ATS host anywhere in the (full) header band warns (`header_url_job_board`) — **unconditionally**, résumé or cover letter, whether or not a contact profile was even supplied (the people most likely to export a raw, unedited header are exactly the ones who never filled one in). Exempts a personal Xing profile.
   - Cover letters keep the original, simpler profile-only form for the mismatch/missing checks — their header override wasn't part of H, so there is no text-derived header to reconstruct against, and no body section can render into a cover letter's header band the way a résumé's can.

## Rationale

**Trade-off accepted:**

| Gain                                                                    | Loss                                                                                                                                                                                                                                                                                                    |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| User edits are preserved; header is not silently overwritten            | The mismatch check verifies the render is self-consistent with whatever the text/profile currently says — it can't tell "the user typed the wrong link" from "the user typed the right link"; catching that class of drift was traded away when the profile stopped being authoritative over user edits |
| Rich customization: users can reorder, drop, or add links to the header | New risk: user accidentally edits the contact line and loses an important link                                                                                                                                                                                                                          |
| Simpler mental model: "edit the document, not the settings"             | The missing-profile-link warning (the closest thing left to a profile-drift signal) only fires when the profile actually supplied the header — once the text has its own, there is no "the profile says X but the text says Y" comparison left to make                                                  |

**Why this trade-off:**

1. Users expect document text to be stable across operations. Silently rewriting the header on every re-export violated that expectation and was surprising (ADR-level surprising: a previous decision claimed the profile was authoritative, but the user had no way to know that or re-read it).

2. Customization is valuable: a user might want to adjust spacing, reorder links, or emphasize certain contact info for a specific role. The current model denied that.

3. The profile is still the starting point: new documents, imports, and re-generations all seed from it. The profile is not forgotten; it's just not authoritative over user edits.

4. Export validation on extracted text (not profile) is correct: the PDF/DOCX export includes what the user sees, and that's what validation should check.

## Consequences

- `contact_profile_header_line()` IPC method is called once per `generateResume()` / `synthesizeResume()` to fetch the profile's header line for seeding
- `seedHeaderFromProfile()` searches the header block — line 0 up to the first blank line, stopping early at the first section boundary — for a line matching the profile's `fullName` (casing/punctuation-insensitively) and overwrites that one line in place. When no line matches, the original fallback still applies: line 0 is replaced when it is name-shaped, and the name is inserted above it otherwise. It then replaces (at most) ONE contact-shaped line on every generation (by design — re-generation is intentional). The in-place reconciliation is the only safe re-scoping of a line the boundary predicate flagged, because `nameKey` can match nothing except the name about to be written there; every other line keeps the never-remove-or-blind-overwrite invariant. Its ceiling: an ALL-CAPS name that does **not** match the profile still falls through to the insert branch and stacks as a visible duplicate — deliberate, since guessing that some other boundary-shaped line is the name would risk destroying a real section heading.
- `apply_to_header()` now explicitly checks for empty fields before applying profile fallbacks
- Export validation calls `extract_section(text, "### CANDIDATE RESUME ###", Some("### JOB ADVERTISEMENT ###"))` to parse header, matching the renderer's extraction
- `header_url_job_board` warning fires when `is_job_board(&url)` detects job-board/ATS hosts (Indeed, Greenhouse, etc.), not LinkedIn/GitHub (which are expected contact links) — checked unconditionally for every document (résumé or cover letter), independent of whether a contact profile was supplied; exempts a personal Xing profile via `is_personal_xing`
- `header_url_mismatch` reconstructs the actually-rendered header (`model_from_resume_text` → `apply_to_header`) and checks only its own topmost N header-band links (N = the header's own expected link count), not the whole 144pt band — a genuine body link rendering early on the page is never mistaken for a header link
- `ContactProfile::header_markdown()` (Rust) is now the single implementation of header construction; shared JSON fixtures (`header-contact-line.json`, `section-names.json`, `all-caps-headings.json`) prevent TS implementation drift
- Tests verify `apply_to_header()` preserves text-derived headers when they exist and only fills blanks

## Alternatives Considered

1. **Keep profile authoritative, improve the warning:** Add a less-disruptive inline hint instead of blocking export. Rejected: the header would still be silently rewritten, and the hint wouldn't appear until export time.

2. **Two-way sync (profile ↔ document):** Offer a UI action to sync the header to/from the profile on demand. Deferred: start with editor ownership; the sync action is a natural future enhancement if users request it.

3. **Validate both:** keep the profile-parity check as a warning and add text-based validation. Rejected: redundant; if the text is valid, that's what matters for the exported document.

## References

- `apps/desktop/src-tauri/src/commands/contact_profile.rs` — `contact_profile_header_line()` IPC command
- `apps/desktop/src-tauri/src/contact_profile/mod.rs` — `header_markdown()` and `apply_to_header()` implementation
- `apps/desktop/src/renderer/lib/generate/generation/generation.ts` — `seedHeaderFromContactProfile()` (the shared, IPC-guarded caller), invoked from both `generateResume()` and `synthesizeResume()`; `seedHeaderFromProfile()` and `pickReplacementIndex()` implement the seeding/selection logic itself
- `apps/desktop/src-tauri/src/export/pdf/mod.rs` — `extract_section(text, start_marker, end_marker)` signature
- `apps/desktop/src-tauri/src/validate/mod.rs` — `pdf_render_issues()`: the reconstructed-header self-consistency check (`header_url_mismatch`), the `header_url_missing` completeness warning (`profile_is_header_source`-gated), and the `header_url_job_board` warning on `is_job_board()`/`is_personal_xing()`
- `packages/prompts/src/generate/text/header-contact-line.ts` — `isHeaderContactLine()` fixture-based parity with Rust
- `packages/prompts/src/fixtures/header-contact-line.json` + `section-names.json` — shared fixtures asserted from both TS and Rust tests
- `docs/knowledge/resume-domain.md` — updated with new header ownership model

## Migration

Existing exported documents are unaffected. Future re-exports of the same document will use `apply_to_header()` fallback only if the text header was missing or blank — otherwise the text is rendered as-is.

## Important Notes

- **Generation overwrites the header intentionally** — re-generating a document will replace the header from the profile. This is by design; if users edit a header and then re-generate, their edits are lost. This is not a bug; it's the expected behavior of re-generation.
- **Export only falls back to the profile** — on export, the text wins. If the user has edited the header in the editor, that edit appears in the PDF/DOCX/TXT, and the profile does not overwrite it (it only fills blanks).
- **Validation parses the actual rendered text** — not the profile, ensuring the gate is live and catches header issues that matter for export.
