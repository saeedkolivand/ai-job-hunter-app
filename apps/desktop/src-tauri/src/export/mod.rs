/*!
 * Export module — résumé and cover-letter generation (PDF + DOCX + TXT).
 *
 * Sub-modules:
 * - `typst_engine/` — sole PDF engine (Typst adapter); all eight templates rendered here.
 * - `pdf/`          — `generate_pdf` / `generate_preview_svg` entry points.
 * - `docx/`         — `generate_docx` entry point; cover-letter DOCX renderer.
 * - `model_docx`    — model-based résumé DOCX renderer (canonical résumé-DOCX path).
 * - `docx_renderer` — shared DOCX primitive helpers (runs, colors, paragraphs).
 * - `parser/`       — flat résumé text parser → `ParsedDocument` / `ParsedLine`.
 * - `templates/`    — template registry, styling, spacing rules.
 * - `links/`        — URL splitting for clickable hyperlinks in DOCX.
 * - `types`         — shared types: `ExportRequest`, `ExportResult`, `TemplateId`, …
 * - `commands/`     — Tauri command wrappers (`documents_export_document`, …).
 */

pub mod commands;
pub mod docx;
pub mod docx_renderer;
pub mod links;
pub mod model_docx;
pub mod parser;
pub mod pdf;
pub mod templates;
pub mod types;
pub mod typst_engine;
