# ADR-002: Dual PDF/DOCX backends with golden parity

Last updated: 2026-06-01

**Status:** Accepted · See also [`docs/EXPORT_TEMPLATES.md`](../../EXPORT_TEMPLATES.md)

## Context

Résumés must export to both PDF (pixel-faithful, print) and DOCX (editable, recruiter-friendly), and both must be ATS-safe. A single renderer can't serve both formats well.

## Decision

Maintain **two rendering backends** behind one `ExportRequest`/`ExportResult` contract (`export/types.rs`): PDF via [printpdf][printpdf] + ttf-parser (`export/pdf/`, `export/layout_pdf/`), DOCX via [docx-rs][docx-rs] (`export/docx/`, `export/model_docx/`). Keep outputs in **golden parity** where the design requires, pinned by deterministic golden snapshot tests.

## Consequences

- Format-specific rendering quality without forking the document model (`model/DocumentModel`).
- Golden tests guard visual regressions; non-deterministic snapshots are a finding (`test-author` / `testing-reviewer`).
- Rendering _implementation_ is owned by `pdf-docx-generator`; export _review_ by `resume-export-expert`.

[printpdf]: https://github.com/fschutt/printpdf
[docx-rs]: https://github.com/bokuweb/docx-rs
