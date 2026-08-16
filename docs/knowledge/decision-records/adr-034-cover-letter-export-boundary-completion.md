# ADR-034: Cover-letter export boundary — completion at render time

**Status:** Accepted

Last updated: 2026-08-16

## Context

Cover-letter generation spans two phases: the **pipeline** (running the LLM over a structured job analysis) and **export** (rendering to PDF/DOCX). The prompt instructs the model to omit certain letter parts, delegating them to export time. However, there is no structural enforcement of this split; if generation ever emits a full letter (with salutation + sign-off), and export attempts to add them again, the result is a duplicate — one in the letterhead model, one in the body, visible to the reader.

The letter parser (`parse_cover_letter` in `typst_engine/letter.rs`) operates on the completed letter text and splits it into structured fields (salutation / body / signoff / etc). The parser **stops classifying lines as furniture the moment it sees a salutation** — once `body_started = true`, all subsequent lines are body, even if they look like furniture. If the model emitted a salutation before the body, the parser treats that salutation as furniture (correct). But if the export layer then prepends a second salutation, the model's original one lands in the body text, creating a duplicate the reader sees.

## Decision

The cover-letter export boundary (in `export/commands/mod.rs::validate_and_normalize`) **completes a body-only letter** — it calls `complete_letter_text` to synthesize the market-specific salutation, sign-off, and signature name if they are missing.

**Ownership split:**

- **Generation (model-owned):**
  - Body text (three to five paragraphs of prose).
  - Optional: subject line and date (governed by `pipeline::resume::prompts::letter_system` per market conventions).
- **Export (application-owned):**
  - Salutation (e.g., "Dear Hiring Manager," / "Sehr geehrte Damen und Herren,") — resolved from `locale/letter.rs::conventions(market)`.
  - Sign-off (e.g., "Sincerely," / "Mit freundlichen Grüßen") — resolved from market conventions.
  - Signature name — resolved in `validate_and_normalize` as `meta.candidate_name` (when non-blank) → `ContactProfile.full_name` → empty. The blank rung matters: the tailor surface sends `candidate_name: Some("")`, which is `Some`, not `None`, so every consumer of it needs an explicit non-blank filter to reach the profile fallback.

**Idempotency guard:** `complete_letter_text` detects whether both salutation and sign-off are already present in the input (via `is_salutation` / `is_signoff` line classifiers). If both exist, the function returns the text unchanged — a letter re-exported or imported from an external source is never double-completed.

**Prompt contract:** The pipeline prompt explicitly states: _"Do NOT write a contact header, a salutation line, or a signature block — the application adds them at export time."_ This is a hard boundary; if the prompt ever changes to ask the model to emit these parts, the export completion will produce duplicates. Any future prompt change that adds these to the model's output must be paired with a disabling of the export completion, enforced via a test that verifies the new shape.

## Consequences

1. **All letter exports (PDF, DOCX, preview) inherit completion:** The completion is called in `validate_and_normalize`, which runs before every render. Live previews in the AI-Generate UI, PDF exports, and DOCX exports all see the same completed letter.

2. **Prompt—export coupling is visible, but only partly enforced.** The prompt says "do NOT write X," and the export writes X. The guard that exists is `letter_system_prompt_still_promises_the_export_adds_the_salutation` (`export/typst_engine/test.rs`): it fires when that instruction is **removed or reworded**, and its failure message names `complete_letter_text` as the thing that must be retired with it. It does **not** fire when the model's output shape drifts while the sentence stays — the adversarial body-only fixtures exercise the completion path, but they never invoke `letter_system`, so they cannot detect that drift either. The coupling is intentional — the alternative (silent duplication) is worse.
   - **Update 2026-08-16 — the placeholder half of this gap is now mechanically closed.** A live defect showed the drift this consequence warned about: the model reproduced a German letter template's unfilled slot verbatim ("Ihr Name" — "Your Name" — after the sign-off), and `parse_cover_letter`'s post-signoff rule ("first non-blank non-name line is `signature_title`") promoted it to a rendered title on every German letter. `complete_letter_text`'s idempotency guard did not catch it either, because the model _had_ emitted its own salutation and sign-off — the guard only checks that both are present, not that everything between them and the signature is real content. Two things now close this specific shape of drift: (1) `locale::letter::is_template_placeholder` — a shared predicate (bracket/slot syntax, plus known en/de placeholder tokens, case-insensitive and trailing-punctuation tolerant) that `parse_cover_letter` consults before promoting a post-signoff line to `signature_title`, so the placeholder is dropped rather than rendered; (2) `validate::content::letter`'s new `letter.template_placeholder` (Critical) issue, which scans the rendered letter text with the same predicate and fails the export's content validation if a placeholder slipped through anywhere in the document — the mechanical guard this consequence said did not exist. What remains open: this closes the _placeholder_ shape of drift specifically; a model emitting some other unanticipated furniture shape (not a placeholder, not a duplicate salutation/sign-off) is still only caught by a human re-reading this ADR.

3. **Market conventions are centralized:** Salutations and sign-offs come from one source (`conventions`), shared with the resume export for consistency. A future market-data change applies uniformly.

4. **Cross-stage ordering is critical:** If a future stage (or alternate prompt) opens with a subject line or date BEFORE the body, the salutation must be inserted AFTER that furniture, not at line 0. The completion function scans for these furniture lines and positions the salutation after them, not before. This is the guard against the parser mis-classifying the salutation when other prompt changes add subject/date support.

5. **Repair and re-export:** If a user repairs a letter (e.g., via the humanize stage or manual edit), the completed letter is re-validated and may be re-exported. The idempotency guard ensures multiple completion passes do not accumulate salutations.

6. **Known gap — a marker-wrapped letter missing only its salutation is a silent no-fix.** `complete_letter_text` runs over the whole `request.text`, including any section preceding a `### COMPLETE COVER LETTER ###` marker. For such a document missing only the salutation, the insertion point can land in the pre-marker section, which `extract_section` later discards — the letter renders unchanged rather than corrupted. This lives here as well as in `export/letter_shape.rs`'s module doc because a source comment is refactored away more easily than an ADR consequence, and this boundary's ownership document is where the next reader will look.

## Related

- `apps/desktop/src-tauri/src/export/letter_shape.rs` — `complete_letter_text` function and tests.
- `apps/desktop/src-tauri/src/pipeline/resume/prompts.rs::letter_system` — the prompt that delegates these parts to export.
- `apps/desktop/src-tauri/src/export/typst_engine/letter.rs::parse_cover_letter` — the parser that splits the completed letter into fields.
- `apps/desktop/src-tauri/src/locale/letter.rs::conventions` — market-specific salutations, sign-offs, and other conventions.
- `apps/desktop/src-tauri/src/export/commands/mod.rs::validate_and_normalize` — where completion is called.
- `apps/desktop/src-tauri/src/locale/letter.rs::is_template_placeholder` — the shared placeholder predicate (Consequence #2 update).
- `apps/desktop/src-tauri/src/validate/content/letter.rs` — `letter.template_placeholder`, the mechanical guard added for Consequence #2.
