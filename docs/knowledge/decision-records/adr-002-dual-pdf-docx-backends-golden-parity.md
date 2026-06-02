# ADR-002: Dual PDF/DOCX backends with golden parity

Last updated: 2026-06-02

**Status:** Accepted · See also [`docs/EXPORT_TEMPLATES.md`](../../EXPORT_TEMPLATES.md)

## Context

Résumés must export to both PDF (pixel-faithful, print) and DOCX (editable, recruiter-friendly), and both must be ATS-safe. A single renderer can't serve both formats well.

## Decision

Maintain **two rendering backends** behind one `ExportRequest`/`ExportResult` contract (`export/types.rs`): PDF via the Typst adapter (`export/typst_engine/`), DOCX via [docx-rs][docx-rs] (`export/docx/`, `export/model_docx.rs`). Keep outputs in **golden parity** where the design requires, pinned by deterministic golden snapshot tests.

The PDF backend was migrated from printpdf to Typst in the `feat/typst-premium-resume-templates` branch. The Typst adapter is the sole PDF engine going forward; printpdf and ttf-parser are removed from `Cargo.toml`.

## Consequences

- Format-specific rendering quality without forking the document model (`model/DocumentModel`).
- Golden tests guard visual regressions; non-deterministic snapshots are a finding (`test-author` / `testing-reviewer`).
- Rendering _implementation_ is owned by `pdf-docx-generator`; export _review_ by `resume-export-expert`.
- The Typst adapter isolation boundary (only `engine.rs` + `render.rs` import typst crates) keeps the dependency ring-fenced.

[docx-rs]: https://github.com/bokuweb/docx-rs
