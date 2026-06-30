# Export Templates — the resume/cover-letter rendering contract

Last updated: 2026-06-11

The normative reference for the document export system: the nine templates, the
single PDF engine, and the cross-cutting rules (page size, ATS mode, links, fonts,
validation). This is a **contract** — behavior described here is locked by tests;
changing it means changing the tests too.

Source of truth in code:

| Concern                           | Where                                                             |
| --------------------------------- | ----------------------------------------------------------------- |
| Template registry (styling data)  | `apps/desktop/src-tauri/src/export/templates/mod.rs`              |
| Template IDs + serde fallback     | `apps/desktop/src-tauri/src/export/types.rs` (`TemplateId`)       |
| Canonical document model          | `apps/desktop/src-tauri/src/model/`                               |
| PDF engine (Typst adapter)        | `apps/desktop/src-tauri/src/export/typst_engine/`                 |
| DOCX backend (flow)               | `apps/desktop/src-tauri/src/export/docx/`, `export/model_docx.rs` |
| Section placement / two-col rules | `apps/desktop/src-tauri/src/theme/mod.rs`                         |
| Locale profiles (page size, …)    | `apps/desktop/src-tauri/src/locale/mod.rs`                        |
| Cover-letter market conventions   | `apps/desktop/src-tauri/src/locale/letter.rs`                     |
| Validation + ATS gate             | `apps/desktop/src-tauri/src/validate/mod.rs`                      |
| IPC contract                      | `packages/shared/src/ipc/contracts/documents.ts`                  |
| Output languages (renderer SSOT)  | `apps/desktop/src/renderer/lib/generate/locales.ts`               |
| CJK detection (UI notice gate)    | `packages/shared/src/language-detection.ts` (`isCjkLanguage`)     |

---

## Architecture

A resume is rendered from a single canonical `DocumentModel` (header + titled
sections of paragraphs / bullets / entries with rich-text runs). Backends
**translate** the model; they never re-parse text:

```
resume text ──adapter──▶ DocumentModel ──▶ Typst engine  ──▶ PDF   (fixed pages)
                                       └──▶ model_docx    ──▶ DOCX  (Word reflow)
                              TXT = stripped markdown
```

The two backends are **asymmetric by design** and this is intentional:

- **PDF** is a _fixed_ backend — the Typst engine (`typst_engine/`) compiles
  `.typ` template sources (embedded at build time via `include_str!`) with a
  `ResumeWorld` offline world, producing deterministic paginated bytes. No
  network, no disk access at runtime.
- **DOCX** is a _flow_ backend — it emits paragraphs / a borderless table and
  lets Word measure, wrap, and paginate. `keepNext` / `keepLines` keep headings
  with their content and bullets intact.

### Typst adapter isolation boundary

`engine.rs` and `render.rs` are the **only** files that import the `typst` and
`typst_pdf` crates. No `typst` or `typst_pdf` types appear in any `pub` signature
outside `typst_engine/` — callers only see `AppResult<Vec<u8>>`. This keeps the
typst dependency ring-fenced behind the adapter.

The shared layer is `DocumentModel` + `Theme` + `LocaleProfile` + section routing
— **not** a shared paginator.

### Markdown ATX headings (user-created custom sections)

The resume text parser (`parse_line` in `export/parser/mod.rs`) classifies lines
beginning with 1–6 `#` markers plus an ASCII space (`# `, `## `, `### `, …) as
section headers with `LineKind::SectionHeader`, stripping the marker and preserving
inline markdown marks (`**bold**`). This means **any line the WYSIWYG editor emits
as `## Custom Section` will always render as a section heading**, independent of
whether the text matches a known section name (`SECTION_NAMES`) or is ALL-CAPS.

The rule is additive: known-section, ALL-CAPS, thematic-break (`---`), and job-entry
heuristics are unchanged. A bare `# ` with no heading text falls through to blank.

**Significance for the editor↔export contract:** The WYSIWYG editor's markdown
serializer (`packages/ui/src/components/RichTextEditor/markdown.ts`) emits user-created
h2/h3 nodes as `## text` / `### text` lines. The parser's ATX rule guarantees they render
as sections. The significant-whitespace preservation (job-entry date alignment, e.g.
`Senior Engineer␣␣Jan 2020`) is a separate round-trip invariant; see the no-drift
gate in `markdown.roundtrip.test.ts`.

---

## The nine templates

`TemplateId` (kebab-case on the wire) in `export/types.rs`. Unknown / removed IDs
(e.g. stale frontend sending `"two-column"` or `"refined-executive"`) are silently
mapped to `Classic` via the custom `Deserialize` impl — a stale frontend id
degrades gracefully rather than breaking export.

