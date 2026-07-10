use serde::{Deserialize, Serialize};

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Docx,
    Pdf,
    Txt,
}

/// Template ID for styling.
/// Serde uses kebab-case so "swiss-minimal" etc. round-trip correctly.
/// Single-word IDs (classic, academic) are unaffected.
///
/// Unknown / removed IDs (e.g. "modern", "two-column", "refined-executive",
/// "bogus") are silently mapped to `Classic` via the custom `Deserialize` impl
/// below — a stale frontend id degrades gracefully rather than breaking export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateId {
    #[default]
    Classic,
    SwissMinimal,
    Academic,
    /// Premium two-column sidebar template — Atelier design (Phase 1b).
    Atelier,
    /// Phase 3a premium single-column: full-width tinted header band, airy body.
    Meridian,
    /// Phase 3a premium single-column: timeline spine for experience/projects.
    Throughline,
    /// Phase 3b-i: two-column photo template — circular photo top-left, name/title
    /// stacked right, accent keyline, sidebar for contact/skills/education.
    Portrait,
    /// Phase 3b-i: DACH DIN-style tabular CV — photo top-right, formal A4,
    /// left-label / right-value rows, restrained accent.
    Lebenslauf,
    /// PR3: Cadence — Claude-PDF-style ATS single-column (Inter, blue-grey accent,
    /// letter-spaced all-caps ruled headings, underlined links). Renders through
    /// the parametric `single_column.typ`.
    Cadence,
    /// PR3: Regent — executive serif ATS single-column (Source Serif 4, burgundy
    /// small-caps headings, rose rule, first-line-indent letter). Renders through
    /// the parametric `single_column.typ`.
    Regent,
    /// PR4: Aria — minimalist design two-column with an untinted RIGHT sidebar and
    /// a rectangular top-right photo (Manrope name, Inter body, slate accent,
    /// letter-spaced caps headings). Education reads in the main column. Renders
    /// through the bespoke `aria.typ`.
    Aria,
    /// PR4: Saffron — warm design two-column with a tinted LEFT sidebar and a
    /// circular ringed photo (Source Serif 4 small-caps headings, Inter body,
    /// terracotta accent). Certifications read in the main column. Renders through
    /// the bespoke `saffron.typ`.
    Saffron,
}

impl<'de> serde::Deserialize<'de> for TemplateId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(match s.as_str() {
            "classic" => TemplateId::Classic,
            // "modern" was removed (deduped into the parametric single-column
            // family); a saved/stale "modern" id maps to Classic, not an error.
            "modern" => TemplateId::Classic,
            "swiss-minimal" => TemplateId::SwissMinimal,
            "academic" => TemplateId::Academic,
            "atelier" => TemplateId::Atelier,
            "meridian" => TemplateId::Meridian,
            "throughline" => TemplateId::Throughline,
            "portrait" => TemplateId::Portrait,
            "lebenslauf" => TemplateId::Lebenslauf,
            "cadence" => TemplateId::Cadence,
            "regent" => TemplateId::Regent,
            "aria" => TemplateId::Aria,
            "saffron" => TemplateId::Saffron,
            // Any unknown / removed id (e.g. "two-column", "refined-executive",
            // "executive", "editorial-serif", "mono-technical", "bogus") falls
            // back to Classic so a stale frontend never breaks export.
            _ => {
                log::warn!("TemplateId: unknown id {:?}, falling back to Classic", s);
                TemplateId::Classic
            }
        })
    }
}

/// Cover-letter **layout** (canonical term — NOT "letter template").
///
/// A *layout* owns only the **arrangement/composition** of the letter. The
/// palette and fonts are NOT chosen here — they always inherit from the
/// selected résumé [`TemplateId`] via
/// `crate::export::typst_engine::letter::style_from_template`, so a letter keeps
/// matching its résumé family. Market conventions (date position, subject line)
/// still own the WHAT/WHERE semantics; where a convention and the layout's
/// arrangement conflict, the convention wins (e.g. DE DIN date-top-right).
///
/// Serde uses kebab-case (`"classic"` / `"refined"` / `"banded"`). Unknown /
/// removed ids fall back to `Classic` via the custom `Deserialize` impl below —
/// mirroring [`TemplateId`] so a stale frontend id degrades gracefully rather
/// than breaking export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum LetterLayout {
    /// The original single `letter.typ` arrangement. Default, so a request that
    /// omits the field renders byte-identically to the pre-layout-picker output.
    #[default]
    Classic,
    /// Olivia-Wilson minimalist: large sans name + role top-left, right-aligned
    /// contact, horizontal rule, always-visible job-reference line, spaced
    /// signature. Source: `letter_refined.typ`.
    Refined,
    /// Belinda-Davidson: angled pale accent band across the top of page 1
    /// (decorative, behind text), serif small-caps name, stacked right contact,
    /// short rule footer. Source: `letter_banded.typ`.
    Banded,
}

