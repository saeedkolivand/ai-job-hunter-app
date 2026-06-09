# ADR-012: AI-Generate live preview renders the real exported PDF; templates stay single-source

Last updated: 2026-06-09

**Status:** Accepted (revised during implementation)

## Context

The AI-Generate wizard (`apps/tauri/src/renderer/features/ai-generate/`) currently
shows the generated résumé/cover letter as prettified markdown text (`OutputPanelDone` →
`EditableOutput`), NOT the actual exported layout.

UX item #24 asks the preview to render the real template and let the user edit in that
template. The 9 templates render in Rust: PDF via printpdf and DOCX via the model_docx
backend. ADR-002 (dual PDF/DOCX backends, golden parity) keeps those two outputs
byte-for-byte aligned via golden snapshots.

During implementation we discovered that the Rust export command `documents_export_document`
(exposed in the renderer as `documents.exportDocument`) already returns the rendered PDF
bytes (`ExportResult { data: Uint8Array, mimeType, ... }`) without saving to disk. This
makes a **real-PDF live preview cheap and perfectly faithful** — zero drift, because it
IS the export pipeline.

## Decision

The AI-Generate live preview renders the **real exported PDF** from the Rust renderer
(the same pipeline as the final export), re-rendering ~500ms after edits settle
(debounced). The raw textarea remains the edit surface; the preview pane shows a
`<iframe>` with the live PDF.

**This supersedes the original ADR-012 decision** (approximate HTML/CSS mirror). Reasoning:

- The real-PDF preview is perfectly faithful (zero drift — it IS the export).
- No second render path or duplicated template specs.
- Far less code than a 9-template HTML mirror + a text→sections parser.
- ADR-002 (dual PDF/DOCX parity) is untouched; the preview reuses the existing renderer.

**Rejected alternative (original):** Build an approximate HTML/CSS mirror of the 9 templates.
Rejected during implementation because the real-PDF approach is cheaper, more faithful,
and eliminates the drift risk entirely.

**Rejected alternative (long-term escape hatch):** Make HTML the source of truth and
generate PDF/DOCX from it (HTML→PDF). Out of scope; ADR-002 golden-parity guarantees
are unchanged.

## Consequences

- The live preview is the **actual export output**, debounced per keystroke. Cost:
  each refresh is a Rust round-trip (acceptable for a preview; not per-keystroke).
- No HTML render path to maintain → zero drift between preview and export.
  Template changes update the Rust renderer once; the preview automatically reflects them.
- **CSP allowance:** `frame-src 'self' blob:` added to `apps/tauri/src-tauri/tauri.conf.json`
  so the PDF `blob:` URL can load in an `<iframe>`. Blob URLs are same-origin and app-generated
  (low risk); `tauri-security-reviewer` owns the CSP surface.
- Export renderers (printpdf / model_docx) are unchanged; ADR-002 golden snapshots untouched.
- Implementation pointers: `renderPdfPreview()` in `apps/tauri/src/renderer/lib/generate/export/export.ts`;
  `PdfPreview` component in `apps/tauri/src/renderer/features/ai-generate/components/PdfPreview/`;
  `EditableOutput` gained optional `previewSlot`; `OutputPanelDone` supplies it.