| Id              | Name          | Layout         | Character                                | Best for                                            |
| --------------- | ------------- | -------------- | ---------------------------------------- | --------------------------------------------------- |
| `classic`       | ATS Classic   | Single column  | Black, no color, underlined headings     | Maximum ATS safety; finance / legal / public sector |
| `modern`        | Modern        | Single column  | Navy ruled headings                      | Software / engineering                              |
| `swiss-minimal` | Swiss Minimal | Single column  | Geometric sans, minimal                  | Design-adjacent / product                           |
| `academic`      | Academic      | Single column  | Full serif, formal                       | Academia / research                                 |
| `atelier`       | Atelier       | **Two column** | Shaded sidebar, premium                  | Design; skills-forward                              |
| `meridian`      | Meridian      | Single column  | Full-width tinted header band, airy body | Creative / modern professional                      |
| `throughline`   | Throughline   | Single column  | Timeline spine for experience/projects   | Engineering / product; tenure-story emphasis        |
| `portrait`      | Portrait      | **Two column** | Circular photo top-left, accent keyline  | European market; personal brand                     |
| `lebenslauf`    | Lebenslauf    | Single column  | DACH DIN-style tabular, photo top-right  | German-speaking market                              |

`classic`, `modern`, `swiss-minimal`, `academic` are the original ported set.
`atelier`, `meridian`, `throughline`, `portrait`, `lebenslauf` are premium additions.

Adding a template is **localized and additive**: one `TemplateId` variant + one
`Template::*` constructor in `export/templates/mod.rs` + one `.typ` source under
`export/typst_engine/templates/`. The backends, validation, and locale logic
consume it unchanged.

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
- The DOCX backend therefore emits **no table** in ATS mode; the Typst engine
  lays out a single column.

ATS mode is the answer to position-based parsers (e.g. some modern ATS) that can
still interleave a visually two-column PDF. The recommender suggests it for
conservative fields.

---

## Two-column layout

`Atelier` and `Portrait` are the two two-column templates. Section → column
assignment is the canonical `theme::placement_for` decision (single source of
truth — not a per-template string list):

- **Sidebar**: Skills, Education, Languages, Certifications.
- **Main**: everything else (Summary, Experience, Projects, custom sections).

`theme::is_two_column(id)` is the authoritative boolean gate.

The header (name + contact) always spans the full width above the columns.

- **PDF**: handled inside the respective `.typ` template; each two-column template
  manages its own sidebar band and column flows entirely within Typst.
- **DOCX**: a borderless, single-row two-cell table — a shaded sidebar cell
  (`Shading.fill` = the template tint) + a main cell, fixed layout, borders
  cleared — so Word flows and paginates it.

---

## Cover-letter PDF (`letter.typ`)

`render_letter_pdf` in `typst_engine/engine.rs` compiles a finished cover-letter
text through `letter.typ`. The letter is **not** template-specific; instead it
**inherits visual styling from the chosen resume template** via
`letter_style_from_template` (in `typst_engine/letter.rs`), deriving accent
color, body/name fonts, and font sizes from the resume `Template` registry entry.
It is **market-aware**: `letter::conventions` (from `locale/letter.rs`) provides
`LetterMarketConventions` (date placement, recipient block position, sign-off
style) derived from the job ad's detected locale.

`parse_cover_letter` in `typst_engine/letter.rs` splits the text into
`LetterModel` fields (letterhead / date / recipient / subject / salutation / body /
signoff / signature). The model is serialised to JSON and injected via the Typst
virtual `data.json` — no user content is ever concatenated into Typst markup
(injection-safe).

### Cover-letter template previews (AI-Generate UI)

The AI-Generate template picker surfaces visual preview thumbnails for the
cover-letter rendering, one per resume template. These previews are generated
offline by the `generate_cover_template_previews` test (ignored, run via
`cargo test --lib -- --ignored generate_cover_template_previews`) in
`typst_engine/test.rs`.

Each preview:

- Renders a sample cover letter (US locale, English, reusing `LETTER_FIXTURE_US`)
  through the nine resume templates via the exact same `ResumeWorld` + Typst
  compilation path as `render_letter_pdf` production code.
- Applies the template's visual style (accent, fonts, name_pt/body_pt) via
  `letter_style_from_template`.
- Exports page 1 as **SVG** (vector, zero rasterisation) to
  `apps/desktop/src/renderer/features/ai-generate/assets/cover-template-previews/<slug>.svg`.
- Is consumed by the renderer's `COVER_TEMPLATE_PREVIEWS` Vite glob (in
  `samples/cover-template-previews.ts`), which emits lazy-loaded hashed URLs.
- Mirrors the existing resume `generate_templates_showcase_banner` test
  (which generates PNG previews under `assets/template-previews/`).

The test is a **hard-wall isolation**: `typst` and `typst_svg` crates stay
confined to the test function; no typst types appear in production code paths.
`typst-svg` is a dev-dependency, never shipped.

