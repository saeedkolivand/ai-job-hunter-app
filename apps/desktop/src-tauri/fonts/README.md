# Fonts for PDF Export

This directory may optionally contain the Calibri font files:

- `calibri.ttf` — Calibri Regular
- `calibrib.ttf` — Calibri Bold

## Where to get Calibri fonts

Calibri is a Microsoft font included with Windows and Microsoft Office.

### Option 1: Use system fonts (Windows)

Copy from: `C:\Windows\Fonts\`

### Option 2: Use alternative open-source fonts

If Calibri is not available, you can use these alternatives:

- **Carlito** (Calibri metric-compatible, already bundled — see below)
- **Liberation Sans**
- **Arial**

### Option 3: Download from Microsoft

Calibri is part of the Microsoft Office suite.

## License Note

Calibri is proprietary. Ensure you have the right to distribute it if bundling with the app.
For open-source distribution, use metric-compatible alternatives like Carlito.

---

## Carlito (vendored)

Carlito is a Calibri-metric-compatible open font bundled for the Typst rendering engine:

- `carlito_regular.ttf` — Carlito Regular
- `carlito_bold.ttf` — Carlito Bold
- `carlito_italic.ttf` — Carlito Italic
- `carlito_bolditalic.ttf` — Carlito Bold Italic

Source: https://github.com/google/fonts/tree/main/ofl/carlito

License: SIL Open Font License 1.1 (OFL-1.1). Copyright 2010, 2012 Google Corp.
with Reserved Font Name "Carlito". Full license text: https://scripts.sil.org/OFL

---

## Vendored Typst engine fonts

The following 11 fonts are compiled into the binary via `include_bytes!` in
`src/export/typst_engine/world.rs`. They are required for PDF export and must
be present in this directory.

| File                        | Family         | Style       | Used by                              |
| --------------------------- | -------------- | ----------- | ------------------------------------ |
| `carlito_regular.ttf`       | Carlito        | Regular     | Calibri-metric-compatible fallback   |
| `carlito_bold.ttf`          | Carlito        | Bold        | Calibri-metric-compatible fallback   |
| `carlito_italic.ttf`        | Carlito        | Italic      | Calibri-metric-compatible fallback   |
| `carlito_bolditalic.ttf`    | Carlito        | Bold Italic | Calibri-metric-compatible fallback   |
| `inter_regular.ttf`         | Inter          | Regular     | Templates with good Unicode coverage |
| `inter_bold.ttf`            | Inter          | Bold        | Templates with good Unicode coverage |
| `source_serif4_regular.ttf` | Source Serif 4 | Regular     | Editorial / academic serif templates |
| `source_serif4_bold.ttf`    | Source Serif 4 | Bold        | Editorial / academic serif templates |
| `source_serif4_italic.ttf`  | Source Serif 4 | Italic      | Editorial / academic serif templates |
| `manrope_regular.ttf`       | Manrope        | Regular     | Swiss Minimal template               |
| `manrope_bold.ttf`          | Manrope        | Bold        | Swiss Minimal template               |

### Also fetched by `download-fonts.ps1` but NOT compiled into the binary

`jetbrains_mono_regular.ttf`, `jetbrains_mono_bold.ttf`, `playfair_display_regular.ttf`,
`playfair_display_bold.ttf` — present in this directory but not referenced by `include_bytes!`
in `world.rs` and therefore not bundled into the app binary.

Run `apps/desktop/src-tauri/fonts/download-fonts.ps1` to download all font files.
