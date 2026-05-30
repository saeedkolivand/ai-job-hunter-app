# Export Templates — the resume/cover-letter rendering contract

The normative reference for the document export system: the nine templates, the
two backends, and the cross-cutting rules (page size, ATS mode, links, fonts,
validation). This is a **contract** — behavior described here is locked by tests;
changing it means changing the tests too.

Source of truth in code:

| Concern                          | Where                                                      |
| -------------------------------- | ---------------------------------------------------------- |
| Template registry (styling data) | `apps/tauri/src-tauri/src/export/templates/mod.rs`         |
| Canonical document model         | `apps/tauri/src-tauri/src/model/`                          |
| PDF layout engine (fixed)        | `apps/tauri/src-tauri/src/layout/`, `export/layout_pdf.rs` |
| DOCX backend (flow)              | `apps/tauri/src-tauri/src/export/model_docx.rs`            |
| Section placement / link style   | `apps/tauri/src-tauri/src/theme/mod.rs`                    |
| Locale profiles (page size, …)   | `apps/tauri/src-tauri/src/locale/mod.rs`                   |
| Validation + ATS gate            | `apps/tauri/src-tauri/src/validate/mod.rs`                 |
| IPC contract                     | `packages/shared/src/ipc/contracts/documents.ts`           |

---

## Architecture

A resume is rendered from a single canonical `DocumentModel` (header + titled
sections of paragraphs / bullets / entries with rich-text runs). Backends
**translate** the model; they never re-parse text:

```
resume text ──adapter──▶ DocumentModel ──▶ layout engine ──▶ PDF   (fixed pages)
                                       └──▶ model_docx    ──▶ DOCX  (Word reflow)
                              TXT = stripped markdown
```

The two backends are **asymmetric by design** and this is intentional:

- **PDF** is a _fixed_ backend — the engine measures glyphs (real font metrics),
  places every line at an absolute position, paginates itself, and draws the
  two-column sidebar band per page. Deterministic, pixel-stable.
- **DOCX** is a _flow_ backend — it emits paragraphs / a borderless table and
  lets Word measure, wrap, and paginate. `keepNext` / `keepLines` keep headings
  with their content and bullets intact.

The shared layer is `DocumentModel` + `MeasureText` + `Theme` + `LocaleProfile` +
section routing — **not** a shared paginator.

---

## The nine templates