---

## Live preview (AI-Generate)

The AI-Generate wizard displays a real-time preview of the resume/cover letter as the user
edits the raw text. The preview renders the **exact same Typst document** as the final export
— no approximation, no drift.

**Backend:** `documents_render_preview_images` in `export/commands/mod.rs` parses the request,
compiles the Typst template with the same `DocumentModel` + `ResumeWorld` as the export path,
and emits per-page SVG strings (via `render_resume_svg_pages` / `render_letter_svg_pages` in
`export/typst_engine/render.rs`). The validation gate is omitted for the preview (validation is
redundant; the preview and export follow the same pipeline up to the final emit).

**Frontend:** `renderDocumentPreview()` in `apps/desktop/src/renderer/lib/generate/export/export.ts`
calls the backend, XML-escapes stray `&` characters in SVG link hrefs (Typst leaves them raw for
performance; invalid XML unless escaped), wraps each SVG string in a `Blob` with type
`image/svg+xml`, and returns per-page `blob:` URLs.

**UI:** `PdfPreview` in `apps/desktop/src/renderer/features/ai-generate/components/PdfPreview/`
renders a scrollable container of `<img src=blob:>` elements, one per page, with Blob URL
lifecycle management (revoke on each render batch and on unmount). The preview debounces
~500 ms after same-document edits and re-renders immediately on document switches (résumé ↔
cover letter).

**Rationale:** See ADR-012. SVG via `<img>` is no-script, no-fetch, safe for backend-produced
vector, and requires no CSP `frame-src` (only the existing `img-src 'self' blob:`). The preview
is the authoritative output before download — template changes in Typst automatically appear in
the preview, and any future export changes are reflected immediately.

---

## Candidate photo

The `ContactProfile.photo` field carries an optional candidate photo used by
`Portrait` and `Lebenslauf`. **Only `data:image/<mime>;base64,<payload>` URIs are
accepted.** File paths are rejected unconditionally by `resolve_photo` in
`typst_engine/photo.rs` — there is no path-traversal surface from IPC.

Additional safety measures in `photo.rs`:

- Raw input capped at 10 MB before decoding.
- Only raster formats accepted (PNG/JPEG/WebP/GIF); SVG and HDR rejected.
- Longest edge downscaled to at most 1 200 px before re-encode.
- Output always re-encoded as lossless PNG — strips all EXIF/XMP/ICC metadata.
- All errors swallowed; `resolve_photo` returns `None` and templates render
  without a photo rather than failing.

Client-side pipeline (`apps/desktop/src/renderer/lib/photo.ts`): decodes, square-crops,
downscales, EXIF-strips, and produces a bounded JPEG data URL before the IPC call.
Upload UI lives in `ContactProfileForm`.

---

## Output languages (11 supported)

**Single source of truth:** `apps/desktop/src/renderer/lib/generate/locales.ts`
(`OUTPUT_LANGUAGES`, `VALID_LOCALES`, `SupportedLocale`, `safeLocale`).

Supported locales: en, de, fr, es, it, tr, pt, ru, zh, ja, ko.

Generation and DOCX export work for all 11. **CJK limitation:** The bundled Typst
fonts (Carlito + Noto Sans) support Latin + Cyrillic only; Chinese, Japanese, and
Korean resumes render with character tofu in PDF and live preview. The Resume
Builder flags CJK languages with an amber warning and a font icon. Follow-up: bundle
`Noto Sans CJK` to unblock zh/ja/ko PDF/preview rendering.

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

Two font families are vendored and embedded via `include_bytes!` in the Typst
world (`typst_engine/world.rs`):

| Bundled family                       | Used by                               | License |
| ------------------------------------ | ------------------------------------- | ------- |
| Carlito (Calibri-metric-compatible)  | `classic`, `modern` (body + headings) | OFL     |
| Noto Sans (Latin + Cyrillic subsets) | all templates (body fallback)         | OFL     |

Carlito provides Calibri-metric compatibility so exported PDFs measure identically
to the DOCX Calibri fallback. Noto Sans covers Latin/Cyrillic scripts.

**CJK (zh/ja/ko) limitation:** the bundled fonts do not include CJK glyphs; see
[Output languages](#output-languages-11-supported) above.

The DOCX backend references widely-available system fallbacks (not embedded); OOXML
true embedding is a tracked follow-up.

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

The validate gate uses **content-based** URL checks and reads Typst inline-dict
`/Annots` (`page_annot_dicts`) for link-annotation verification. The old
printpdf-era geometric checks (`empty_anchor_link`, `text_baseline_ys`) have been
removed.

The report (`ok`, `atsMode`, `issues`, `fixed`) rides back on the export result.

---

## TXT

`txt` is produced client-side: the markdown is stripped of `**bold**` markers.
No layout, no validation report.
