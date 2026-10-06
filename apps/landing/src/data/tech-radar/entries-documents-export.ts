import type { TechRadarEntry } from './types';

export const documentsExportEntries: readonly TechRadarEntry[] = [
  // ── Documents & Export ──────────────────────────────────────────────────
  {
    id: 'typst',
    name: 'Typst (typst / typst-pdf / typst-layout / typst-svg)',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'typst',
    summary: 'One pure-Rust engine renders every résumé and cover-letter PDF.',
    rationale:
      'A single Typst engine backs both documents so résumé and cover-letter output share one layout system instead of two. The whole family is exact-pinned to =0.15.1 in lockstep — a solo version bump anywhere in it is a red flag, not routine maintenance.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'docx-rs',
    name: 'docx-rs',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'docx-rs',
    summary: "Native DOCX generation for the résumé's second export format.",
    rationale:
      "Renders the canonical document model straight to a real two-column DOCX table with native ATS-mode support, guarded by golden invariants so parity between the DOCX and PDF paths doesn't silently drift.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'lopdf',
    name: 'lopdf',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'lopdf',
    summary: 'Low-level PDF manipulation — including inline annotation dicts.',
    rationale:
      "Used where PDF structure needs direct manipulation rather than pure rendering; inline (non-referenced) /Annots dictionaries needed custom parsing since lopdf's own annotation handling assumes the referenced form.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'pdf-extract',
    name: 'pdf-extract',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'pdf-extract',
    summary: 'Text extraction for imported PDF résumés.',
    rationale:
      'Backs step one of the document-import pipeline (format detection → text extraction → SQLite storage → chunking → embedding) for PDF specifically; DOCX and images go through their own dedicated parser/OCR paths.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'image-crate',
    name: 'image (Rust crate)',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'image',
    summary: 'Raster handling for OCR input and export assets.',
    rationale:
      "Exact-pinned alongside the Typst family (=0.25.10) since Typst's own SVG/PDF export path depends on it — kept in lockstep rather than left to float independently.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'dual-engine-parity',
    name: 'Dual-engine golden-parity migration',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'technique',
    summary: 'The legacy renderer stays compiled as the parity reference.',
    rationale:
      "layout_pdf and model_docx are on by default now that the canonical layout engine has snapshot parity with the legacy line-based renderer, but the legacy path stays compiled — it's still the parity reference, and it still renders cover letters — so --no-default-features can fall back to it if the canonical path ever regresses.",
    lastReviewed: '2026-08-05',
  },
];
