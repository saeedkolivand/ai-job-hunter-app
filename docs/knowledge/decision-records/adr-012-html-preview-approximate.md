# ADR-012: AI-Generate live preview is an approximate HTML mirror; PDF/DOCX remain the source of truth

Last updated: 2026-06-09

**Status:** Accepted

## Context

The AI-Generate wizard (`apps/tauri/src/renderer/features/ai-generate/`) currently
shows the generated résumé/cover letter as prettified markdown text (`OutputPanelDone` →
`EditableOutput`), NOT the actual exported layout.

UX item #24 asks the preview to render the real template and let the user edit in that
template. The 9 templates render in Rust: PDF via printpdf and DOCX via the model_docx
backend. ADR-002 (dual PDF/DOCX backends, golden parity) keeps those two outputs
byte-for-byte aligned via golden snapshots. There is no HTML render path today.

A live, edit-as-you-type preview requires a render path that updates instantly in the
renderer process.

## Decision

Build a live HTML/CSS **mirror** of the templates for the in-app editing preview.

The HTML mirror is explicitly **approximate** — a preview aid for the editing flow only.
The Rust PDF/DOCX renderers remain the **single source of truth** for exported
documents; ADR-002 parity is untouched.

The user keeps a way to see the **real exported PDF** before download (the authoritative
output).

This is a deliberately accepted **third render path**, chosen for editing UX. It is NOT a
replacement of the export pipeline.

**Rejected alternative:** Make HTML the source of truth and generate PDF/DOCX from it
(HTML→PDF), retiring the Rust renderers. Rejected as a massive export-pipeline rewrite
that would reject ADR-002 and its golden-parity guarantees.

## Consequences

- A third render path to maintain → genuine drift risk between the HTML preview and the
  actual PDF/DOCX export. Mitigated by (a) labelling the preview approximate and
  (b) keeping the real-PDF view available before download.
- Template visual specs now live in two places (the Rust renderer and the HTML mirror);
  template changes must update both, or accept that the preview diverges from the export.
- Export renderers (printpdf / model_docx) are unchanged; no impact on existing golden
  snapshots or ADR-002.
- Revisit if drift becomes a recurring user-facing problem — a future move to an
  HTML-source-of-truth pipeline (HTML→PDF) is the escape hatch, but is out of scope here.
