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

1. **Generation:** `generateResume()` calls `seedHeaderFromProfile()` which replaces line-0 (name) and all detected contact lines with the profile's values via the `contact_profile_header_line` IPC call. This happens on every generation, seeding the initial content.

2. **Editing:** Users can edit the generated header in the rich-text editor. These edits are part of the document text.

3. **Export:** `apply_to_header()` now uses the profile as a **fallback only**:
   - If `header.name` is blank, fill it from `profile.full_name`
   - If `header.contact` (extracted from text) is empty, fill it from the profile's `header_markdown()`
   - If both already have content, the text wins — the profile doesn't overwrite

4. **Export validation:** The gate now validates the **extracted text**, not the profile:
   - Parses the document via the same `extract_section(text, start_marker, end_marker)` call as the renderer uses (plain string slicing, not a Markdown AST)
   - Validates extracted header URLs are `http(s)` or `mailto:` only
   - Fires a non-blocking `header_url_job_board` warning when a job-board/ATS host (e.g., Indeed, Greenhouse) appears in the header (these belong in the document body, not personal contact info)
   - Removed profile-parity validation gate entirely

## Rationale

**Trade-off accepted:**

| Gain                                                                    | Loss                                                                                               |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| User edits are preserved; header is not silently overwritten            | Profile-parity validation removed; a hand-edited header can drift from the profile without warning |
| Rich customization: users can reorder, drop, or add links to the header | New risk: user accidentally edits the contact line and loses an important link                     |
| Simpler mental model: "edit the document, not the settings"             | Non-blocking warning less visible than a hard validation gate                                      |

**Why this trade-off:**

1. Users expect document text to be stable across operations. Silently rewriting the header on every re-export violated that expectation and was surprising (ADR-level surprising: a previous decision claimed the profile was authoritative, but the user had no way to know that or re-read it).

2. Customization is valuable: a user might want to adjust spacing, reorder links, or emphasize certain contact info for a specific role. The current model denied that.

3. The profile is still the starting point: new documents, imports, and re-generations all seed from it. The profile is not forgotten; it's just not authoritative over user edits.

4. Export validation on extracted text (not profile) is correct: the PDF/DOCX export includes what the user sees, and that's what validation should check.

## Consequences

- `contact_profile_header_line()` IPC method is called once per `generateResume()` to fetch the profile's header line for seeding
- `seedHeaderFromProfile()` replaces name and contact lines on every generation (by design — re-generation is intentional)
- `apply_to_header()` now explicitly checks for empty fields before applying profile fallbacks
- Export validation calls `extract_section(text, "### CANDIDATE RESUME ###", Some("### JOB ADVERTISEMENT ###"))` to parse header, matching the renderer's extraction
- `header_url_job_board` warning fires when `is_job_board(&url)` detects job-board/ATS hosts (Indeed, Greenhouse, etc.), not LinkedIn/GitHub (which are expected contact links)
- `ContactProfile::header_markdown()` (Rust) is now the single implementation of header construction; shared JSON fixtures (`header-contact-line.json`, `section-names.json`) prevent TS implementation drift
- Tests verify `apply_to_header()` preserves text-derived headers when they exist and only fills blanks

## Alternatives Considered

1. **Keep profile authoritative, improve the warning:** Add a less-disruptive inline hint instead of blocking export. Rejected: the header would still be silently rewritten, and the hint wouldn't appear until export time.

2. **Two-way sync (profile ↔ document):** Offer a UI action to sync the header to/from the profile on demand. Deferred: start with editor ownership; the sync action is a natural future enhancement if users request it.

3. **Validate both:** keep the profile-parity check as a warning and add text-based validation. Rejected: redundant; if the text is valid, that's what matters for the exported document.

## References

- `apps/desktop/src-tauri/src/commands/contact_profile.rs` — `contact_profile_header_line()` IPC command
- `apps/desktop/src-tauri/src/contact_profile/mod.rs` — `header_markdown()` and `apply_to_header()` implementation
- `apps/desktop/src/renderer/lib/generate/generation/generation.ts:365-408` — `generateResume()` calls `seedHeaderFromProfile()`
- `apps/desktop/src-tauri/src/export/pdf/mod.rs:19-23` — `extract_section(text, start_marker, end_marker)` signature
- `apps/desktop/src-tauri/src/validate/mod.rs:454-463` — `header_url_job_board` warning on `is_job_board()` match
- `packages/prompts/src/generate/text/header-contact-line.ts` — `isHeaderContactLine()` fixture-based parity with Rust
- `packages/prompts/src/fixtures/header-contact-line.json` + `section-names.json` — shared fixtures asserted from both TS and Rust tests
- `docs/knowledge/resume-domain.md` — updated with new header ownership model

## Migration

Existing exported documents are unaffected. Future re-exports of the same document will use `apply_to_header()` fallback only if the text header was missing or blank — otherwise the text is rendered as-is.

## Important Notes

- **Generation overwrites the header intentionally** — re-generating a document will replace the header from the profile. This is by design; if users edit a header and then re-generate, their edits are lost. This is not a bug; it's the expected behavior of re-generation.
- **Export only falls back to the profile** — on export, the text wins. If the user has edited the header in the editor, that edit appears in the PDF/DOCX/TXT, and the profile does not overwrite it (it only fills blanks).
- **Validation parses the actual rendered text** — not the profile, ensuring the gate is live and catches header issues that matter for export.