impl<'de> serde::Deserialize<'de> for LetterLayout {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(match s.as_str() {
            "classic" => LetterLayout::Classic,
            "refined" => LetterLayout::Refined,
            "banded" => LetterLayout::Banded,
            // Any unknown / removed id falls back to Classic so a stale frontend
            // never breaks cover-letter export.
            _ => {
                log::warn!("LetterLayout: unknown id {:?}, falling back to Classic", s);
                LetterLayout::Classic
            }
        })
    }
}

/// Document type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentType {
    Resume,
    CoverLetter,
}

/// Font family selector — shared between template config and the Typst rendering engine.
/// Defined here (types) to keep `templates` dependency-light (no direct dep on the engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFamily {
    Calibri,
    Inter,
    SourceSerif4,
    Manrope,
}

/// Metadata for generation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationMeta {
    pub candidate_name: Option<String>,
    pub job_title: Option<String>,
    pub company_name: Option<String>,
    pub target_language: Option<String>,
}

/// Export request from frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub text: String,
    pub format: ExportFormat,
    pub document_type: DocumentType,
    pub template_id: TemplateId,
    pub meta: Option<GenerationMeta>,
    /// Linearize two-column layouts for ATS parsers.
    /// Defaults to false so existing frontends that omit it keep working.
    #[serde(default)]
    pub ats_mode: bool,
    /// Target market id (`us`, `uk`, `de`, …) resolved by
    /// [`crate::locale::LocaleProfile::get`]; drives the page size (US → Letter,
    /// the rest → A4). Optional so frontends that omit it keep the A4 default.
    #[serde(default)]
    pub locale: Option<String>,
    /// User contact profile — the single source of truth for the header contact
    /// line (name fields → `[Label](url)`), localized per language. When present
    /// it overrides whatever links the generated text carried, so a personal
    /// LinkedIn / Website can never be displaced by a company-link. Optional so
    /// older frontends keep the text-derived header.
    #[serde(default)]
    pub contact: Option<crate::contact_profile::ContactProfile>,
    /// Per-export **document accent** (ADR 0004): an optional 6-digit hex
    /// (`#RRGGBB` or bare `RRGGBB`) that recolors the chosen template's accent.
    /// Distinct from the app-UI accent — it never reads `ThemePrefs`. `None`
    /// (the default) leaves the template's built-in palette untouched; a
    /// malformed value is ignored. Validated by
    /// [`crate::export::typst_engine`]'s `normalise_accent` on the résumé-PDF
    /// path and by [`crate::export::templates::Template::with_accent_override`]
    /// on the letter / DOCX paths.
    #[serde(default)]
    pub accent: Option<String>,
    /// Cover-letter **layout** — the arrangement/composition of the letter,
    /// independent of the résumé [`TemplateId`] (which still supplies the
    /// palette + fonts via `style_from_template`). `Classic` (the default) is
    /// the original single-`letter.typ` arrangement, so a request that omits the
    /// field keeps the pre-layout-picker output. Ignored for résumé exports.
    ///
    /// Wire name is `letterLayoutId` (the shared TS contract field) rather than
    /// the camelCase default so the frontend picker's value binds correctly.
    #[serde(default, rename = "letterLayoutId")]
    pub letter_layout: LetterLayout,
}

impl ExportRequest {
    /// Page geometry for this request's locale (international A4 by default).
    pub fn page_geometry(&self) -> crate::locale::PageGeometry {
        crate::locale::LocaleProfile::get(self.locale.as_deref().unwrap_or("en")).page_geometry()
    }

    /// Language the document is written in (drives the localized header location).
    /// Falls back to the request locale, then `en`.
    pub fn target_lang(&self) -> String {
        self.meta
            .as_ref()
            .and_then(|m| m.target_language.as_deref())
            .filter(|s| !s.is_empty())
            .or(self.locale.as_deref())
            .unwrap_or("en")
            .to_string()
    }
}

/// Export result (binary data)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub filename: String,
    /// Pre-export validation report (PDF/DOCX). `None` for TXT, which has no
    /// layout to validate. Optional on the wire so older frontends keep working.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<crate::validate::ExportReport>,
}

/// Live-preview result: per-page rendered images (no file bytes, no filename).
///
/// Returned by `documents_render_preview_images`. `pages` is one rendered page
/// per element; `mime_type` is always `image/svg+xml` for the SVG render path.
/// The renderer shows each page via `<img>` (CSP `img-src 'self' data: blob:`),
/// avoiding the `frame-src blob:` dependency the PDF→iframe preview needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    /// One rendered page per element (SVG document strings, in page order).
    pub pages: Vec<String>,
    /// MIME type of every page string (`image/svg+xml`).
    pub mime_type: String,
}

/// Line kind in parsed document
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Name,
    Contact,
    SectionHeader,
    JobEntry,
    JobTitle,
    Bullet,
    Text,
    Blank,
}