`TemplateId` (kebab-case on the wire) → template. Fonts list `name / heading /
body`; the DOCX backend substitutes a system fallback (see [Fonts](#fonts)).

| Id                  | Name              | Fonts                                            | Character                                    | Layout         | Best for                                            |
| ------------------- | ----------------- | ------------------------------------------------ | -------------------------------------------- | -------------- | --------------------------------------------------- |
| `classic`           | ATS Classic       | Calibri / Calibri / Calibri                      | Black, no color, underlined headings         | Single column  | Maximum ATS safety; finance / legal / public sector |
| `modern`            | Modern Technical  | Calibri / Calibri / Calibri                      | Navy, ruled headings                         | Single column  | Software / engineering                              |
| `executive`         | Executive         | Calibri / Calibri / Calibri                      | Charcoal, centered name, generous whitespace | Single column  | Senior / leadership                                 |
| `editorial-serif`   | Editorial Serif   | Source Serif 4 / Source Serif 4 / Inter          | Indigo accent, op-ed serif                   | Single column  | Editorial / communications                          |
| `swiss-minimal`     | Swiss Minimal     | Manrope / Manrope / Manrope                      | Geometric sans, minimal                      | Single column  | Design-adjacent / product                           |
| `two-column`        | Two Column        | Inter / Inter / Inter                            | Shaded sidebar band                          | **Two column** | Design; skills-forward                              |
| `mono-technical`    | Mono Technical    | JetBrains Mono / JetBrains Mono / Inter          | Monospace headings                           | Single column  | Systems / low-level engineering                     |
| `refined-executive` | Refined Executive | Playfair Display / Inter / Inter                 | Display-serif name                           | Single column  | Executive / brand-forward                           |
| `academic`          | Academic          | Source Serif 4 / Source Serif 4 / Source Serif 4 | Full serif, formal                           | Single column  | Academia / research                                 |

Adding a template is **localized and additive**: one `TemplateId` variant + one
`Template::*` constructor in the registry. The backends, validation, and locale
logic consume it unchanged.

---

## Page size & locale

Page geometry comes from the request's locale (`LocaleProfile::get`), resolved to
**one source of truth** read by every backend:

- **US** market → **US Letter** (215.9 × 279.4 mm).
- Every other market and the omitted default → **A4** (210 × 297 mm).

The recommender derives the locale from the job ad (explicit target country wins,
then an `en-US` / `en-GB` region subtag, then the language). PDF and DOCX always
agree on the page size for a given request.

---

## ATS mode

`atsMode` makes the document parser-safe:

- The model is **linearized** (`transform::linearize`) into a single canonical
  reading order, and two-column templates collapse to one column.
- The DOCX backend therefore emits **no table** in ATS mode; the PDF backend lays
  out a single column.

ATS mode is the answer to position-based parsers (e.g. some modern ATS) that can
still interleave a visually two-column PDF. The recommender suggests it for
conservative fields.

---

## Two-column layout

Only `two-column` is two-column. Section → column assignment is the canonical
`theme::placement_for` decision (not a per-template string list):

- **Sidebar**: Skills, Education, Languages, Certifications.
- **Main**: everything else (Summary, Experience, Projects, custom sections).

The header (name + contact) always spans the full width above the columns.

- **PDF**: a shaded sidebar band drawn per page behind an independent sidebar
  flow; the main flow paginates separately and pages are merged.
- **DOCX**: a borderless, single-row two-cell table — a shaded sidebar cell
  (`Shading.fill` = the template tint) + a main cell, fixed layout, borders
  cleared — so Word flows and paginates it.

---

## Links

Contact and body hyperlinks are first-class rich-text runs, rendered as real
clickable links (PDF `/Link /URI` annotations; DOCX `w:hyperlink` with the URL in
the relationships part). The visible label is shown, never the raw URL.

`theme::link_style` per template:

- `classic` → body color, **no** underline (maximum parser/printer safety).
- all others → accent color + underline.

---

## Fonts

Six families are bundled as TTFs and embedded in the PDF. The DOCX is **not**
embedded yet, so it references a widely-available fallback so output is
predictable on machines without the bundled fonts (true OOXML embedding is a
tracked follow-up):

| Bundled family   | DOCX fallback |
| ---------------- | ------------- |
| Calibri          | Calibri       |
| Inter            | Calibri       |
| Manrope          | Calibri       |
| Source Serif 4   | Georgia       |
| Playfair Display | Cambria       |
| JetBrains Mono   | Consolas      |

The fallback is applied to both the ASCII and high-ANSI ranges so accented Latin
(common in DACH names) renders in the same face.

---

## Validation + ATS gate

Every PDF/DOCX export runs through `validate::validate_and_fix` after rendering:

1. **Round-trip** — the bytes are re-extracted (pdf-extract for PDF, the unzipped
   `document.xml` for DOCX) and checked for content survival and sane reading
   order.
2. **Auto-fix** — a two-column layout whose sections interleave when read back is
   re-exported single-column (ATS-safe) and re-checked.
3. **Block** — only when a critical defect _survives_ auto-fix (e.g. no
   extractable text at all). Missing name/email/section and single-column order
   quirks are warnings, never blocks.

The report (`ok`, `atsMode`, `issues`, `fixed`) rides back on the export result.

---

## TXT

`txt` is produced client-side: the markdown is stripped of `**bold**` markers.
No layout, no validation report.
