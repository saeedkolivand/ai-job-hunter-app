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
/// Single-word IDs (classic, modern, academic) are unaffected.
///
/// Unknown / removed IDs (e.g. "two-column", "refined-executive", "bogus") are
/// silently mapped to `Classic` via the custom `Deserialize` impl below —
/// a stale frontend id degrades gracefully rather than breaking export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateId {
    #[default]
    Classic,
    Modern,
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
}

impl<'de> serde::Deserialize<'de> for TemplateId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(match s.as_str() {
            "classic" => TemplateId::Classic,
            "modern" => TemplateId::Modern,
            "swiss-minimal" => TemplateId::SwissMinimal,
            "academic" => TemplateId::Academic,
            "atelier" => TemplateId::Atelier,
            "meridian" => TemplateId::Meridian,
            "throughline" => TemplateId::Throughline,
            "portrait" => TemplateId::Portrait,
            "lebenslauf" => TemplateId::Lebenslauf,
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
#[allow(dead_code)]
pub struct ParsedLine {
    pub kind: LineKind,
    pub raw: String,
    pub text: String,
    pub segments: Vec<TextSegment>,
    pub right_text: Option<String>, // For job entries (date)
}

/// Parsed document structure
#[derive(Debug, Clone)]
#[allow(dead_code)]
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
            (TemplateId::Modern, "\"modern\""),
            (TemplateId::SwissMinimal, "\"swiss-minimal\""),
            (TemplateId::Academic, "\"academic\""),
            (TemplateId::Atelier, "\"atelier\""),
            (TemplateId::Meridian, "\"meridian\""),
            (TemplateId::Throughline, "\"throughline\""),
            (TemplateId::Portrait, "\"portrait\""),
            (TemplateId::Lebenslauf, "\"lebenslauf\""),
        ];
        for (id, expected_json) in cases {
            let serialized = serde_json::to_string(&id).expect("serialize");
            assert_eq!(serialized, expected_json, "{id:?} serialized wrong");
            let deserialized: TemplateId = serde_json::from_str(&serialized).expect("deserialize");
            assert_eq!(deserialized, id, "{id:?} did not round-trip");
        }
    }
}