/// Text segment with optional bold formatting
#[derive(Debug, Clone)]
pub struct TextSegment {
    pub text: String,
    pub bold: bool,
}

/// Parsed line with metadata
#[derive(Debug, Clone)]
pub struct ParsedLine {
    pub kind: LineKind,
    pub raw: String,
    pub text: String,
    pub segments: Vec<TextSegment>,
    pub right_text: Option<String>, // For job entries (date)
}

/// Parsed document structure
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub lines: Vec<ParsedLine>,
    pub has_name: bool,
    pub has_contact: bool,
    pub section_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stale frontend that sends a removed template id (e.g. "two-column",
    /// "refined-executive") or a completely unknown string must NEVER cause a
    /// deserialisation error — it must silently fall back to `Classic`.
    #[test]
    fn unknown_template_id_falls_back_to_classic() {
        for bad in &[
            "modern",
            "two-column",
            "refined-executive",
            "executive",
            "editorial-serif",
            "mono-technical",
            "bogus",
            "BOGUS",
            "",
        ] {
            let json = format!("\"{}\"", bad);
            let id: TemplateId = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("deserialise {:?} failed: {e}", bad));
            assert_eq!(
                id,
                TemplateId::Classic,
                "unknown id {:?} should fall back to Classic, got {id:?}",
                bad
            );
        }
    }

    /// Live ids must still round-trip correctly (no regression).
    #[test]
    fn live_template_ids_round_trip() {
        let cases = [
            (TemplateId::Classic, "\"classic\""),
            (TemplateId::SwissMinimal, "\"swiss-minimal\""),
            (TemplateId::Academic, "\"academic\""),
            (TemplateId::Atelier, "\"atelier\""),
            (TemplateId::Meridian, "\"meridian\""),
            (TemplateId::Throughline, "\"throughline\""),
            (TemplateId::Portrait, "\"portrait\""),
            (TemplateId::Lebenslauf, "\"lebenslauf\""),
            (TemplateId::Cadence, "\"cadence\""),
            (TemplateId::Regent, "\"regent\""),
            (TemplateId::Aria, "\"aria\""),
            (TemplateId::Saffron, "\"saffron\""),
        ];
        for (id, expected_json) in cases {
            let serialized = serde_json::to_string(&id).expect("serialize");
            assert_eq!(serialized, expected_json, "{id:?} serialized wrong");
            let deserialized: TemplateId = serde_json::from_str(&serialized).expect("deserialize");
            assert_eq!(deserialized, id, "{id:?} did not round-trip");
        }
    }

    /// All three letter layouts round-trip through kebab-case serde.
    #[test]
    fn letter_layout_round_trips() {
        let cases = [
            (LetterLayout::Classic, "\"classic\""),
            (LetterLayout::Refined, "\"refined\""),
            (LetterLayout::Banded, "\"banded\""),
        ];
        for (layout, expected_json) in cases {
            let serialized = serde_json::to_string(&layout).expect("serialize");
            assert_eq!(serialized, expected_json, "{layout:?} serialized wrong");
            let deserialized: LetterLayout =
                serde_json::from_str(&serialized).expect("deserialize");
            assert_eq!(deserialized, layout, "{layout:?} did not round-trip");
        }
    }

    /// An unknown / removed letter-layout id must never error — it falls back to
    /// `Classic`, mirroring `TemplateId`'s graceful degradation.
    #[test]
    fn unknown_letter_layout_falls_back_to_classic() {
        for bad in &["olivia", "belinda", "two-column", "bogus", "BANDED", ""] {
            let json = format!("\"{}\"", bad);
            let layout: LetterLayout = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("deserialise {:?} failed: {e}", bad));
            assert_eq!(
                layout,
                LetterLayout::Classic,
                "unknown layout {:?} should fall back to Classic, got {layout:?}",
                bad
            );
        }
    }

    /// The default (used by `#[serde(default)]` when the field is absent) is
    /// `Classic` — the pre-layout-picker output.
    #[test]
    fn letter_layout_default_is_classic() {
        assert_eq!(LetterLayout::default(), LetterLayout::Classic);
    }

    /// The wire field is `letterLayoutId` (shared TS contract), and an absent
    /// field defaults to `Classic` so existing cover-letter requests are
    /// unaffected.
    #[test]
    fn export_request_reads_letter_layout_id_and_defaults() {
        let with_layout: ExportRequest = serde_json::from_str(
            r#"{"text":"x","format":"pdf","documentType":"cover-letter",
                "templateId":"classic","meta":null,"letterLayoutId":"banded"}"#,
        )
        .expect("deserialize request with letterLayoutId");
        assert_eq!(with_layout.letter_layout, LetterLayout::Banded);

        let without: ExportRequest = serde_json::from_str(
            r#"{"text":"x","format":"pdf","documentType":"cover-letter",
                "templateId":"classic","meta":null}"#,
        )
        .expect("deserialize request without letterLayoutId");
        assert_eq!(without.letter_layout, LetterLayout::Classic);
    }
}
