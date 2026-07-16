# ADR-012: Live preview renders the real exported document via SVG; templates stay single-source

Last updated: 2026-07-16

**Status:** Accepted (revised during implementation)

## Context

The AI-Generate wizard (`apps/desktop/src/renderer/features/ai-generate/`) initially showed
generated résumés/cover letters as prettified markdown text only. UX #24 requested a real
template preview so users could edit while seeing the actual layout.

The résumé templates render in Rust: PDF via Typst engine (Typst §2), DOCX via model_docx.
ADR-002 (dual PDF/DOCX backends, golden parity) keeps those outputs byte-for-byte aligned.

During implementation we discovered that a **preview-only SVG render path** is cheaper and
more faithful than a PDF-in-iframe (which adds CSP surface and bloats the live-edit loop).

## Decision

The AI-Generate live preview renders **SVG page images** from the Rust Typst engine's
`render_resume_svg_pages` / `render_letter_svg_pages` paths. These emit the _exact same_
document structure as the PDF export — same Typst world, same template compilation, same
validation — but stop at vector SVG instead of PDF bytes.

- **Backend command:** `documents_render_preview_images` in `apps/desktop/src-tauri/src/export/commands/mod.rs`
  → parses request, compiles template, renders per-page SVG, returns `RenderPreviewImagesResult { pages: Vec<String> }`.
- **Frontend:** `renderDocumentPreview()` in `apps/desktop/src/renderer/lib/generate/export/export.ts`
  calls the backend, XML-escapes stray `&` in SVG hrefs (Typst leaves them raw), wraps each SVG
  string in a `Blob([svg], { type: 'image/svg+xml' })`, and returns `<img src=blob:>` URLs.
- **UI:** `PdfPreview` in `apps/desktop/src/renderer/components/generation/PdfPreview/`
  renders a scrollable stack of `<img>` elements, with 500ms debounce on same-doc edits and
  zero-debounce on doc switches (résumé ↔ cover letter).

**This supersedes the earlier PDF-in-iframe design.** Reasoning:

- SVG `<img>` is no-script, no-fetch (safe from backend-produced vector).
- Removes CSP `frame-src 'self' blob:` surface; `img-src 'self' blob:` is a tighter gate.
- Simpler memory model: Blob URL revoke on each render batch and on unmount.
- Typst SVG output is deterministic and pixel-faithful (same engine as export).

**Rejected alternatives:**

1. **PDF-in-iframe:** Requires CSP `frame-src 'self' blob:` + iframe sandboxing. SVG avoids both.
2. **HTML/CSS mirror:** Would drift from Typst templates; this approach is zero-drift.
3. **Skip validation for preview:** Preview reuses the export validate gate (validation is cheap;
   auto-fix skipped for preview, but lint runs).

## Consequences

- **Preview is the authoritative output** — it IS the Typst render, not an approximation.
- Each edit triggers a Rust round-trip (cost acceptable for ~700ms debounce; covers both resume and letter).
  **Debounce amendment (2026-06-22):** The explicit Save button is no longer required. Local edits now
  commit to the preview on a ~700ms debounce (`useDebouncedCommit` hook), while generation/regeneration
  commits immediately. This preserves the intent of the earlier save-gating (no per-keystroke Typst
  recompiles) while removing the manual step. An "Updating preview…" hint shows while a commit is pending.
- No HTML render path → zero drift. Template changes in Typst automatically appear in the preview.
- **SVG sanitization:** Typst leaves raw `&` in link hrefs. Before wrapping as `<img>`, escape them
  to `&amp;` so the XML parses. See `escapeSvgAmpersands()` in `export/export.ts`.
- **No CSP frame-src needed.** Only `img-src 'self' blob:` (which already exists).
- Cover-letter previews inherit the resume template's visual style via `letter_style_from_template`
  (same as export).
- Typst dependency remains ring-fenced: only `export/typst_engine/` imports `typst` and `typst_svg` crates;
  SVG generation is isolated from IPC contracts.
- ADR-002 (golden PDF/DOCX parity) is unchanged; export pipeline untouched.

## Implementation pointers

- `renderDocumentPreview()` – `apps/desktop/src/renderer/lib/generate/export/export.ts`
- `PdfPreview` – `apps/desktop/src/renderer/components/generation/PdfPreview/`
- `useDebouncedCommit()` – `apps/desktop/src/renderer/hooks/use-debounced-commit/use-debounced-commit.ts` (debounced local-edit commit)
- `OutputPanelDone` – `apps/desktop/src/renderer/features/ai-generate/components/OutputPanelDone/index.tsx` (integration point)
- `documents_render_preview_images` – `apps/desktop/src-tauri/src/export/commands/mod.rs`
- `render_resume_svg_pages` / `render_letter_svg_pages` – `apps/desktop/src-tauri/src/export/typst_engine/render.rs`
