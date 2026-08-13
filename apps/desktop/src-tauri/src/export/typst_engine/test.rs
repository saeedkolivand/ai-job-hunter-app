//! Tests for the Typst engine — smoke tests + model-based + ATS harness.
//!
//! All tests run fully in-process (no disk, no network) via the offline
//! ResumeWorld hard-wall.

use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::{
    render_letter_pdf, render_letter_svg_pages, render_pdf, render_pdf_from_source,
    render_resume_svg_pages, RenderOpts, TypstTemplate,
};
use crate::locale::PageGeometry;
use crate::model::adapter::model_from_resume_text;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Count `/Type /Page` (individual page) objects in PDF bytes.
///
/// Uses a byte-level scan rather than lopdf's `get_pages()` because lopdf's
/// page-tree walker does not handle all page-tree structures that Typst emits
/// (it misses pages under certain indirect-reference trees and returns 1 even
/// for multi-page documents). The scan finds all occurrences of the `/Type`
/// `/Page` dictionary entry that marks an individual page object (not `/Type`
/// `/Pages` which marks a page-tree node).
///
/// Tolerates zero-or-more spaces between `/Type` and `/Page`: typst-pdf 0.15's
/// krilla/pdf-writer backend serialises dict entries as `/Type/Page` (no
/// space), where the pinned 0.14.2 backend wrote `/Type /Page` (one space).
/// Matching both keeps this scan from silently reporting zero pages again on
/// the next writer-formatting tweak.
fn count_pdf_pages(bytes: &[u8]) -> usize {
    let key = b"/Type";
    let val = b"/Page";
    let mut count = 0usize;
    let mut i = 0usize;
    while i + key.len() < bytes.len() {
        if bytes[i..i + key.len()] == *key {
            let mut j = i + key.len();
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if bytes[j..].starts_with(val) {
                // The character after `/Page` must not be `s` (which would make it `/Pages`).
                if bytes.get(j + val.len()) != Some(&b's') {
                    count += 1;
                }
            }
            i += key.len();
        } else {
            i += 1;
        }
    }
    count
}

/// Extract every `/Link` annotation target URI from a rendered PDF.
///
/// Typst writes `/Annots` as an array of **inline dictionaries**; lopdf's
/// `get_page_annotations` only resolves *indirect references* and so misses them
/// entirely — the documented regression that once made every header link read as
/// "missing". We therefore walk each object's `/Annots` array ourselves (mirroring
/// the validator's reader) and pull `/A /URI` off each `/Link`.
fn link_uris(bytes: &[u8]) -> Vec<String> {
    let doc = lopdf::Document::load_mem(bytes).expect("rendered PDF should parse with lopdf");

    fn uri_of(annot: &lopdf::Dictionary, doc: &lopdf::Document) -> Option<String> {
        let is_link = annot
            .get(b"Subtype")
            .and_then(|v| v.as_name())
            .map(|n| n == b"Link")
            .unwrap_or(false);
        if !is_link {
            return None;
        }
        annot
            .get(b"A")
            .ok()
            .and_then(|a| match a {
                lopdf::Object::Dictionary(d) => Some(d.clone()),
                lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok().cloned(),
                _ => None,
            })
            .and_then(|d| {
                d.get(b"URI")
                    .ok()
                    .and_then(|u| u.as_str().ok())
                    .map(|b| String::from_utf8_lossy(b).into_owned())
            })
    }

    let mut uris = Vec::new();
    for obj in doc.objects.values() {
        let Ok(dict) = obj.as_dict() else {
            continue;
        };
        let array = match dict.get(b"Annots") {
            Ok(lopdf::Object::Array(a)) => a.clone(),
            Ok(lopdf::Object::Reference(id)) => {
                match doc.get_object(*id).and_then(|o| o.as_array()) {
                    Ok(a) => a.clone(),
                    Err(_) => continue,
                }
            }
            _ => continue,
        };
        for entry in &array {
            let annot = match entry {
                lopdf::Object::Dictionary(d) => d.clone(),
                lopdf::Object::Reference(id) => match doc.get_dictionary(*id) {
                    Ok(d) => d.clone(),
                    Err(_) => continue,
                },
                _ => continue,
            };
            if let Some(uri) = uri_of(&annot, &doc) {
                uris.push(uri);
            }
        }
    }
    uris
}

// ── Smoke tests (raw source path) ─────────────────────────────────────────────

/// Minimal Typst document that exercises font loading and basic layout.
const SMOKE_SOURCE: &str = "= Hello\n\nSome body text rendered with the bundled font.";

#[test]
fn smoke_pdf_is_non_empty_and_starts_with_pdf_header() {
    let bytes = render_pdf_from_source(SMOKE_SOURCE)
        .expect("render_pdf_from_source should succeed for a trivial document");

    assert!(!bytes.is_empty(), "rendered PDF must be non-empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "rendered PDF must begin with %PDF, got: {:?}",
        &bytes[..4.min(bytes.len())]
    );
}

#[test]
fn smoke_pdf_text_extraction_contains_expected_words() {
    let bytes = render_pdf_from_source(SMOKE_SOURCE)
        .expect("render_pdf_from_source should succeed for a trivial document");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract should be able to read our output");

    let lower = extracted.to_lowercase();
    assert!(
        lower.contains("hello"),
        "extracted text should contain 'hello'; got: {extracted:?}"
    );
    assert!(
        lower.contains("body"),
        "extracted text should contain 'body'; got: {extracted:?}"
    );
}

// ── Fixture ───────────────────────────────────────────────────────────────────

/// Short one-page resume fixture — enough content to exercise all block types
/// (header, paragraph, entry with bullets, standalone bullets) while keeping
/// compilation fast.
const FIXTURE_RESUME: &str = "\
Jane Doe
jane@example.com | https://linkedin.com/in/janedoe | https://github.com/janedoe

SUMMARY
Experienced software engineer with a passion for building reliable systems.

EXPERIENCE
Senior Engineer | Acme Corp | 2021 – Present
- Designed distributed task scheduler reducing latency by 40 percent
- Led migration to Rust-based microservices across three product teams

Software Engineer | Beta Inc | 2018 – 2021
- Built real-time data pipeline processing one million events per day
- Mentored two junior engineers through onboarding

EDUCATION
B.Sc. Computer Science | State University | 2014 – 2018

SKILLS
Rust, Python, TypeScript, PostgreSQL, Kubernetes, AWS
";

/// Accented-Latin résumé fixture — grave-accented lowercase (à, ò, ì) PLUS
/// capital grave accents (È, À), the less-tested shape flagged by the
/// `no_extractable_text` incident audit (a macOS user could not download
/// either PDF; the leading hypothesis is a broken ToUnicode CMap on a subset
/// font — glyphs render fine on screen but `pdf_extract` gets nothing back).
/// The incident involved Italian text; German (ü) is already covered by the
/// DE letter fixtures and the Portrait/Saffron "Über Ödegaard" grapheme pins.
/// Same section-heading shape as [`FIXTURE_RESUME`] so section classification
/// is unaffected — only the header name and body prose carry accents.
const ACCENTED_RESUME_FIXTURE: &str = "\
Àlvaro Èsposito
alvaro.esposito@example.it | https://linkedin.com/in/alvaroesposito

SUMMARY
Ingegnere del software cresciuto vicino a Città di Torino, però orientato ai \
sistemi distribuiti costruiti così da scalare senza sforzo.

EXPERIENCE
Senior Engineer | Acme Corp | 2021 – Present
- Migrated the payments service to a microservices architecture, cutting latency by 40 percent
- Guidato il team attraverso la migrazione, mantenendo però sempre alta la qualità

EDUCATION
Laurea in Informatica | Università degli Studi di Torino | 2014 – 2018

SKILLS
Rust, Python, TypeScript, PostgreSQL, Kubernetes, AWS
";

// ── Model-based render tests ──────────────────────────────────────────────────

fn opts_a4() -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats: false,
    }
}

#[test]
fn classic_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed");

    assert!(!bytes.is_empty(), "PDF bytes must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "output must start with %PDF header"
    );
}

// ── SVG live-preview emit ───────────────────────────────────────────────────────
//
// The live preview renders the SAME model + SAME Typst world as the PDF export,
// emitting one SVG string per page instead of a PDF blob. These guard that the
// SVG sibling fns return ≥1 non-empty page whose string is a real SVG document.

#[test]
fn render_resume_svg_pages_returns_svg_page() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let pages = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_resume_svg_pages(classic) should succeed");

    assert!(
        !pages.is_empty(),
        "résumé preview must produce at least one page"
    );
    for (i, page) in pages.iter().enumerate() {
        assert!(
            page.contains("<svg"),
            "résumé preview page {i} must contain an <svg root element; got start: {:?}",
            &page[..page.len().min(80)]
        );
    }
}

#[test]
fn document_accent_overrides_letter_accent_color() {
    use super::letter::style_from_template as letter_style_from_template;

    // Cover letters inherit the résumé template's accent. A document accent
    // applied via `Template::with_accent_override` must surface as the letter's
    // `c_accent`; a malformed value must leave the template's palette intact.
    let base_accent = letter_style_from_template(&Template::get(TemplateId::Classic)).c_accent;

    let overridden = Template::get(TemplateId::Classic).with_accent_override(Some("#AA0000"));
    assert_eq!(
        letter_style_from_template(&overridden).c_accent,
        "#AA0000",
        "a valid document accent must recolor the letter accent"
    );

    let malformed = Template::get(TemplateId::Classic).with_accent_override(Some("nope"));
    assert_eq!(
        letter_style_from_template(&malformed).c_accent,
        base_accent,
        "a malformed accent must leave the letter palette unchanged"
    );
}

#[test]
fn render_letter_svg_pages_returns_svg_page() {
    let t = Template::get(TemplateId::SwissMinimal);
    let pages = render_letter_svg_pages(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_svg_pages(us) should succeed");

    assert!(
        !pages.is_empty(),
        "cover-letter preview must produce at least one page"
    );
    for (i, page) in pages.iter().enumerate() {
        assert!(
            page.contains("<svg"),
            "cover-letter preview page {i} must contain an <svg root element; got start: {:?}",
            &page[..page.len().min(80)]
        );
    }
}

// ── Link-annotation round-trip (header contact links) ───────────────────────────
//
// The header carries the candidate's email/LinkedIn/GitHub as clickable links.
// They must survive into the PDF as real `/Link` annotations with extractable
// `/A /URI` targets — the exact path that regressed before (lopdf inline-annot
// parsing). Render through the live engine, then read the links back. Replaces the
// `resume_embeds_contact_link_annotations` + `every_template_renders_a_valid_pdf`
// coverage that lived in the deleted printpdf `layout_pdf` suite.

#[test]
fn classic_resume_embeds_contact_link_annotations() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed");
    let uris = link_uris(&bytes);
    assert!(
        uris.iter().any(|u| u.contains("linkedin.com/in/janedoe")),
        "LinkedIn link annotation missing from classic resume; found {uris:?}"
    );
    assert!(
        uris.iter().any(|u| u.contains("github.com/janedoe")),
        "GitHub link annotation missing from classic resume; found {uris:?}"
    );
}

#[test]
fn two_column_resume_embeds_contact_link_annotations() {
    // The full-width header in two-column templates is the higher-risk path for
    // dropped annotations, so assert links survive there too (Atelier).
    let model = model_from_resume_text(FIXTURE_RESUME);
    let template = Template::get(TemplateId::Atelier);
    let bytes = render_pdf(
        &model,
        TypstTemplate::from_template(&template),
        &opts_a4(),
        Some(&template),
    )
    .expect("render_pdf(atelier) should succeed");
    let uris = link_uris(&bytes);
    assert!(
        uris.iter().any(|u| u.contains("linkedin.com/in/janedoe")),
        "LinkedIn link annotation missing from two-column resume; found {uris:?}"
    );
    assert!(
        uris.iter().any(|u| u.contains("github.com/janedoe")),
        "GitHub link annotation missing from two-column resume; found {uris:?}"
    );
}

/// Canonical user-facing template set — must match the `TemplateId` enum
/// (pinned by the serde round-trip test in types.rs and the TS sync guard).
/// Shared by every test that iterates "all templates" so a newly added
/// template is covered automatically rather than needing a remembered edit.
///
/// Now a thin alias over `templates::CANONICAL_TEMPLATE_IDS`: the validator
/// matrices in `validate/tests.rs` iterate the same list, so a new template
/// cannot be covered here and silently skipped there.
fn canonical_template_ids() -> [TemplateId; 16] {
    crate::export::templates::CANONICAL_TEMPLATE_IDS
}

/// The part of an extracted cover letter AFTER the sign-off, i.e. the signature
/// block. Both letter fixtures print the candidate's name twice (letterhead and
/// signature), so a whole-document `contains` cannot tell "the signature
/// extracted" from "only the letterhead extracted". Returns `""` when the
/// sign-off itself is missing, which fails the caller's assertion — correct,
/// since a letter whose "Sincerely" did not extract is already broken.
fn signature_block(lowercased: &str) -> &str {
    lowercased
        .split_once("sincerely")
        .map(|(_, tail)| tail)
        .unwrap_or("")
}

/// Mirrors `validate::mod::normalize` (validate/mod.rs:874-882): lowercased,
/// whitespace-collapsed, alphanumeric-only text used for tolerant `contains`
/// checks. Duplicated here (rather than exposed as `pub(crate)`) because this
/// test file owns no production code — see [`NO_EXTRACTABLE_TEXT_THRESHOLD`].
fn normalize_like_validator(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The exact threshold `validate::mod::evaluate` uses (validate/mod.rs:796) to
/// raise the CRITICAL `no_extractable_text` issue that blocks an export: fewer
/// than this many [`normalize_like_validator`]-normalized characters means the
/// document has (almost) no extractable text. Asserting against this constant
/// — not just "some text extracted" — means a passing test proves the real
/// validator would not have blocked the export.
const NO_EXTRACTABLE_TEXT_THRESHOLD: usize = 20;

// ── Render-measurement helpers (glyph geometry from typst-svg) ────────────────
//
// A registry field can claim "centred name" while the layout silently ignores
// it, so the tests below assert against the RENDERED page, not `data.style`.

/// Value of attribute `name` in an SVG start tag (`name="…"`), if present.
/// Matches on `" name=\""` — the leading space is what stops `x` from matching
/// `xlink:href` and `fill` from matching `fill-rule`.
fn svg_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// Parse the translation component of a typst-svg `transform` attribute.
/// typst-svg only ever emits pure translations (`translate(x)` / `translate(x
/// y)`) and the baseline y-flip (`matrix(1 0 0 -1 e f)`), and never nests two
/// flips — so a glyph's page position is the running sum of these pairs plus its
/// own `x`.
///
/// **Panics on any other transform**, rather than the `(0.0, 0.0)` this used to
/// return. A silent zero here is the worst possible failure mode for a
/// measurement helper: every geometry assertion built on it (band containment,
/// centring, section offsets) keeps passing while measuring the wrong place. A
/// `scale(…)`/`rotate(…)`/general-`matrix(…)` group would also invalidate the
/// running-sum model itself, not just shift the origin — so the only correct
/// response is to stop and say the helper needs extending.
fn svg_translation(transform: &str) -> (f64, f64) {
    let inner = transform
        .split_once('(')
        .and_then(|(_, rest)| rest.strip_suffix(')'))
        .unwrap_or_else(|| panic!("svg_translation: malformed transform {transform:?}"));
    let nums: Vec<f64> = inner
        .split([' ', ','])
        .filter(|t| !t.is_empty())
        // Strict: an unparseable component used to be dropped by `filter_map`,
        // which silently turned `translate(3 bogus)` into the 1-arg form.
        .map(|t| {
            t.parse::<f64>()
                .unwrap_or_else(|_| panic!("svg_translation: bad number {t:?} in {transform:?}"))
        })
        .collect();

    if let Some(kind) = transform.split('(').next() {
        match (kind, nums.len()) {
            ("translate", 2) => return (nums[0], nums[1]),
            // SVG's one-argument form: `translate(x)` means ty = 0. typst-svg
            // emits it whenever a group only shifts horizontally — dropping it
            // silently under-reports x by the whole shift.
            ("translate", 1) => return (nums[0], 0.0),
            ("matrix", 6) => {
                // Only the identity and the pure y-flip keep "page position =
                // running sum of translations" true. Anything else (a scale, a
                // rotation, a skew) also transforms the CHILD coordinates, which
                // this walker does not model at all.
                let linear = (nums[0], nums[1], nums[2], nums[3]);
                assert!(
                    linear == (1.0, 0.0, 0.0, 1.0) || linear == (1.0, 0.0, 0.0, -1.0),
                    "svg_translation: {transform:?} has a non-identity, non-y-flip \
                     linear part {linear:?} — it scales/rotates its children, so the \
                     running-sum model is invalid. Extend this helper (and every \
                     caller's assumptions) rather than measuring the wrong geometry."
                );
                return (nums[4], nums[5]);
            }
            _ => {}
        }
    }
    panic!(
        "svg_translation: unsupported transform {transform:?} — this helper models \
         only `translate(x)`, `translate(x y)` and `matrix(1 0 0 ±1 e f)`. Returning \
         zero here would silently corrupt every geometry assertion built on it."
    );
}

/// Walk a typst-svg document's start tags, tracking the cumulative `<g
/// transform>` translation, and hand every non-group tag to `visit` together
/// with the offset in force at that point.
///
/// Shared by [`glyph_positions`] and [`first_filled_rect_bottom`] so the
/// group-stack rules (self-closing groups, balance checks) exist once and cannot
/// drift between the two.
fn walk_svg_tags(svg: &str, mut visit: impl FnMut(&str, (f64, f64)) -> bool) {
    let mut stack: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    let mut rest = svg;
    while let Some(lt) = rest.find('<') {
        let after = &rest[lt + 1..];
        let Some(gt) = after.find('>') else { break };
        let tag = &after[..gt];
        rest = &after[gt + 1..];

        if tag == "/g" {
            assert!(
                stack.len() > 1,
                "walk_svg_tags: `</g>` with no matching `<g>` — the transform \
                 stack underflowed, so every later offset would be wrong"
            );
            stack.pop();
        } else if tag == "g" || tag.starts_with("g ") {
            // A SELF-CLOSING group (`<g … />`, which `xmlwriter` emits for a
            // group it never wrote children into) has no `</g>` to pop it.
            // Pushing it would leave the stack permanently deep and shift every
            // following sibling by its transform.
            if tag.ends_with('/') {
                continue;
            }
            let (dx, dy) = svg_attr(tag, "transform")
                .map(svg_translation)
                .unwrap_or((0.0, 0.0));
            let top = *stack.last().expect("transform stack is never empty");
            stack.push((top.0 + dx, top.1 + dy));
        } else if visit(tag, *stack.last().expect("transform stack is never empty")) {
            return;
        }
    }
    assert_eq!(
        stack.len(),
        1,
        "walk_svg_tags: {} unclosed `<g>` element(s) — the document is truncated \
         or the tag scanner lost sync",
        stack.len() - 1
    );
}

/// Every glyph in a typst-svg page as `(page_x, baseline_y, fill)`, in Typst
/// points with the page's top-left as the origin. Glyphs are `<use>` elements;
/// decorative `<path>` shapes (page background, rules, bars) are ignored.
fn glyph_positions(svg: &str) -> Vec<(f64, f64, String)> {
    let mut out = Vec::new();
    walk_svg_tags(svg, |tag, (ox, oy)| {
        if tag.starts_with("use ") {
            let gx: f64 = svg_attr(tag, "x")
                .map(|v| {
                    v.parse()
                        .unwrap_or_else(|_| panic!("glyph_positions: bad `use` x={v:?}"))
                })
                .unwrap_or(0.0);
            // A `<use y=…>` would need the enclosing y-flip applied to it; the
            // walker only sums translations, so a non-zero one it silently
            // ignored would misplace the glyph vertically.
            let gy: f64 = svg_attr(tag, "y")
                .map(|v| {
                    v.parse()
                        .unwrap_or_else(|_| panic!("glyph_positions: bad `use` y={v:?}"))
                })
                .unwrap_or(0.0);
            assert_eq!(
                gy, 0.0,
                "glyph_positions: `<use y=\"{gy}\">` — glyph-local vertical offsets \
                 are not modelled (the enclosing y-flip would have to be applied); \
                 extend the helper instead of dropping it"
            );
            let fill = svg_attr(tag, "fill").unwrap_or("").to_string();
            out.push((ox + gx, oy, fill));
        }
        false
    });
    out
}

/// Width, in points, of the topmost baseline among the glyphs left of `max_x`
/// — first glyph origin to last glyph origin.
///
/// Reads the RENDERED TYPE SIZE, which glyph positions alone cannot: typst-svg
/// gives every glyph an explicit `x` advance, so the same string set in the same
/// font scales this figure linearly with its point size. Comparing the same
/// string in two renders therefore yields their size ratio directly, with no
/// hardcoded point values and no dependency on which glyphs typst-svg chose to
/// group.
///
/// `max_x` selects a column: pass the body's left edge to measure the sidebar
/// rail, or `f64::INFINITY` for the whole page.
fn top_line_extent(glyphs: &[(f64, f64, String)], max_x: f64) -> f64 {
    let zone: Vec<&(f64, f64, String)> = glyphs.iter().filter(|(x, _, _)| *x < max_x).collect();
    if zone.is_empty() {
        return 0.0;
    }
    let top = zone
        .iter()
        .map(|(_, y, _)| *y)
        .fold(f64::INFINITY, f64::min);
    // 0.5pt tolerance: one baseline, not "roughly the top of the page".
    let xs: Vec<f64> = zone
        .iter()
        .filter(|(_, y, _)| (*y - top).abs() < 0.5)
        .map(|(x, _, _)| *x)
        .collect();
    let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    hi - lo
}

/// Bottom edge (page y, in points) of the first FULL-WIDTH rectangle painted in
/// `fill` — for a header-band template that is the band itself, drawn first as
/// the page background. Narrow accent shapes (section-marker bars) share the
/// fill, so anything under 100pt wide is skipped.
///
/// Reading the band out of the render instead of restating its constant is what
/// makes the containment assertions real: a test that compares glyphs against a
/// hardcoded band height silently keeps passing when the band shrinks.
fn first_filled_rect_bottom(svg: &str, fill: &str) -> Option<f64> {
    let mut found = None;
    walk_svg_tags(svg, |tag, (_, oy)| {
        if !tag.starts_with("path ") {
            return false;
        }
        if !svg_attr(tag, "fill").is_some_and(|f| f.eq_ignore_ascii_case(fill)) {
            return false;
        }
        // `d="M x yv Hh WvΩ-HZ"` — the axis-aligned rect typst-svg emits for
        // a `rect(...)`: origin, height (v), width (h).
        let Some(d) = svg_attr(tag, "d") else {
            return false;
        };
        let nums: Vec<f64> = d
            .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .filter_map(|t| t.parse::<f64>().ok())
            .collect();
        if nums.len() >= 4 && nums[3].abs() >= 100.0 {
            found = Some(oy + nums[1] + nums[2]);
            return true; // stop the walk
        }
        false
    });
    found
}

/// Collapse [`glyph_positions`] to one entry per text line: `(baseline_y,
/// leftmost_x, rightmost_x)`, ordered top-to-bottom. Glyphs sharing a baseline
/// are one line.
fn text_lines(svg: &str) -> Vec<(f64, f64, f64)> {
    let mut lines: Vec<(f64, f64, f64)> = Vec::new();
    for (x, y, _) in glyph_positions(svg) {
        match lines.iter_mut().find(|l| (l.0 - y).abs() < 0.01) {
            Some(line) => {
                line.1 = line.1.min(x);
                line.2 = line.2.max(x);
            }
            None => lines.push((y, x, x)),
        }
    }
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    lines
}

// ── Tests for the measurement helpers themselves ──────────────────────────────
//
// Every geometry assertion in this file is only as trustworthy as these three
// functions. Their old failure mode was SILENT: an unparsed transform became
// `(0.0, 0.0)` and a self-closing `<g/>` unbalanced the stack, both of which
// move measurements without failing anything. These pin the hardened behavior.

#[test]
fn svg_translation_parses_the_forms_typst_actually_emits() {
    assert_eq!(svg_translation("translate(3 4)"), (3.0, 4.0));
    assert_eq!(svg_translation("translate(3,4)"), (3.0, 4.0));
    // One-argument form: ty is 0, not "unparsed".
    assert_eq!(svg_translation("translate(5)"), (5.0, 0.0));
    // The baseline y-flip.
    assert_eq!(svg_translation("matrix(1 0 0 -1 7 8)"), (7.0, 8.0));
    assert_eq!(svg_translation("matrix(1 0 0 1 7 8)"), (7.0, 8.0));
    assert_eq!(svg_translation("translate(-2.5 -0.75)"), (-2.5, -0.75));
}

#[test]
#[should_panic(expected = "unsupported transform")]
fn svg_translation_rejects_scale_instead_of_reading_it_as_zero() {
    // Used to return (0.0, 0.0): a scaled group's contents would be reported at
    // their parent's origin, and every containment assertion would still pass.
    svg_translation("scale(2)");
}

#[test]
#[should_panic(expected = "unsupported transform")]
fn svg_translation_rejects_rotate_instead_of_reading_it_as_zero() {
    svg_translation("rotate(90)");
}

#[test]
#[should_panic(expected = "non-identity, non-y-flip linear part")]
fn svg_translation_rejects_a_scaling_matrix() {
    // Shape-wise a valid 6-number matrix, so the old `starts_with("matrix(")`
    // arm accepted it and returned (e, f) — silently dropping a 2x scale of
    // every child coordinate.
    svg_translation("matrix(2 0 0 2 10 20)");
}

#[test]
fn glyph_positions_ignores_self_closing_groups() {
    // `xmlwriter` writes `<g …/>` for a group it never put children into. It has
    // no `</g>`, so pushing it left the stack permanently deep and shifted every
    // later sibling — here, the second glyph would read x=1030 instead of 30.
    let svg = concat!(
        r##"<svg xmlns="http://www.w3.org/2000/svg">"##,
        r##"<g transform="translate(10 20)"><use x="5" fill="#111111"/></g>"##,
        r##"<g transform="translate(1000 1000)"/>"##,
        r##"<g transform="translate(30 40)"><use x="0" fill="#222222"/></g>"##,
        r##"</svg>"##,
    );
    let glyphs = glyph_positions(svg);
    assert_eq!(
        glyphs,
        vec![
            (15.0, 20.0, "#111111".to_string()),
            (30.0, 40.0, "#222222".to_string()),
        ],
        "a self-closing <g/> must contribute no offset to its siblings"
    );
}

#[test]
fn glyph_positions_accumulates_nested_group_translations() {
    // The property every measurement here relies on, pinned directly.
    let svg = concat!(
        r##"<svg xmlns="http://www.w3.org/2000/svg">"##,
        r##"<g transform="translate(10 20)">"##,
        r##"<g transform="matrix(1 0 0 -1 1 2)"><use x="3" fill="#abcdef"/></g>"##,
        r##"</g>"##,
        r##"<use x="7" fill="#000000"/>"##,
        r##"</svg>"##,
    );
    assert_eq!(
        glyph_positions(svg),
        vec![
            (14.0, 22.0, "#abcdef".to_string()),
            // After `</g></g>` the stack is back at the root, not still nested.
            (7.0, 0.0, "#000000".to_string()),
        ]
    );
}

#[test]
#[should_panic(expected = "underflowed")]
fn walk_svg_tags_rejects_an_unbalanced_closing_group() {
    glyph_positions(r#"<svg><g transform="translate(1 1)"></g></g></svg>"#);
}

#[test]
#[should_panic(expected = "unclosed")]
fn walk_svg_tags_rejects_a_truncated_document() {
    glyph_positions(r##"<svg><g transform="translate(1 1)"><use x="0" fill="#fff"/>"##);
}

/// Render page 1 of `template` for `model` to SVG.
fn svg_page1(
    model: &crate::model::document::DocumentModel,
    template: &Template,
    ats: bool,
) -> String {
    let mut opts = opts_a4();
    opts.ats = ats;
    render_resume_svg_pages(
        model,
        TypstTemplate::from_template(template),
        &opts,
        Some(template),
    )
    .unwrap_or_else(|e| {
        panic!(
            "render_resume_svg_pages({:?}) should succeed: {e:?}",
            template.id
        )
    })
    .into_iter()
    .next()
    .unwrap_or_else(|| panic!("{:?}: at least one SVG page", template.id))
}

#[test]
fn every_template_renders_a_valid_pdf() {
    let ids = canonical_template_ids();
    assert_eq!(ids.len(), 16, "expected the sixteen canonical templates");

    let model = model_from_resume_text(FIXTURE_RESUME);
    for id in ids {
        let template = Template::get(id);
        let bytes = render_pdf(
            &model,
            TypstTemplate::from_template(&template),
            &opts_a4(),
            Some(&template),
        )
        .unwrap_or_else(|e| panic!("render_pdf({id:?}) should succeed: {e:?}"));
        assert!(!bytes.is_empty(), "{id:?}: PDF bytes must not be empty");
        assert!(
            bytes.starts_with(b"%PDF"),
            "{id:?}: output must start with %PDF"
        );
        assert!(
            count_pdf_pages(&bytes) >= 1,
            "{id:?}: must emit at least one page"
        );
    }
}

// Every canonical template must round-trip accented-Latin content — grave
// lowercase + capital È/À, the shape the `no_extractable_text` incident audit
// flagged as under-tested — through `pdf_extract`, not just emit a %PDF
// header. `every_template_renders_a_valid_pdf` above never calls
// `pdf_extract` at all, so a broken/missing ToUnicode CMap on a subset font
// (renders fine on screen, extracts to nothing) would pass it silently; this
// is the regression test for exactly that class of bug.
#[test]
fn every_template_extracts_accented_latin_content() {
    let model = model_from_resume_text(ACCENTED_RESUME_FIXTURE);
    for id in canonical_template_ids() {
        let template = Template::get(id);
        let bytes = render_pdf(
            &model,
            TypstTemplate::from_template(&template),
            &opts_a4(),
            Some(&template),
        )
        .unwrap_or_else(|e| panic!("render_pdf({id:?}) should succeed: {e:?}"));

        let extracted = pdf_extract::extract_text_from_mem(&bytes).unwrap_or_else(|e| {
            panic!("{id:?}: pdf-extract must succeed on rendered output: {e:?}")
        });
        let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
        let lower = normalised.to_lowercase();

        assert!(
            lower.contains("àlvaro") && lower.contains("èsposito"),
            "{id:?}: accented name missing — capitals È/À did not survive extraction\n---\n{extracted:?}"
        );
        assert!(
            lower.contains("così") || lower.contains("però") || lower.contains("città"),
            "{id:?}: grave-accented-lowercase body word missing\n---\n{extracted:?}"
        );

        // Same gate `validate::mod::evaluate` uses to raise the CRITICAL
        // `no_extractable_text` issue — a passing assertion here means the
        // real validator would NOT have blocked this export.
        let normalized_len = normalize_like_validator(&extracted).len();
        assert!(
            normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
            "{id:?}: only {normalized_len} normalized chars extracted — the real \
             validator's no_extractable_text gate (< {NO_EXTRACTABLE_TEXT_THRESHOLD}) \
             would block this export; got {extracted:?}"
        );
    }
}

/// A4 width in Typst points — the page every `opts_a4()` render uses.
const A4_WIDTH_PT: f64 = 595.275_590_551;
/// `single_column.typ`'s locked page margin (25.4 mm) in points. Left-aligned
/// header lines start exactly here.
const SINGLE_COLUMN_MARGIN_PT: f64 = 72.0;

/// `Template::name_centered` must be a fact about the RENDERED page, not just a
/// registry field. `single_column.typ` centres with `align(center, …)`, which
/// does nothing inside an `auto`-width block (the block shrinks to its content,
/// so there is no slack to centre in): Jake shipped `name_centered: true` while
/// its name rendered at x=72.0 — the left margin, byte-identical in position to
/// the `name_centered: false` templates. `jake_matches_spec` passed throughout,
/// because a field pin cannot see the layout.
///
/// Midpoints are computed from glyph ORIGINS, so they sit half of the last
/// glyph's advance left of the true visual centre; the tolerance covers that.
/// The bug's signature was ~186pt off centre, so it has an enormous margin.
#[test]
fn name_centered_actually_centres_the_rendered_header() {
    let centre = A4_WIDTH_PT / 2.0;
    let mut model = model_from_resume_text(FIXTURE_RESUME);
    model.header.title = Some("Senior Software Engineer".to_string());

    let jake = Template::get(TemplateId::Jake);
    assert!(
        jake.name_centered,
        "fixture guard: Jake is this test's centred single-column case"
    );
    let lines = text_lines(&svg_page1(&model, &jake, false));
    assert!(
        lines.len() >= 3,
        "expected at least name/title/contact lines, got {lines:?}"
    );
    for (label, (y, lo, hi)) in ["name", "title", "contact"]
        .into_iter()
        .zip(lines.iter().copied())
    {
        let mid = (lo + hi) / 2.0;
        assert!(
            (mid - centre).abs() < 15.0,
            "jake's {label} line is not centred: y={y:.2} x=[{lo:.2}..{hi:.2}] \
             midpoint {mid:.2} vs page centre {centre:.2}"
        );
    }

    // Control: the same three lines on a `name_centered: false` template must
    // still start exactly on the left margin. This is what makes the assertion
    // above about CENTRING rather than about "the header moved".
    for id in [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
        TemplateId::Cadence,
        TemplateId::Regent,
    ] {
        let t = Template::get(id);
        assert!(
            !t.name_centered,
            "{id:?}: this control list is the left-aligned single-column set"
        );
        for (y, lo, hi) in text_lines(&svg_page1(&model, &t, false))
            .into_iter()
            .take(3)
        {
            assert!(
                (lo - SINGLE_COLUMN_MARGIN_PT).abs() < 0.01,
                "{id:?}: header line y={y:.2} x=[{lo:.2}..{hi:.2}] must stay flush \
                 to the {SINGLE_COLUMN_MARGIN_PT}pt left margin — the centring fix \
                 must not leak to left-aligned templates"
            );
        }
    }
}

// ── Phase 8 Track B: Awesome / Deedy bespoke-behavior pins ─────────────────────

/// Awesome's design-tier ATS toggle must be more than cosmetic: `ats=true`
/// drops the accent-tinted header band, its keyline, AND the accent-bar
/// section markers, leaving only accent-colored hyperlinks (unaffected by
/// `is-ats`, matching every other non-Classic ATS-safe template). typst-svg
/// renders decorative shapes (the band rect, the keyline, each accent bar) as
/// `<path fill="#…"` / `stroke="#…"` elements, distinct from glyph refs
/// (`<use … fill="#…"`) — the same fill-color detection technique
/// `letter_banded_band_draws_on_page_one_only` uses for the cover-letter band,
/// narrowed to SHAPES so accent-colored link text (unaffected by `is-ats`,
/// same as every other template) can't mask the assertion.
#[test]
fn awesome_ats_mode_drops_the_header_band_and_section_markers() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let template = Template::get(TemplateId::Awesome);
    let accent_hex = format!(
        "#{:02x}{:02x}{:02x}",
        template.accent_color.0, template.accent_color.1, template.accent_color.2
    );
    let fill_needle = format!(r#"<path fill="{accent_hex}""#);
    let stroke_needle = format!(r#"stroke="{accent_hex}""#);
    let count_shapes =
        |svg: &str| svg.matches(&fill_needle).count() + svg.matches(&stroke_needle).count();

    let banded_opts = opts_a4();
    let banded = render_resume_svg_pages(
        &model,
        TypstTemplate::from_template(&template),
        &banded_opts,
        Some(&template),
    )
    .expect("render_resume_svg_pages(awesome, ats=false) should succeed");
    let banded_page1 = banded
        .first()
        .expect("awesome ats=false: at least one page");

    let mut ats_opts = opts_a4();
    ats_opts.ats = true;
    let plain = render_resume_svg_pages(
        &model,
        TypstTemplate::from_template(&template),
        &ats_opts,
        Some(&template),
    )
    .expect("render_resume_svg_pages(awesome, ats=true) should succeed");
    let plain_page1 = plain.first().expect("awesome ats=true: at least one page");

    let banded_shapes = count_shapes(banded_page1);
    let plain_shapes = count_shapes(plain_page1);
    assert!(
        banded_shapes >= 2,
        "awesome (ats=false) must draw at least the band rect + keyline as \
         accent-fill/stroke shapes; got {banded_shapes}"
    );
    // `plain_shapes` isn't zero because the ATS branch draws its OWN decoration:
    // a thin accent-colored `line` under each section heading (`awesome.typ`'s
    // `is-ats` arm; `rule_color == accent_color` in the registry). The non-ATS
    // branch draws no such rule — it draws the band rect, the keyline and one
    // accent bar per section instead — so the two counts are not a subset
    // relation, just strictly ordered. A non-strictly-lower count would mean
    // `is-ats` failed to drop the band/keyline/bars.
    assert!(
        plain_shapes < banded_shapes,
        "awesome (ats=true) must draw fewer decorative accent-fill/stroke shapes \
         than ats=false (band + keyline + section-marker bars must be dropped) — \
         banded={banded_shapes} plain={plain_shapes}"
    );
}

/// Awesome's header is placed inside `page.background`, which lays out at
/// UNBOUNDED width: before `awesome.typ` bounded it to `band-box-w`, a 125-char
/// contact line did not wrap — it ran to x=630pt on a 595pt-wide sheet and the
/// tail was simply not on the page. Bounding it makes it wrap, which only helps
/// while the band is tall enough to hold the wrapped line; otherwise white band
/// ink lands on white paper below the band. Both halves are pinned here against
/// the render, for the two band heights `awesome.typ` budgets (title present or
/// not), because white ink exists ONLY inside the band.
#[test]
fn awesome_band_contains_its_white_header_text() {
    const MM: f64 = 72.0 / 25.4;
    // `awesome.typ`'s `body-margin-h`; the band content shares the body margins.
    let margin = 20.0 * MM;
    let template = Template::get(TemplateId::Awesome);

    for (label, title, band_mm, contact) in [
        (
            "no title, short contact",
            None,
            24.0,
            "jane@example.com | https://linkedin.com/in/janedoe | https://github.com/janedoe",
        ),
        (
            "title + 125-char contact",
            Some("Principal Distributed Systems Engineer"),
            28.0,
            "alexandra.konstantinopoulos@example.com | +1 (415) 555-0189 | San Francisco, CA \
             | https://linkedin.com/in/alexandrakonst | https://alexandrakonstantinopoulos.dev",
        ),
    ] {
        let mut model = model_from_resume_text(FIXTURE_RESUME);
        model.header.name = "Alexandra Konstantinopoulos".to_string();
        model.header.title = title.map(str::to_string);
        model.header.contact = crate::model::rich::tokenize_rich(contact);

        let svg = svg_page1(&model, &template, false);
        let white: Vec<(f64, f64)> = glyph_positions(&svg)
            .into_iter()
            .filter(|(_, _, fill)| fill.eq_ignore_ascii_case("#ffffff"))
            .map(|(x, y, _)| (x, y))
            .collect();
        assert!(
            white.len() > 20,
            "[{label}] expected the band's white name/contact glyphs, found {}",
            white.len()
        );

        // The band the reader actually sees, measured off the rendered rect —
        // not `band_mm` restated, or shrinking the band would keep this green.
        // `band_mm` only pins that the band stayed THIN (its design brief).
        let accent = format!(
            "#{:02x}{:02x}{:02x}",
            template.accent_color.0, template.accent_color.1, template.accent_color.2
        );
        let band_bottom = first_filled_rect_bottom(&svg, &accent)
            .unwrap_or_else(|| panic!("[{label}] no full-width accent band rect in the render"));

        // Vertical: every white baseline, plus room for its descenders, inside
        // the band. Without the taller band the wrapped contact line lands at
        // y=71.70 against a 68.03pt band bottom — invisible white-on-white.
        let lowest = white.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);
        assert!(
            lowest + 3.0 <= band_bottom,
            "[{label}] white header text reaches baseline y={lowest:.2} but the band \
             ends at {band_bottom:.2}pt — the overflow renders white-on-white"
        );
        // The band is content-measured (`awesome.typ`'s `#context`), so this is
        // the other half of that rule: for a common 1–2-line header the thin
        // `band-min-h` must still DOMINATE the measurement — the band grows only
        // for genuine overflow (`awesome_band_grows_to_contain_any_contact_line_count`),
        // never creeping wider on ordinary input. Raising `band-pad-bottom` far
        // enough to inflate these two cases fails here.
        assert!(
            (band_bottom - band_mm * MM).abs() < 0.5,
            "[{label}] band is {band_bottom:.2}pt tall, expected the thin {:.2}pt \
             minimum — an ordinary header must not grow the band",
            band_mm * MM
        );

        // No band glyph may be painted in the ACCENT — the band's own fill. The
        // contact line is mostly link runs (LinkedIn / GitHub / Website), and
        // links are the one run kind that carries its own colour: `render-runs`
        // draws them in the accent for the body, `render-runs-white` in the
        // band's white. Both are now one parametrised ladder
        // (`render-runs-in`), and pointing the band at the body's fill paints
        // #c41e3a links onto a #c41e3a band — invisible contact details, passing
        // every other assertion here (the surrounding non-link text stays white,
        // so the white-glyph count and the containment bounds are unmoved).
        let accent_in_band = glyph_positions(&svg)
            .into_iter()
            .filter(|(_, y, fill)| *y <= band_bottom && fill.eq_ignore_ascii_case(&accent))
            .count();
        assert_eq!(
            accent_in_band, 0,
            "[{label}] {accent_in_band} header glyph(s) are painted in the accent \
             {accent} inside the band, which is filled with that same accent — \
             band text (links included) must use the band's white"
        );

        // Horizontal: inside the printable width. Without `box(width: band-box-w)`
        // this reads 630.03 on a 595.28pt page.
        let rightmost = white.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
        assert!(
            rightmost <= A4_WIDTH_PT - margin,
            "[{label}] white header text reaches x={rightmost:.2}, past the right \
             margin at {:.2}pt — the placed header is laying out unbounded and \
             running off the sheet",
            A4_WIDTH_PT - margin
        );
    }
}

/// The companion to [`awesome_band_contains_its_white_header_text`]: that test
/// pins the THIN band for the common 1–2-line header, this one pins that the
/// band still contains a header that outgrows it.
///
/// `awesome.typ` used to budget "name + optional title + up to TWO contact
/// lines" as a fixed 24mm/28mm. Nothing caps a header at two lines:
/// `ContactProfile.extra_links` is an unbounded `Vec` and `apply_to_header`
/// copies the whole rendered line into `header.contact`. Built through that real
/// adapter path, a twelve-extra-link profile wraps to THREE lines and put a whole
/// white baseline at y=86.03 against a band ending at 79.37pt — invisible
/// white-on-white ink, and contact details silently gone from the page.
///
/// The fix measures the band from its own content, so this holds for ANY line
/// count, not just three — which is why the assertions below are all relative to
/// the measured band and the measured line count, with nothing restating 24/28mm.
#[test]
fn awesome_band_grows_to_contain_any_contact_line_count() {
    let template = Template::get(TemplateId::Awesome);
    let mut model = model_from_resume_text(FIXTURE_RESUME);
    model.header.name = "Alexandra Konstantinopoulos".to_string();
    model.header.title = Some("Principal Distributed Systems Engineer".to_string());
    // The adapter path: a blank contact line filled from the profile.
    model.header.contact = Vec::new();

    let profile = crate::contact_profile::ContactProfile {
        full_name: Some("Alexandra Konstantinopoulos".to_string()),
        email: Some("alexandra.konstantinopoulos@example.com".to_string()),
        phone: Some("+1 (415) 555-0189".to_string()),
        location: Some(crate::contact_profile::LocalizedText {
            default: "San Francisco, California".to_string(),
            by_lang: Default::default(),
        }),
        linkedin: Some("https://linkedin.com/in/alexandrakonst".to_string()),
        github: Some("https://github.com/alexandrakonst".to_string()),
        website: Some("https://alexandrakonstantinopoulos.dev".to_string()),
        extra_links: [
            "Portfolio",
            "Stack Overflow",
            "Google Scholar",
            "Speaker Deck",
            "Dribbble",
            "Behance",
            "Medium",
            "Dev.to",
            "Mastodon",
            "Bluesky",
            "ORCID",
            "Personal Blog",
        ]
        .into_iter()
        .enumerate()
        .map(|(i, label)| crate::contact_profile::ContactLink {
            label: label.to_string(),
            url: format!("https://example.com/alexandra/{i}"),
        })
        .collect(),
        photo: None,
    };
    profile.apply_to_header(&mut model.header, "en");
    assert!(
        !model.header.contact.is_empty(),
        "fixture guard: the profile must have filled the blank contact line"
    );

    let svg = svg_page1(&model, &template, false);
    let white: Vec<(f64, f64)> = glyph_positions(&svg)
        .into_iter()
        .filter(|(_, _, fill)| fill.eq_ignore_ascii_case("#ffffff"))
        .map(|(x, y, _)| (x, y))
        .collect();
    let accent = format!(
        "#{:02x}{:02x}{:02x}",
        template.accent_color.0, template.accent_color.1, template.accent_color.2
    );
    let band_bottom = first_filled_rect_bottom(&svg, &accent)
        .expect("no full-width accent band rect in the render");

    // Fixture guard. The white baselines are name + title + one per wrapped
    // contact line, so fewer than five means the contact did NOT wrap to three
    // lines and this test silently degraded into a copy of the 2-line one —
    // testing the overflow path not at all.
    let mut baselines: Vec<f64> = white.iter().map(|(_, y)| *y).collect();
    baselines.sort_by(f64::total_cmp);
    baselines.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    assert!(
        baselines.len() >= 5,
        "fixture guard: expected name + title + 3+ wrapped contact lines in the \
         band, got {} white baselines ({baselines:?}) — this fixture must exceed \
         the old two-line budget or it tests nothing",
        baselines.len()
    );

    // The band grew past the thin minimum it would otherwise have been pinned at
    // (28mm with a title). Without this, a band that grew to exactly the minimum
    // — i.e. the fix not firing — could still satisfy the containment check on a
    // luckier fixture.
    const MM: f64 = 72.0 / 25.4;
    assert!(
        band_bottom > 28.0 * MM,
        "the band is still {band_bottom:.2}pt — it must grow beyond the 28mm thin \
         minimum to hold a three-line contact"
    );

    let lowest = baselines.last().copied().unwrap_or(f64::MIN);
    assert!(
        lowest + 3.0 <= band_bottom,
        "white header text reaches baseline y={lowest:.2} but the band ends at \
         {band_bottom:.2}pt — the overflow renders white-on-white"
    );

    // Horizontal containment must survive the taller band too.
    let rightmost = white.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
    let margin = 20.0 * MM;
    assert!(
        rightmost <= A4_WIDTH_PT - margin,
        "white header text reaches x={rightmost:.2}, past the right margin at {:.2}pt",
        A4_WIDTH_PT - margin
    );
}

/// Deedy's "generous section spacing" moved out of `deedy.typ` (where it was a
/// local `sp-section-extra = 8pt`, the one template forking `_scale.typ`'s
/// locked rhythm) into `Template::section_above_extra`. The knob has to reach
/// the RENDER, not just sit in the registry: zero it and the first section
/// heading must rise by exactly those 8pt. A field nobody reads moves nothing.
#[test]
fn deedy_section_above_extra_moves_the_rendered_headings() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let deedy = Template::get(TemplateId::Deedy);
    assert_eq!(
        deedy.section_above_extra, 8.0,
        "fixture guard: Deedy is the template carrying the rhythm supplement"
    );
    let mut flat = deedy.clone();
    flat.section_above_extra = 0.0;

    // Line 0 = name, line 1 = contact (this fixture has no title), line 2 = the
    // first section heading — the first thing the supplement pushes down.
    let heading_y = |t: &Template| -> f64 {
        let lines = text_lines(&svg_page1(&model, t, false));
        assert!(lines.len() > 2, "expected a section heading, got {lines:?}");
        lines[2].0
    };

    let with = heading_y(&deedy);
    let without = heading_y(&flat);
    assert!(
        (with - without - 8.0).abs() < 0.1,
        "section_above_extra=8.0 must push the first heading down 8pt: \
         {with:.2} with the knob vs {without:.2} without ({:.2}pt apart)",
        with - without
    );
}

/// `deedy.typ`'s name-block splits `header.name` on the last space to color the
/// surname separately — guarded for a single-token name (`name-tokens.len() <=
/// 1`) that has nothing to split. This is the edge path a naive
/// `.slice(0, len - 1)` would panic on for an empty/underflowing range; render
/// it end-to-end (not just unit-test the split) to prove the guard actually
/// reaches production.
#[test]
fn deedy_single_token_name_does_not_panic() {
    let mut model = model_from_resume_text(FIXTURE_RESUME);
    model.header.name = "Cher".to_string();
    let template = Template::get(TemplateId::Deedy);

    for ats in [false, true] {
        let mut opts = opts_a4();
        opts.ats = ats;
        let bytes = render_pdf(
            &model,
            TypstTemplate::from_template(&template),
            &opts,
            Some(&template),
        )
        .unwrap_or_else(|e| panic!("deedy single-token name (ats={ats}) should render: {e:?}"));
        assert!(
            bytes.starts_with(b"%PDF"),
            "deedy single-token name (ats={ats}) must start with %PDF"
        );
    }
}

#[test]
fn classic_render_letter_page_succeeds() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 215.9,
            height_mm: 279.4,
        },
        lang: "en".to_string(),
        accent: None,
        ats: false,
    };
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts, Some(&classic))
        .expect("render_pdf(classic, Letter) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn classic_render_with_valid_accent_succeeds() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: Some("#1a2b3c".to_string()),
        lang: "en".to_string(),
        ats: false,
    };
    // Classic now renders through the parametric SingleColumn template, which
    // honors the accent override (data.opts.accent) — it must not crash.
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts, Some(&classic))
        .expect("render_pdf should succeed with a valid accent override");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn classic_render_with_invalid_accent_falls_back_gracefully() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: Some("not-a-color".to_string()),
        lang: "en".to_string(),
        ats: false,
    };
    // Invalid accent must not cause an error (normalise_accent returns "" →
    // template defaults apply).
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts, Some(&classic))
        .expect("render_pdf should succeed with an invalid accent color");
    assert!(bytes.starts_with(b"%PDF"));
}

// ── ATS harness ───────────────────────────────────────────────────────────────
//
// Renders the fixture through Classic, extracts text with pdf-extract, and
// asserts three ATS-safety properties:
//
//   (a) READING ORDER — section headings appear in the expected top-to-bottom
//       order (SUMMARY before EXPERIENCE before EDUCATION before SKILLS).
//
//   (b) WORD BOUNDARIES — a known multi-word phrase from the fixture survives
//       WITH spaces and is not run together (e.g. "State University" not
//       "StateUniversity").
//
//   (c) CONTENT PRESENT — the candidate name, all major section headings, and
//       a representative bullet fragment are findable in the extracted text.

#[test]
fn ats_harness_classic_reading_order_word_boundaries_content() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) for ATS harness");

    let extracted =
        pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed on our output");

    let lower = extracted.to_lowercase();

    // ── (c) Content present ───────────────────────────────────────────────────
    assert!(
        lower.contains("jane doe"),
        "ATS: candidate name 'Jane Doe' missing from extracted text\n---\n{extracted}"
    );

    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "ATS: section heading '{heading}' missing from extracted text\n---\n{extracted}"
        );
    }

    // A bullet fragment that must survive intact.
    assert!(
        lower.contains("distributed task scheduler"),
        "ATS: bullet fragment 'distributed task scheduler' missing\n---\n{extracted}"
    );

    // ── (b) Word boundaries ───────────────────────────────────────────────────
    // "State University" must appear with a space, not run together.
    assert!(
        lower.contains("state university"),
        "ATS: 'state university' must appear with preserved word boundary\n---\n{extracted}"
    );

    // ── (a) Reading order ─────────────────────────────────────────────────────
    let order = ["summary", "experience", "education", "skills"];
    let mut last_pos = 0usize;
    for heading in &order {
        let pos = lower.find(heading).unwrap_or_else(|| {
            panic!("ATS reading order: '{heading}' not found in extracted text")
        });
        assert!(
            pos >= last_pos,
            "ATS reading order: '{heading}' (at {pos}) appeared before previous heading (at {last_pos})\n---\n{extracted}"
        );
        last_pos = pos;
    }
}

// ── Accent normalisation unit tests (in render.rs tests, but also here) ───────

#[test]
fn render_opts_default_is_a4_en() {
    let opts = RenderOpts::default();
    assert_eq!(opts.page.width_mm, 210.0);
    assert_eq!(opts.page.height_mm, 297.0);
    assert_eq!(opts.lang, "en");
    assert!(!opts.ats);
    assert!(opts.accent.is_none());
}

// ── JsonSection.kind + emphasize_education unit tests ─────────────────────────
//
// These tests verify the two new data-model fields without needing a PDF render:
//
//   (a) section_id_to_kind: education section serializes as kind == "education".
//   (b) style_from_template: academic → emphasize_education == true;
//       swiss_minimal → emphasize_education == false.

#[test]
fn json_section_kind_education_serializes_correctly() {
    use crate::export::typst_engine::render::{section_id_to_kind, JsonSection};
    use crate::model::document::SectionId;

    let kind = section_id_to_kind(&SectionId::Education);
    assert_eq!(
        kind, "education",
        "SectionId::Education must serialize to \"education\""
    );

    // Spot-check a few other kinds while we are here.
    assert_eq!(section_id_to_kind(&SectionId::Experience), "experience");
    assert_eq!(section_id_to_kind(&SectionId::Skills), "skills");
    assert_eq!(
        section_id_to_kind(&SectionId::Custom("Foo".into())),
        "custom"
    );

    // Confirm a JsonSection round-trips through serde with kind present.
    let section = JsonSection {
        heading: "Education".to_string(),
        blocks: vec![],
        placement: "main".to_string(),
        kind: kind.clone(),
    };
    let json = serde_json::to_string(&section).expect("JsonSection must serialize");
    assert!(
        json.contains("\"kind\":\"education\""),
        "serialized JSON must contain \"kind\":\"education\"; got: {json}"
    );
}

#[test]
fn style_from_template_emphasize_education_academic_true_others_false() {
    use crate::export::typst_engine::render::style_from_template;

    let academic = template_style(TemplateId::Academic);
    assert!(
        style_from_template(&academic).emphasize_education,
        "Academic template must have emphasize_education == true"
    );

    let swiss = template_style(TemplateId::SwissMinimal);
    assert!(
        !style_from_template(&swiss).emphasize_education,
        "SwissMinimal template must have emphasize_education == false"
    );
}

// ── Atelier (Phase 1b) tests ──────────────────────────────────────────────────
//
// Tests cover:
//   (1) Basic render — valid PDF in both ats:false and ats:true.
//   (2) 2-page sidebar repeat — enough content to force ≥2 pages; ALL sidebar
//       items from the fixture must be present in the extracted text (regression
//       guard for the dense-sidebar overflow fix, F1/F4).
//   (3) ATS collapse — ats:true → linear reading order, sidebar headings appear
//       AFTER the main-column headings but still present and in order.
//   (4) Entry integrity — titles + bullets present in extracted text.
//   (5) Accent override — custom accent does not cause a compile error.
//   (6) Sample PDF written to target/ for human review (informational, always passes).
//   (7) Dense-sidebar fixture — 10+ skills, 2 degrees, 3 certs, 4 languages;
//       every sidebar item must be present (F1 regression guard).
//   (8) Empty-sidebar fixture — all sections placed in main; no sidebar sections;
//       template must fall back to single-column (no band) and render cleanly.

/// Single-page fixture — exercises all block types.
const ATELIER_FIXTURE: &str = "\
Alexandra Rivera
alex@example.com | [LinkedIn](https://linkedin.com/in/alexrivera) | https://alexrivera.dev

SUMMARY
Product-focused engineering leader with twelve years building distributed systems.

EXPERIENCE
Principal Engineer | Meridian Systems | 2019 – Present
- Scaled the event-sourcing platform to 500 k events per second
- Drove adoption of a domain-driven architecture across seven product teams

Software Engineer | Cobalt Labs | 2015 – 2019
- Built the real-time collaboration layer used by 200 k active users
- Reduced cold-start latency from 900 ms to 110 ms

EDUCATION
M.Sc. Computer Science | Western University | 2013 – 2015

SKILLS
Rust, Go, TypeScript, Kubernetes, AWS, Kafka, PostgreSQL

LANGUAGES
English (native), Portuguese (fluent)
";

/// Multi-page fixture — enough experience + project entries to force ≥2 pages.
/// The main-column content (SUMMARY + EXPERIENCE + PROJECTS) is deliberately
/// long enough to overflow a single A4 page in the 70% main column.
const ATELIER_MULTIPAGE: &str = "\
Alexandra Rivera
alex@example.com | https://alexrivera.dev

SUMMARY
Engineering leader with a decade of distributed-systems experience building resilient
platforms at scale. Passionate about developer productivity, reliability engineering,
and growing high-performing teams across multiple time zones.

EXPERIENCE
Staff Engineer | Apex Corp | 2022 – Present
- Led the platform-reliability initiative that reduced P99 latency by 60 percent across all production services
- Introduced chaos engineering practices that were adopted across twelve service teams globally
- Architected a zero-downtime schema migration pipeline managing a 10 TB customer dataset
- Mentored eight engineers through promotion to senior level over the course of eighteen months
- Drove the company-wide observability strategy resulting in 99.99 percent annual SLA achievement
- Defined engineering excellence standards that were subsequently adopted by all thirty backend teams
- Designed the on-call runbook system reducing mean time to resolution from 45 minutes to 8 minutes

Senior Engineer | Meridian Systems | 2019 – 2022
- Built the multi-tenant billing engine that processed 50 M transactions per month without downtime
- Migrated a legacy monolith to fifty domain-aligned microservices over an eighteen-month programme
- Designed the event-sourcing backbone now serving 300 k events per second at peak production load
- Reduced infrastructure cost by 35 percent through adaptive auto-scaling policies and spot instances
- Shipped a real-time analytics dashboard that was adopted by over 10 k business users on launch day
- Onboarded and technically led a distributed team of nine engineers across three time zones

Software Engineer | Cobalt Labs | 2016 – 2019
- Delivered the real-time collaboration layer for the flagship product used by 200 k daily active users
- Implemented end-to-end encryption for all user-generated content at rest and in transit
- Reduced cold-start API latency from 900 ms to 110 ms through optimised connection pooling strategies
- Contributed core modules to three open-source libraries with a combined 8 k GitHub stars

Junior Software Engineer | Vertex Startup | 2014 – 2016
- Shipped the initial iOS client that reached 50 k downloads in the first month after public launch
- Rebuilt the search indexing pipeline and cut ingestion lag from five minutes to eight seconds
- Integrated third-party payment providers handling 500 k transactions per day in a PCI-DSS environment

PROJECTS
Distributed Rate Limiter | Open Source | 2021
- Designed a Redis-backed token-bucket rate limiter with sub-millisecond overhead per request
- Published to crates.io; adopted by fourteen organisations within six months of initial release
- Maintained comprehensive documentation, changelog, and semver-stable public API

High-Throughput Log Aggregator | Open Source | 2020
- Built a lock-free ring-buffer pipeline aggregating 1 M log lines per second on commodity hardware
- Presented at a regional systems-programming conference to an audience of 400 engineers

EDUCATION
M.Sc. Computer Science | Western University | 2012 – 2014
B.Sc. Computer Engineering | Eastern College | 2008 – 2012

SKILLS
Rust, Go, TypeScript, Kubernetes, AWS, GCP, Kafka, PostgreSQL, Redis, Terraform, Prometheus, Grafana

LANGUAGES
English (native), Portuguese (fluent), Spanish (working)
";

/// Dense-sidebar fixture — 10+ skills, 2 degrees, 3 certifications, 4 languages.
/// This is the F1 regression fixture: the sidebar content is tall enough that
/// the template must detect overflow and fall back to single-column so that
/// no sidebar item is silently clipped.
const ATELIER_DENSE_SIDEBAR: &str = "\
Jordan Kim
jordan@example.com | https://linkedin.com/in/jordankim | https://jordankim.dev

SUMMARY
Polyglot engineer with deep expertise in distributed systems and cloud infrastructure.

EXPERIENCE
Senior Platform Engineer | Globex Corp | 2020 – Present
- Designed a multi-region failover system achieving five nines availability
- Reduced mean deployment time from 45 minutes to under four minutes

Platform Engineer | Initech Solutions | 2017 – 2020
- Built a shared CI/CD platform adopted by 80 engineering teams
- Introduced contract testing reducing integration failures by 70 percent

EDUCATION
M.Eng. Software Engineering | Metro University | 2015 – 2017
B.Sc. Computer Science | Coastal College | 2011 – 2015

SKILLS
Rust, Go, Python, TypeScript, Java, Kotlin, C++, Bash, SQL, Terraform, Ansible, Pulumi

LANGUAGES
English (native), German (fluent), French (professional), Mandarin (conversational)

CERTIFICATIONS
AWS Solutions Architect Professional
Google Cloud Professional Data Engineer
Certified Kubernetes Administrator
";

fn opts_atelier(ats: bool) -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: Some("#4A4580".to_string()),
        lang: "en".to_string(),
        ats,
    }
}

// (1a) Non-ATS render produces a valid PDF.
#[test]
fn atelier_render_produces_valid_pdf() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier) should succeed");

    assert!(!bytes.is_empty(), "PDF must not be empty");
    assert!(bytes.starts_with(b"%PDF"), "output must start with %PDF");
}

// (1b) ATS render also produces a valid PDF.
#[test]
fn atelier_ats_render_produces_valid_pdf() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(true), None)
        .expect("render_pdf(atelier, ats:true) should succeed");

    assert!(!bytes.is_empty(), "ATS PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "ATS output must start with %PDF"
    );
}

// (2) 2-page sidebar repeat: the multi-page fixture forces ≥2 pages.
// The FULL set of sidebar items from the multipage fixture must be present
// in the extracted text — this is the regression guard for F1/F4 (dense
// sidebar overflow).  A clipped sidebar would cause these assertions to fail.
#[test]
fn atelier_multipage_sidebar_renders_once() {
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, multipage) should succeed");

    assert!(bytes.starts_with(b"%PDF"));

    // Assert ≥2 pages by counting /Type /Page objects directly in the PDF bytes.
    let page_count = count_pdf_pages(&bytes);
    assert!(
        page_count >= 2,
        "multi-page fixture must produce ≥2 pages; got {page_count}"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on our Typst PDF");

    // Normalise: collapse all whitespace (newlines, multiple spaces) to a single
    // space so line-wrapped tokens ("Eastern \nCollege") still match. Education
    // entries are now rendered as entry blocks (grid layout) which can introduce
    // line breaks inside multi-word names — normalization makes assertions robust.
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Every sidebar skill from the fixture must be present.
    let sidebar_skills = [
        "rust",
        "go",
        "typescript",
        "kubernetes",
        "aws",
        "gcp",
        "kafka",
        "postgresql",
        "redis",
        "terraform",
        "prometheus",
        "grafana",
    ];
    for skill in &sidebar_skills {
        assert!(
            lower.contains(skill),
            "sidebar skill '{skill}' missing from extracted text — possible sidebar clip\n---\n{lower}"
        );
    }

    // Education entries in the sidebar must also be present.
    assert!(
        lower.contains("western university"),
        "sidebar education 'western university' missing\n---\n{lower}"
    );
    assert!(
        lower.contains("eastern college"),
        "sidebar education 'eastern college' missing\n---\n{lower}"
    );

    // Languages must be present.
    for lang in &["english", "portuguese", "spanish"] {
        assert!(
            lower.contains(lang),
            "sidebar language '{lang}' missing\n---\n{lower}"
        );
    }

    // The sidebar now renders ONCE (page 1 only), no longer repeated per page.
    // A sidebar-only skill ("Grafana" — never appears in a main-column bullet)
    // must therefore appear exactly once across the whole multi-page document.
    let grafana_count = lower.matches("grafana").count();
    assert_eq!(
        grafana_count, 1,
        "sidebar skill 'Grafana' must appear exactly once (sidebar renders once, \
         not repeated per page); found {grafana_count}\n---\n{lower}"
    );
}

#[test]
fn portrait_multipage_sidebar_renders_once() {
    use crate::export::typst_engine::render_pdf_with_photo;

    // Same multi-page fixture through Portrait (no photo). Portrait uses the same
    // page(background:) sidebar technique, so the page-1-only gate must hold here too.
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(portrait, multipage) should succeed");
    assert!(bytes.starts_with(b"%PDF"));

    let page_count = count_pdf_pages(&bytes);
    assert!(
        page_count >= 2,
        "multi-page fixture must produce ≥2 pages; got {page_count}"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    let lower = extracted.to_lowercase();
    // Sidebar content present (on page 1) …
    assert!(
        lower.contains("grafana"),
        "sidebar skill missing\n---\n{lower}"
    );
    // … and rendered exactly once, not repeated per page.
    assert_eq!(
        lower.matches("grafana").count(),
        1,
        "Portrait sidebar must render once across pages\n---\n{lower}"
    );
}

// (3) ATS collapse: ats:true → single column, linear reading order.
// Main headings (SUMMARY, EXPERIENCE) must appear before sidebar headings
// (EDUCATION, SKILLS, LANGUAGES) in the extracted text.
#[test]
fn atelier_ats_linear_reading_order() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(true), None)
        .expect("render_pdf(atelier, ats:true) should succeed");

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed");

    let lower = extracted.to_lowercase();

    // All major section headings must be present.
    for heading in &["summary", "experience", "education", "skills", "languages"] {
        assert!(
            lower.contains(heading),
            "ATS: heading '{heading}' missing from extracted text\n---\n{extracted}"
        );
    }

    // Main-column sections must appear before sidebar-column sections in the
    // extracted text (linear order = no column interleaving).
    let pos_experience = lower
        .find("experience")
        .expect("'experience' must be present");
    let pos_education = lower
        .find("education")
        .expect("'education' must be present");
    let pos_skills = lower.find("skills").expect("'skills' must be present");

    assert!(
        pos_experience < pos_education,
        "ATS: 'experience' ({pos_experience}) should precede 'education' ({pos_education}) \
         in linear order\n---\n{extracted}"
    );
    assert!(
        pos_experience < pos_skills,
        "ATS: 'experience' ({pos_experience}) should precede 'skills' ({pos_skills}) \
         in linear order\n---\n{extracted}"
    );

    // Word boundaries: "Western University" must appear with a space.
    assert!(
        lower.contains("western university"),
        "ATS: 'western university' must appear with preserved word boundary\n---\n{extracted}"
    );
}

// (4) Entry integrity: entry titles and bullet fragments must all be present.
#[test]
fn atelier_entry_integrity() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier) should succeed");

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed");

    let lower = extracted.to_lowercase();

    // Candidate name.
    assert!(
        lower.contains("alexandra rivera"),
        "entry integrity: candidate name missing\n---\n{extracted}"
    );

    // Entry titles.
    for title in &["meridian systems", "cobalt labs"] {
        assert!(
            lower.contains(title),
            "entry integrity: title '{title}' missing\n---\n{extracted}"
        );
    }

    // Bullet fragments.
    assert!(
        lower.contains("event-sourcing platform"),
        "entry integrity: bullet fragment 'event-sourcing platform' missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("real-time collaboration"),
        "entry integrity: bullet fragment 'real-time collaboration' missing\n---\n{extracted}"
    );
}

// (5) Custom accent override does not cause a compile error.
#[test]
fn atelier_custom_accent_succeeds() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: Some("#1A6B5A".to_string()), // deep teal override
        lang: "en".to_string(),
        ats: false,
    };
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts, None)
        .expect("render_pdf(atelier, custom accent) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
}

// (6) Write a classic sample PDF to target/ for human review.
// This test always passes; it is informational.
// Uses .ok() so a read-only target/ directory does not fail the test run.
#[test]
fn classic_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("classic_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("classic_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Classic sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "classic_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}

// (6b) Write an atelier sample PDF to target/ for human review.
// This test always passes; it is informational.
// Uses .ok() so a read-only target/ directory does not fail the test run.
#[test]
fn atelier_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("atelier_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("atelier_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Atelier sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "atelier_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}

// (6c) Write a MULTI-PAGE atelier sample to target/ for human review.
// Forces ≥2 pages so the page-background sidebar repeat + pagination + the
// locked house spacing scale can be eyeballed across a page break.
// Informational; .ok()-style write never fails the run.
#[test]
fn atelier_write_multipage_sample_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, multipage) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("atelier_write_multipage_sample_for_review: could not create target/: {e}");
    }
    let out_path = target.join("atelier_multipage_diag.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Atelier multipage sample written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "atelier_write_multipage_sample_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}

// (7) Dense-sidebar fixture: 10+ skills, 2 degrees, 3 certs, 4 languages.
// Every sidebar item must appear in the extracted PDF text, proving that
// the dense-sidebar overflow detection (F1) correctly falls back to
// single-column and does NOT silently clip any content.
#[test]
fn atelier_dense_sidebar_no_data_loss() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_DENSE_SIDEBAR);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, dense-sidebar) should succeed");

    assert!(
        bytes.starts_with(b"%PDF"),
        "dense-sidebar PDF must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on dense-sidebar output");

    // Normalise: collapse all whitespace (newlines, multiple spaces) to a
    // single space so that line-wrapped tokens ("Coastal \nCollege") still
    // match the expected substrings.
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // ── Skills (10+) ──────────────────────────────────────────────────────────
    let skills = [
        "rust",
        "go",
        "python",
        "typescript",
        "java",
        "kotlin",
        "c++",
        "bash",
        "sql",
        "terraform",
        "ansible",
        "pulumi",
    ];
    for skill in &skills {
        assert!(
            lower.contains(skill),
            "dense-sidebar: skill '{skill}' missing — possible silent clip\n---\n{lower}"
        );
    }

    // ── Education (2 degrees) ─────────────────────────────────────────────────
    assert!(
        lower.contains("metro university"),
        "dense-sidebar: 'metro university' missing\n---\n{lower}"
    );
    assert!(
        lower.contains("coastal college"),
        "dense-sidebar: 'coastal college' missing\n---\n{lower}"
    );

    // ── Languages (4) ─────────────────────────────────────────────────────────
    for lang in &["english", "german", "french", "mandarin"] {
        assert!(
            lower.contains(lang),
            "dense-sidebar: language '{lang}' missing\n---\n{lower}"
        );
    }

    // ── Certifications (3) ────────────────────────────────────────────────────
    assert!(
        lower.contains("aws solutions architect"),
        "dense-sidebar: 'aws solutions architect' cert missing\n---\n{lower}"
    );
    assert!(
        lower.contains("google cloud"),
        "dense-sidebar: 'google cloud' cert missing\n---\n{lower}"
    );
    assert!(
        lower.contains("kubernetes administrator"),
        "dense-sidebar: 'kubernetes administrator' cert missing\n---\n{lower}"
    );

    // Write the dense-sidebar sample for eyeballing.
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("atelier_dense_sidebar_no_data_loss: could not create target/: {e}");
    }
    let out_path = target.join("atelier_dense_sidebar.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!(
            "Dense-sidebar sample PDF written to: {}",
            out_path.display()
        ),
        Err(e) => eprintln!(
            "atelier_dense_sidebar_no_data_loss: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
}

// (8) Empty-sidebar fixture: all sections map to main; no sidebar sections.
// The template must render cleanly in single-column mode (no band) and all
// content must be present in the extracted text.
#[test]
fn atelier_empty_sidebar_renders_single_column() {
    // A resume with only SUMMARY + EXPERIENCE + PROJECTS — none of these
    // sections map to the sidebar (Skills/Education/Languages/Certifications
    // are the sidebar sections).  The template must detect no sidebar sections
    // and fall back to single-column to avoid rendering an empty tinted band.
    let fixture = "\
Morgan Ellis
morgan@example.com | https://morganellis.dev

SUMMARY
Full-stack engineer specialising in high-throughput data pipelines.

EXPERIENCE
Senior Engineer | DataCo | 2020 – Present
- Designed a streaming ingestion layer processing 2 M events per second
- Reduced P99 query latency from 800 ms to 35 ms via index optimisation

Engineer | PipeCraft | 2017 – 2020
- Built the core ETL framework adopted by all twelve data teams
- Migrated a batch pipeline to a streaming architecture with zero downtime

PROJECTS
OpenStream | Open Source | 2022
- High-throughput event router with pluggable backends
- 2 k GitHub stars; used in production by three Fortune 500 companies
";

    let model = model_from_resume_text(fixture);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, empty-sidebar) should succeed");

    assert!(
        bytes.starts_with(b"%PDF"),
        "empty-sidebar PDF must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on empty-sidebar output");

    let lower = extracted.to_lowercase();

    // All content must be present — none clipped by a missing sidebar.
    assert!(
        lower.contains("morgan ellis"),
        "empty-sidebar: candidate name missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("dataco"),
        "empty-sidebar: 'dataco' entry missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("streaming ingestion"),
        "empty-sidebar: bullet fragment missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("openstream"),
        "empty-sidebar: project 'openstream' missing\n---\n{extracted}"
    );
}

// ── Phase 2: Classic, SwissMinimal, Academic — SingleColumn parametric ────────
//
// For each new template:
//   (a) Render produces a valid PDF.
//   (b) ATS harness: reading order + word boundaries + content present.
//   (c) Sample PDF written to target/ for human review (informational, always passes).

fn template_style(id: TemplateId) -> Template {
    Template::get(id)
}

fn opts_sc() -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats: false,
    }
}

// ── Swiss Minimal ─────────────────────────────────────────────────────────────

#[test]
fn swiss_minimal_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::SwissMinimal);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(swiss-minimal) should succeed");
    assert!(!bytes.is_empty(), "Swiss Minimal PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Swiss Minimal output must start with %PDF"
    );
}

#[test]
fn swiss_minimal_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::SwissMinimal);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(swiss-minimal) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on swiss-minimal output");
    let lower = extracted.to_lowercase();

    assert!(
        lower.contains("jane doe"),
        "swiss-minimal ATS: 'jane doe' missing\n---\n{extracted}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "swiss-minimal ATS: heading '{heading}' missing\n---\n{extracted}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "swiss-minimal ATS: bullet fragment missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("state university"),
        "swiss-minimal ATS: 'state university' word boundary broken\n---\n{extracted}"
    );

    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("swiss-minimal ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "swiss-minimal ATS: '{h}' ({pos}) before previous ({last})"
        );
        last = pos;
    }
}

#[test]
fn swiss_minimal_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::SwissMinimal);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(swiss-minimal) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("swiss_minimal_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("swiss_minimal_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Swiss Minimal sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "swiss_minimal_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// ── Academic ──────────────────────────────────────────────────────────────────

#[test]
fn academic_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Academic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(academic) should succeed");
    assert!(!bytes.is_empty(), "Academic PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Academic output must start with %PDF"
    );
}

#[test]
fn academic_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Academic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(academic) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on academic output");
    let lower = extracted.to_lowercase();

    assert!(
        lower.contains("jane doe"),
        "academic ATS: 'jane doe' missing\n---\n{extracted}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "academic ATS: heading '{heading}' missing\n---\n{extracted}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "academic ATS: bullet fragment missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("state university"),
        "academic ATS: 'state university' word boundary broken\n---\n{extracted}"
    );

    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("academic ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "academic ATS: '{h}' ({pos}) before previous ({last})"
        );
        last = pos;
    }
}

#[test]
fn academic_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Academic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(academic) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("academic_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("academic_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Academic sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "academic_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// ── Phase 1c: Cover-letter render tests ───────────────────────────────────────
//
// Tests cover:
//   (1) US letter — renders valid PDF on Letter-size page (215.9 × 279.4 mm),
//       no subject line; salutation, body phrase, sign-off present.
//   (2) DE letter — renders valid PDF on A4; DIN subject "Betreff:" present;
//       German salutation + signoff recognised.
//   (3) Both PDFs start with %PDF.
//   (4) Sample PDF writers: target/letter_us_sample.pdf and
//       target/letter_de_sample.pdf — informational, always pass.

/// US English cover letter fixture.
const LETTER_FIXTURE_US: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street
New York, NY 10001

Dear Hiring Manager,

I am writing to express my strong interest in the Software Engineer position at \
Acme Corp. With five years of experience building distributed systems in Rust and \
Go, I believe I would be a great addition to your team.

During my time at Beta Inc, I led the migration of our payments service to a \
microservices architecture, reducing end-to-end latency by 40 percent and \
cutting infrastructure costs by 30 percent.

I would welcome the opportunity to discuss how my background aligns with your needs.

Sincerely,

Jane Smith
Software Engineer
";

/// German DIN 5008 cover letter fixture.
const LETTER_FIXTURE_DE: &str = "\
Max Müller
max@example.de | https://linkedin.com/in/maxmueller

Frankfurt, 2. Juni 2025

Frau Dr. Anna Weber
Musterfirma GmbH
Hauptstraße 1
60311 Frankfurt am Main

Betreff: Bewerbung als Software Engineer

Sehr geehrte Frau Dr. Weber,

mit großem Interesse habe ich Ihre Stellenausschreibung für die Position als \
Software Engineer gelesen. Ich bewerbe mich hiermit für diese Stelle.

In meiner bisherigen Tätigkeit bei der Beta GmbH habe ich umfangreiche Erfahrungen \
in der Entwicklung verteilter Systeme gesammelt und konnte die Systemlatenz um \
40 Prozent reduzieren.

Über eine Einladung zum Vorstellungsgespräch würde ich mich sehr freuen.

Mit freundlichen Grüßen,

Max Müller
";

/// Accented-Latin cover-letter fixture — grave-accented lowercase (à, ò, ì)
/// PLUS capital grave accents (È, À), the shape the `no_extractable_text`
/// incident audit flagged as under-tested (see [`ACCENTED_RESUME_FIXTURE`]
/// for the full rationale). US-market shape (reuses [`LETTER_FIXTURE_US`]'s
/// structure) so DIN-specific parsing isn't a confound here.
const LETTER_FIXTURE_IT: &str = "\
Àlvaro Èsposito
alvaro.esposito@example.it | https://linkedin.com/in/alvaroesposito

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street
New York, NY 10001

Dear Hiring Manager,

I am writing to express my strong interest in the Software Engineer position at \
Acme Corp. Growing up near Città di Torino and studying at the Università degli \
Studi, I built a solid foundation in distributed systems — però my passion has \
always been building things that scale così well they disappear into the \
background.

During my five years at Beta Inc, I led the migration of our payments service to \
a microservices architecture, reducing end-to-end latency by 40 percent.

Sincerely,

Àlvaro Èsposito
Software Engineer
";

// (1) US letter renders to a valid PDF and contains expected text.
#[test]
fn letter_us_renders_valid_pdf_with_expected_content() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(us) should succeed");

    assert!(!bytes.is_empty(), "US letter PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "US letter output must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on US letter output");
    let lower = extracted.to_lowercase();

    // Salutation must be present.
    assert!(
        lower.contains("dear hiring manager"),
        "US letter: salutation 'Dear Hiring Manager' missing\n---\n{extracted}"
    );

    // A body phrase must survive.
    assert!(
        lower.contains("distributed systems"),
        "US letter: body phrase 'distributed systems' missing\n---\n{extracted}"
    );

    // Sign-off must be present.
    assert!(
        lower.contains("sincerely"),
        "US letter: sign-off 'Sincerely' missing\n---\n{extracted}"
    );

    // Signature name.
    assert!(
        lower.contains("jane smith"),
        "US letter: signature name 'Jane Smith' missing\n---\n{extracted}"
    );

    // Ordering: salutation before body before sign-off.
    let pos_sal = lower.find("dear").expect("salutation must be present");
    let pos_body = lower
        .find("distributed")
        .expect("body phrase must be present");
    let pos_signoff = lower.find("sincerely").expect("sign-off must be present");
    assert!(
        pos_sal < pos_body && pos_body < pos_signoff,
        "US letter: reading order broken — sal={pos_sal} body={pos_body} signoff={pos_signoff}"
    );

    // Recipient / inside-address block — the day-one Classic bug dropped it
    // entirely (fixture `recipientPosition: "left"` matched neither the old
    // "after-date" nor "" gate). The street line is unique to the recipient,
    // and the FIRST "Acme Corp" (the body also names it) must read before the
    // salutation, proving the inside address renders in the right slot.
    assert!(
        lower.contains("123 main street"),
        "US letter: recipient street '123 Main Street' missing — inside address dropped\n---\n{extracted}"
    );
    let pos_recipient = lower
        .find("acme corp")
        .expect("recipient company must be present");
    assert!(
        pos_recipient < pos_sal,
        "US letter: inside address must read before the salutation — recipient={pos_recipient} sal={pos_sal}"
    );
}

// (2) DE letter renders to a valid PDF and contains DIN subject + German conventions.
#[test]
fn letter_de_renders_valid_pdf_with_subject_line() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(de) should succeed");

    assert!(!bytes.is_empty(), "DE letter PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "DE letter output must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on DE letter output");

    // Normalise whitespace (Typst can wrap long lines).
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Subject label "Betreff" must be present.
    assert!(
        lower.contains("betreff"),
        "DE letter: subject label 'Betreff' missing\n---\n{lower}"
    );

    // German salutation.
    assert!(
        lower.contains("sehr geehr"),
        "DE letter: German salutation 'Sehr geehr...' missing\n---\n{lower}"
    );

    // Body phrase.
    assert!(
        lower.contains("verteilter systeme") || lower.contains("verteilter"),
        "DE letter: body phrase missing\n---\n{lower}"
    );

    // German sign-off.
    assert!(
        lower.contains("freundlichen"),
        "DE letter: German sign-off missing\n---\n{lower}"
    );

    // Signature name.
    assert!(
        lower.contains("max") && lower.contains("müller"),
        "DE letter: signature name missing\n---\n{lower}"
    );

    // Recipient / inside-address block (Anschriftfeld) — DIN 5008 makes it
    // MANDATORY, yet the day-one Classic bug dropped it entirely. Company +
    // recipient name are unique to the inside address (the body names "Beta
    // GmbH", the salutation only "Weber"), and both must read before the
    // Betreff subject line.
    assert!(
        lower.contains("musterfirma gmbh"),
        "DE letter: recipient company 'Musterfirma GmbH' missing — Anschriftfeld dropped\n---\n{lower}"
    );
    assert!(
        lower.contains("anna weber"),
        "DE letter: recipient name 'Anna Weber' missing\n---\n{lower}"
    );
    let pos_recipient = lower
        .find("musterfirma gmbh")
        .expect("recipient company must be present");
    let pos_subject = lower.find("betreff").expect("subject must be present");
    assert!(
        pos_recipient < pos_subject,
        "DE letter: Anschriftfeld must read before the Betreff line — recipient={pos_recipient} subject={pos_subject}"
    );
}

// (3) Both outputs start with %PDF — belt-and-suspenders after the content tests
// above already assert this; kept as a quick standalone guard.
#[test]
fn letter_us_and_de_both_start_with_pdf_header() {
    let t = Template::get(TemplateId::SwissMinimal);
    let us = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(us)");
    let de = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(de)");
    assert!(us.starts_with(b"%PDF"), "US letter must start with %PDF");
    assert!(de.starts_with(b"%PDF"), "DE letter must start with %PDF");
}

// (4a) Write the US letter sample to target/ for human eyeballing.
// Informational; always passes; .ok()-style write.
#[test]
fn letter_us_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(us) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("letter_us_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("letter_us_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("US letter sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "letter_us_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// (4b) Write the DE letter sample to target/ for human eyeballing.
// Informational; always passes; .ok()-style write.
#[test]
fn letter_de_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_pdf(de) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("letter_de_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("letter_de_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("DE letter sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "letter_de_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// ── PR5: Letter layouts (Classic / Refined / Banded) ──────────────────────────
//
// The three layouts share the identical `LetterModel` / `data.json` contract;
// only the arrangement (`letter*.typ` source) differs. Palette + fonts still
// inherit from the résumé template, and market conventions (DE DIN date-top-
// right + subject line, US below-header) are still honoured per layout.

/// US letter carrying an explicit "Re: …" subject line. The US market sets
/// `subject_line_used = false`, so the Classic layout drops this subject while
/// the Refined layout always foregrounds it — the discriminator below.
const LETTER_FIXTURE_US_SUBJECT: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street

Re: Application for Platform Engineer (Ref PX-2291)

Dear Hiring Manager,

I am writing to express my strong interest in the Platform Engineer position at \
Acme Corp, where I would bring five years of distributed-systems experience.

Sincerely,

Jane Smith
Software Engineer
";

/// A long US letter that reflows onto multiple pages — used to prove the Banded
/// layout draws its accent band on page 1 only.
const LETTER_FIXTURE_LONG_US: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp

Dear Hiring Manager,

I am writing to express my strong interest in the Software Engineer position at \
Acme Corp. Over the past five years I have designed and operated distributed \
systems in Rust and Go, consistently reducing latency and cost while raising the \
reliability bar for every team I have worked with.

During my time at Beta Inc I led the migration of our payments service to a \
microservices architecture, reducing end-to-end latency by 40 percent and \
cutting infrastructure costs by 30 percent across the platform.

I introduced a service-level-objective culture, instrumented the critical paths, \
and mentored a cohort of engineers who now own those services end to end. The \
result was a measurable drop in incidents and a faster, calmer on-call rotation.

At Gamma LLC I rebuilt the ingestion pipeline to handle a tenfold increase in \
event volume without a proportional increase in cost, using back-pressure and \
adaptive batching to keep tail latency predictable under bursty load.

I care deeply about developer experience, and I have shipped internal tooling \
that shortened the local feedback loop from minutes to seconds, which paid for \
itself many times over in team velocity and morale.

Beyond the technical work, I have partnered closely with product and design to \
make sure the systems we build actually serve the people who use them, and I \
have found that this partnership consistently produces better outcomes.

I would welcome the opportunity to bring the same rigour, curiosity, and sense \
of ownership to your team, and to help you scale the platform through its next \
phase of growth with confidence and care.

Earlier in my career at Delta Systems I built the on-call tooling that our whole \
engineering org still relies on, cutting mean time to resolution significantly \
by surfacing the right context the moment an alert fired.

I have also invested heavily in testing culture, introducing contract tests \
between services that caught integration regressions before they ever reached \
production, which meaningfully reduced the number of incidents quarter over \
quarter.

Outside of pure execution, I enjoy mentoring engineers earlier in their careers, \
pairing regularly and helping them build the judgment to make good trade-offs \
under real constraints rather than just following a checklist.

I have led cross-team initiatives that required aligning stakeholders with \
different priorities, and I take real pride in finding the solution that \
actually satisfies everyone's constraints rather than the loudest one.

Reliability work is often invisible when done well, so I have made a habit of \
writing clear postmortems and sharing them broadly, turning painful incidents \
into lasting organizational learning rather than one-off fire drills.

Thank you for considering my application; I would be glad to discuss how my \
background aligns with your needs at any time that is convenient for you.

Sincerely,

Jane Smith
Software Engineer
";

/// Distinct `fill="#…"` colours present in an SVG string.
fn svg_fill_colors(svg: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let needle = "fill=\"";
    let mut rest = svg;
    while let Some(pos) = rest.find(needle) {
        rest = &rest[pos + needle.len()..];
        if let Some(end) = rest.find('"') {
            out.insert(rest[..end].to_string());
        }
    }
    out
}

// (R1) Refined layout renders a valid US PDF with correct reading order.
#[test]
fn letter_refined_us_renders_valid_pdf() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Refined,
            ats: false,
        },
    )
    .expect("refined US render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "refined US must start with %PDF"
    );

    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on refined US")
        .to_lowercase();
    assert!(
        lower.contains("dear hiring manager"),
        "salutation missing:\n{lower}"
    );
    assert!(
        lower.contains("distributed systems"),
        "body phrase missing:\n{lower}"
    );
    assert!(lower.contains("sincerely"), "sign-off missing:\n{lower}");
    assert!(
        lower.contains("jane smith"),
        "signature name missing:\n{lower}"
    );

    let pos_sal = lower.find("dear").expect("salutation present");
    let pos_body = lower.find("distributed").expect("body present");
    let pos_signoff = lower.find("sincerely").expect("sign-off present");
    assert!(
        pos_sal < pos_body && pos_body < pos_signoff,
        "refined US reading order broken — sal={pos_sal} body={pos_body} signoff={pos_signoff}"
    );

    // Recipient inside-address renders (unconditional in Refined) — pin it so a
    // future refactor can't regress it the way Classic silently did.
    assert!(
        lower.contains("123 main street"),
        "refined US: recipient inside address missing:\n{lower}"
    );
    assert!(
        lower.find("acme corp").is_some_and(|p| p < pos_sal),
        "refined US: inside address must read before the salutation:\n{lower}"
    );
}

// (R2) Refined honours DE DIN conventions: A4, Betreff subject present, German
// salutation + sign-off, and the DIN top-right date reads before the salutation.
#[test]
fn letter_refined_de_honors_din_conventions() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Refined,
            ats: false,
        },
    )
    .expect("refined DE render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "refined DE must start with %PDF"
    );

    let normalised: String = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on refined DE")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let lower = normalised.to_lowercase();

    // The subject text survives (rendered as the job-reference line). The
    // "Betreff" label prefix is stripped, so assert on the subject body.
    assert!(
        lower.contains("bewerbung"),
        "DE subject body missing:\n{lower}"
    );
    assert!(
        lower.contains("sehr geehr"),
        "German salutation missing:\n{lower}"
    );
    assert!(
        lower.contains("freundlichen"),
        "German sign-off missing:\n{lower}"
    );
    // Signature name — previously unasserted here, leaving the Refined layout
    // with zero extraction coverage of the candidate's own name.
    assert!(
        lower.contains("max") && lower.contains("müller"),
        "refined DE: signature name missing\n---\n{lower}"
    );

    // DIN date-top-right → the date reads near the top, before the salutation.
    let pos_date = lower.find("2025").expect("date present");
    let pos_sal = lower.find("sehr geehr").expect("salutation present");
    assert!(
        pos_date < pos_sal,
        "DE DIN date should precede the salutation — date={pos_date} sal={pos_sal}"
    );
}

// (R2b) Refined round-trips accented-Latin content — grave lowercase + capital
// È/À (see `ACCENTED_RESUME_FIXTURE`/`LETTER_FIXTURE_IT` doc comments for
// rationale). Complements (R2) above, which only exercises German ü/ß.
#[test]
fn letter_refined_extracts_accented_latin_content() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_IT,
        &t,
        None,
        Some("Àlvaro Èsposito"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Refined,
            ats: false,
        },
    )
    .expect("refined accented-Latin render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "refined accented-Latin must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on refined accented-Latin output");
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    assert!(
        lower.contains("àlvaro") && lower.contains("èsposito"),
        "refined: accented signature name missing — capitals È/À did not survive extraction\n---\n{extracted}"
    );
    // The name appears TWICE — letterhead and signature — so the global
    // `contains` above still passes if extraction drops the whole sign-off
    // block. Pin the signature itself by looking only after the sign-off.
    assert!(
        signature_block(&lower).contains("àlvaro èsposito"),
        "refined: accented name missing from the SIGNATURE (after the sign-off) — a \
         letterhead-only match would hide a dropped signature\n---\n{extracted}"
    );
    assert!(
        lower.contains("così") || lower.contains("però") || lower.contains("città"),
        "refined: grave-accented-lowercase body word missing\n---\n{extracted}"
    );

    let normalized_len = normalize_like_validator(&extracted).len();
    assert!(
        normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
        "refined accented-Latin: only {normalized_len} normalized chars extracted — \
         the real validator's no_extractable_text gate would block this export"
    );
}

// (R3) Refined always shows a subject as the JOB REFERENCE line — even when the
// market omits the subject (US `subject_line_used = false`). Classic drops it.
#[test]
fn letter_refined_shows_subject_when_market_omits_it() {
    let t = Template::get(TemplateId::SwissMinimal);

    let refined = render_letter_pdf(
        LETTER_FIXTURE_US_SUBJECT,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Refined,
            ats: false,
        },
    )
    .expect("refined US-subject render");
    let refined_txt = pdf_extract::extract_text_from_mem(&refined)
        .expect("pdf-extract refined")
        .to_lowercase();
    assert!(
        refined_txt.contains("px-2291"),
        "Refined must render the subject reference (px-2291):\n{refined_txt}"
    );

    let classic = render_letter_pdf(
        LETTER_FIXTURE_US_SUBJECT,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("classic US-subject render");
    let classic_txt = pdf_extract::extract_text_from_mem(&classic)
        .expect("pdf-extract classic")
        .to_lowercase();
    assert!(
        !classic_txt.contains("px-2291"),
        "Classic must NOT render the subject when the market omits it (subject_line_used=false):\n{classic_txt}"
    );
}

// (B1) Banded layout renders a valid US PDF with correct reading order.
#[test]
fn letter_banded_us_renders_valid_pdf() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Banded,
            ats: false,
        },
    )
    .expect("banded US render should succeed");
    assert!(bytes.starts_with(b"%PDF"), "banded US must start with %PDF");

    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on banded US")
        .to_lowercase();
    assert!(
        lower.contains("dear hiring manager"),
        "salutation missing:\n{lower}"
    );
    assert!(
        lower.contains("distributed systems"),
        "body phrase missing:\n{lower}"
    );
    assert!(lower.contains("sincerely"), "sign-off missing:\n{lower}");

    let pos_sal = lower.find("dear").expect("salutation present");
    let pos_body = lower.find("distributed").expect("body present");
    let pos_signoff = lower.find("sincerely").expect("sign-off present");
    assert!(
        pos_sal < pos_body && pos_body < pos_signoff,
        "banded US reading order broken — sal={pos_sal} body={pos_body} signoff={pos_signoff}"
    );

    // Recipient inside-address renders (unconditional in Banded) — pin it so a
    // future refactor can't regress it the way Classic silently did.
    assert!(
        lower.contains("123 main street"),
        "banded US: recipient inside address missing:\n{lower}"
    );
    assert!(
        lower.find("acme corp").is_some_and(|p| p < pos_sal),
        "banded US: inside address must read before the salutation:\n{lower}"
    );
}

// (B2) Banded honours DE conventions: A4, Betreff subject (subject_line_used),
// German salutation + sign-off.
#[test]
fn letter_banded_de_honors_din_subject() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_DE,
        &t,
        None,
        Some("Max Müller"),
        LetterRender {
            market: "de",
            lang: "de",
            layout: LetterLayout::Banded,
            ats: false,
        },
    )
    .expect("banded DE render should succeed");
    assert!(bytes.starts_with(b"%PDF"), "banded DE must start with %PDF");

    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on banded DE")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    assert!(
        lower.contains("betreff"),
        "DE Betreff subject missing:\n{lower}"
    );
    assert!(
        lower.contains("sehr geehr"),
        "German salutation missing:\n{lower}"
    );
    assert!(
        lower.contains("freundlichen"),
        "German sign-off missing:\n{lower}"
    );
    assert!(
        lower.contains("max") && lower.contains("müller"),
        "banded DE: signature name missing\n---\n{lower}"
    );
}

// (B2b) Banded round-trips accented-Latin content — grave lowercase + capital
// È/À (see `ACCENTED_RESUME_FIXTURE`/`LETTER_FIXTURE_IT` doc comments for
// rationale). Complements (B2) above, which only exercises German ü/ß.
#[test]
fn letter_banded_extracts_accented_latin_content() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_IT,
        &t,
        None,
        Some("Àlvaro Èsposito"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Banded,
            ats: false,
        },
    )
    .expect("banded accented-Latin render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "banded accented-Latin must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on banded accented-Latin output");
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    assert!(
        lower.contains("àlvaro") && lower.contains("èsposito"),
        "banded: accented signature name missing — capitals È/À did not survive extraction\n---\n{extracted}"
    );
    // Same letterhead-vs-signature distinction as the refined case above.
    assert!(
        signature_block(&lower).contains("àlvaro èsposito"),
        "banded: accented name missing from the SIGNATURE (after the sign-off) — a \
         letterhead-only match would hide a dropped signature\n---\n{extracted}"
    );
    assert!(
        lower.contains("così") || lower.contains("però") || lower.contains("città"),
        "banded: grave-accented-lowercase body word missing\n---\n{extracted}"
    );

    let normalized_len = normalize_like_validator(&extracted).len();
    assert!(
        normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
        "banded accented-Latin: only {normalized_len} normalized chars extracted — \
         the real validator's no_extractable_text gate would block this export"
    );
}

/// The Navy letter layout must extract like its siblings. This is the test that
/// caught Cologne Navy's tracking: at the design brief's 0.14em the NAME came
/// back as "À LVA R O   È S P O S I T O" — unreadable to an ATS. The letterhead
/// carries the same tracked-caps treatment, so it needs the same guard.
#[test]
fn letter_navy_extracts_accented_latin_content() {
    let t = Template::get(TemplateId::CologneNavy);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_IT,
        &t,
        None,
        Some("Àlvaro Èsposito"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Navy,
            ats: false,
        },
    )
    .expect("navy accented-Latin render should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "navy accented-Latin must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract on navy accented-Latin output");
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    assert!(
        lower.contains("àlvaro") && lower.contains("èsposito"),
        "navy: accented name missing — capitals È/À did not survive extraction
---
{extracted}"
    );
    assert!(
        signature_block(&lower).contains("àlvaro èsposito"),
        "navy: accented name missing from the SIGNATURE (after the sign-off) — a          letterhead-only match would hide a dropped signature
---
{extracted}"
    );
    assert!(
        lower.contains("così") || lower.contains("però") || lower.contains("città"),
        "navy: grave-accented-lowercase body word missing
---
{extracted}"
    );

    let normalized_len = normalize_like_validator(&extracted).len();
    assert!(
        normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
        "navy accented-Latin: only {normalized_len} normalized chars extracted —          the real validator's no_extractable_text gate would block this export"
    );
}

// ── Phase 8: Sidebar + Monogram layouts ───────────────────────────────────────
//
// Both are decorated layouts, so each needs three separate guarantees:
//   (a) the words still come out, in reading order (the ATS harness);
//   (b) the decoration is DROPPED under `data.opts.ats` without losing a word;
//   (c) structural elements gate on `data.opts` (market conventions), never on
//       the layout id.

/// Layouts that must never emit a hyphenated line break into the PDF text
/// layer, i.e. the ones that set `hyphenate: false`. Now the full six-layout
/// roster — Classic/Refined/Banded/Navy picked up the flag in the same change
/// that extended this const (Sidebar and Monogram already had it from Phase
/// 8). Still an explicit roster rather than "every `LetterLayout`" so a future
/// layout that forgets the flag is a missing-test gap to notice, not a
/// silently-passing wildcard.
const NO_SOFT_HYPHEN_LAYOUTS: [LetterLayout; 6] = [
    LetterLayout::Classic,
    LetterLayout::Refined,
    LetterLayout::Banded,
    LetterLayout::Navy,
    LetterLayout::Sidebar,
    LetterLayout::Monogram,
];

/// Render + extract + whitespace-normalise + lowercase, the shape every letter
/// assertion below wants. Panics with the layout name so a failure says which.
///
/// Also the single choke point for the SOFT-HYPHEN guard: every extraction in
/// this file flows through here, so a layout in [`NO_SOFT_HYPHEN_LAYOUTS`]
/// cannot regress into hyphenated line breaks via any test, not just a
/// dedicated one. A U+00AD in the extracted text means the PDF really did break
/// the word — "microservices architecture" comes out as "architec­ture" and an
/// ATS tokenising on whitespace loses the keyword.
///
/// Only the SOFT hyphen is checked. The critic's correction to my first
/// measurement: of the three U+00AD-adjacent breaks I counted, only one is a
/// genuine soft-hyphen break — the others are HARD hyphens ("end-to-end"),
/// which Typst tags with `/ActualText` and are recoverable by a conforming
/// extractor. Asserting on the hard ones would be asserting on a non-defect.
fn letter_lower(layout: LetterLayout, fixture: &str, market: &str, ats: bool) -> String {
    let t = Template::get(TemplateId::SwissMinimal);
    let name = if market == "de" {
        "Max Müller"
    } else {
        "Jane Smith"
    };
    let lang = if market == "de" { "de" } else { "en" };
    let bytes = render_letter_pdf(
        fixture,
        &t,
        None,
        Some(name),
        LetterRender {
            market,
            lang,
            layout,
            ats,
        },
    )
    .unwrap_or_else(|e| panic!("{layout:?} (ats={ats}) render failed: {e}"));
    assert!(
        bytes.starts_with(b"%PDF"),
        "{layout:?} (ats={ats}) must start with %PDF"
    );
    let txt = pdf_extract::extract_text_from_mem(&bytes)
        .unwrap_or_else(|e| panic!("pdf-extract on {layout:?} (ats={ats}): {e}"))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    if NO_SOFT_HYPHEN_LAYOUTS.contains(&layout) {
        assert!(
            !txt.contains('\u{00AD}'),
            "{layout:?} (ats={ats}) emitted a soft hyphen — a hyphenated line break splits a \
             word in the PDF text layer, so an ATS tokenising on whitespace loses the keyword. \
             `hyphenate: false` must stay set on this layout.\n{txt}"
        );
    }
    txt
}

/// (S1/M1) Both new layouts render a valid US PDF whose text extracts in
/// letterhead → recipient → salutation → body → sign-off order. Sidebar's rail
/// is a MARGIN POSITION, not a second column of prose, so it must not scramble
/// this the way a real two-column letter would.
#[test]
fn new_letter_layouts_extract_in_reading_order() {
    for layout in [LetterLayout::Sidebar, LetterLayout::Monogram] {
        let lower = letter_lower(layout, LETTER_FIXTURE_US, "us", false);

        for needle in [
            "jane smith",
            "dear hiring manager",
            "distributed systems",
            "sincerely",
        ] {
            assert!(
                lower.contains(needle),
                "{layout:?}: {needle:?} missing from the extracted text:\n{lower}"
            );
        }

        let pos_head = lower.find("jane smith").expect("letterhead present");
        let pos_recipient = lower.find("acme corp").expect("recipient present");
        let pos_sal = lower.find("dear").expect("salutation present");
        let pos_body = lower.find("distributed").expect("body present");
        let pos_signoff = lower.find("sincerely").expect("sign-off present");
        assert!(
            pos_head < pos_recipient
                && pos_recipient < pos_sal
                && pos_sal < pos_body
                && pos_body < pos_signoff,
            "{layout:?}: reading order broken — head={pos_head} recipient={pos_recipient} \
             sal={pos_sal} body={pos_body} signoff={pos_signoff}\n{lower}"
        );
        assert!(
            lower.contains("123 main street"),
            "{layout:?}: recipient inside address missing:\n{lower}"
        );
    }
}

/// The roster [`NO_SOFT_HYPHEN_LAYOUTS`] just grew from two layouts to the
/// full six — Classic/Refined/Banded/Navy picked up `#set text(hyphenate:
/// false)` in the same change (Sidebar/Monogram already had it from Phase 8).
/// `letter_lower` is the SINGLE choke point that enforces the guard, but every
/// existing call site only ever passed Sidebar or Monogram — extending the
/// roster alone would have been a silent no-op for the other four without
/// this test actually routing them through it.
///
/// [`LETTER_FIXTURE_LONG_US`] carries "microservices architecture" — the
/// exact phrase cited in every layout's `hyphenate: false` comment — inside a
/// long, narrow-column paragraph, i.e. a fixture that WOULD hyphenate if the
/// flag were ever dropped. `letter_lower` asserts the U+00AD absence
/// internally for every layout in the roster; this test's own job is just to
/// call it for all six and confirm the phrase still extracts as one
/// unbroken pair of words.
#[test]
fn every_letter_layout_stays_hyphen_free_on_a_hyphenation_prone_word() {
    for layout in [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let lower = letter_lower(layout, LETTER_FIXTURE_LONG_US, "us", false);
        assert!(
            lower.contains("microservices architecture"),
            "{layout:?}: the hyphenation-prone phrase must still extract as one \
             unbroken pair of words:\n{lower}"
        );
    }
}

/// (S2/M2) Accented-Latin round trip — the guard that caught Cologne Navy's
/// 0.14em tracking, where the name extracted as "À LVA R O È S P O S I T O".
/// Both new layouts track their name, so both need it.
#[test]
fn new_letter_layouts_extract_accented_latin_content() {
    for layout in [LetterLayout::Sidebar, LetterLayout::Monogram] {
        let t = Template::get(TemplateId::SwissMinimal);
        let bytes = render_letter_pdf(
            LETTER_FIXTURE_IT,
            &t,
            None,
            Some("Àlvaro Èsposito"),
            LetterRender {
                market: "us",
                lang: "en",
                layout,
                ats: false,
            },
        )
        .unwrap_or_else(|e| panic!("{layout:?} accented-Latin render failed: {e}"));
        let extracted = pdf_extract::extract_text_from_mem(&bytes)
            .unwrap_or_else(|e| panic!("pdf-extract on {layout:?} accented-Latin: {e}"));
        let lower = extracted
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();

        assert!(
            lower.contains("àlvaro èsposito"),
            "{layout:?}: the accented name did not survive extraction as one word pair — \
             tracking too wide?\n---\n{extracted}"
        );
        assert!(
            signature_block(&lower).contains("àlvaro èsposito"),
            "{layout:?}: accented name missing from the SIGNATURE (after the sign-off); a \
             letterhead-only match would hide a dropped signature\n---\n{extracted}"
        );
        assert!(
            lower.contains("così") || lower.contains("però") || lower.contains("città"),
            "{layout:?}: grave-accented-lowercase body word missing\n---\n{extracted}"
        );

        let normalized_len = normalize_like_validator(&extracted).len();
        assert!(
            normalized_len >= NO_EXTRACTABLE_TEXT_THRESHOLD,
            "{layout:?} accented-Latin: only {normalized_len} normalized chars extracted — \
             the real validator's no_extractable_text gate would block this export"
        );
    }
}

/// (S3) ATS mode drops Sidebar's tinted rail. Detected the same way the Banded
/// band is: a page-1 fill that design mode has and the ATS render does not.
///
/// The load-bearing second half is that no WORD is lost — a "degradation" that
/// silently drops the contact line would pass a fill-only assertion.
#[test]
fn sidebar_rail_drops_under_ats_mode_without_losing_words() {
    let t = Template::get(TemplateId::SwissMinimal);
    let svg = |ats: bool| {
        render_letter_svg_pages(
            LETTER_FIXTURE_US,
            &t,
            None,
            Some("Jane Smith"),
            LetterRender {
                market: "us",
                lang: "en",
                layout: LetterLayout::Sidebar,
                ats,
            },
        )
        .expect("sidebar SVG render")
    };

    let design_fills = svg_fill_colors(&svg(false)[0]);
    let ats_fills = svg_fill_colors(&svg(true)[0]);
    let rail_only: Vec<&String> = design_fills.difference(&ats_fills).collect();
    assert!(
        !rail_only.is_empty(),
        "the Sidebar rail tint must be present in design mode and absent under ATS mode;\n\
         design={design_fills:?}\nats={ats_fills:?}"
    );

    // Same words, both modes — the rail is a position and a tint, not content.
    let design_txt = letter_lower(LetterLayout::Sidebar, LETTER_FIXTURE_US, "us", false);
    let ats_txt = letter_lower(LetterLayout::Sidebar, LETTER_FIXTURE_US, "us", true);
    for needle in [
        "jane smith",
        "jane@example.com",
        "acme corp",
        "dear hiring manager",
        "distributed systems",
        "sincerely",
    ] {
        assert!(
            ats_txt.contains(needle),
            "ATS-mode Sidebar dropped {needle:?} — degradation must lose decoration, not words:\n{ats_txt}"
        );
        assert!(
            design_txt.contains(needle),
            "design-mode Sidebar lost {needle:?}"
        );
    }
}

/// (S3b) Item-2 interaction: when the letterhead is genuinely EMPTY — no
/// candidate name (the fallback landed on a date, refused by
/// `is_letterhead_name`), no attached `ContactProfile`, no title — Sidebar's
/// rail must not paint a pale panel with nothing in it. `show-rail` collapses
/// to the same plain/symmetric treatment ATS mode uses, even though `ats`
/// itself is false here. Detected the same way ATS-mode rail-dropping is: a
/// page-1 fill a named render has that this one must not.
#[test]
fn sidebar_rail_collapses_when_the_letterhead_is_empty() {
    let t = Template::get(TemplateId::SwissMinimal);
    const NO_HEADER_DATE: &str =
        "12 March 2025\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    let render = |fixture: &str, meta_name: Option<&str>, ats: bool| {
        render_letter_svg_pages(
            fixture,
            &t,
            None,
            meta_name,
            LetterRender {
                market: "us",
                lang: "en",
                layout: LetterLayout::Sidebar,
                ats,
            },
        )
        .expect("sidebar SVG render")
    };

    // Precondition: a named letter DOES paint the rail tint, in both design
    // colour sets used below.
    let named_fills = svg_fill_colors(&render(LETTER_FIXTURE_US, Some("Jane Smith"), false)[0]);
    let ats_fills = svg_fill_colors(&render(LETTER_FIXTURE_US, Some("Jane Smith"), true)[0]);
    let rail_tint: Vec<&String> = named_fills.difference(&ats_fills).collect();
    assert!(
        !rail_tint.is_empty(),
        "precondition: a named Sidebar letter must paint a fill the ATS (no-rail) \
         render lacks; named={named_fills:?} ats={ats_fills:?}"
    );

    // The letterhead-less render (no candidate name, date-opening, no
    // ContactProfile) must NOT contain that rail-only fill either, even
    // though `ats` is false — i.e. it must not have painted an empty box.
    let empty_header_fills = svg_fill_colors(&render(NO_HEADER_DATE, None, false)[0]);
    for tint in &rail_tint {
        assert!(
            !empty_header_fills.contains(*tint),
            "a letterhead-less Sidebar page must not paint the rail tint with nothing in it; \
             found {tint:?} in {empty_header_fills:?}"
        );
    }

    // Suppression must lose only the fabricated name, not the words — the
    // date and salutation must still extract normally.
    let bytes = render_letter_pdf(
        NO_HEADER_DATE,
        &t,
        None,
        None,
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Sidebar,
            ats: false,
        },
    )
    .expect("letterhead-less sidebar PDF render");
    assert!(bytes.starts_with(b"%PDF"));
    let txt = pdf_extract::extract_text_from_mem(&bytes)
        .expect("extract text")
        .to_lowercase();
    assert!(
        txt.contains("12") && txt.contains("march") && txt.contains("2025"),
        "the date must still extract, just not as the letterhead name:\n{txt}"
    );
    assert!(
        txt.contains("dear hiring manager"),
        "the salutation must still render:\n{txt}"
    );
}

/// Item-2 sanity on the CLASSIC (undecorated) layout: a letterhead-less,
/// date-opening letter with no candidate name still renders a valid PDF and
/// still carries the date and salutation — `data.letterhead.name` is empty
/// (proven directly on the model by `letter.rs`'s
/// `letterhead_name_suppressed_for_date_opening_and_date_still_captured`);
/// this is the end-to-end confirmation that an empty name doesn't break
/// the plainest layout either.
#[test]
fn classic_letter_pdf_renders_with_a_suppressed_date_opening_name() {
    let t = Template::get(TemplateId::SwissMinimal);
    const NO_HEADER_DATE: &str =
        "12 March 2025\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    let bytes = render_letter_pdf(
        NO_HEADER_DATE,
        &t,
        None,
        None,
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("letterhead-less classic PDF render");
    assert!(bytes.starts_with(b"%PDF"));

    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("extract text")
        .to_lowercase();
    assert!(
        lower.contains("12") && lower.contains("march") && lower.contains("2025"),
        "the date must still extract:\n{lower}"
    );
    assert!(
        lower.contains("dear hiring manager"),
        "the salutation must still render:\n{lower}"
    );
}

/// (M3) ATS mode drops Monogram's initials device. Unlike a tint, the device is
/// real TEXT: in design mode extraction reads "js jane smith", two characters of
/// noise ahead of the candidate's actual name, which is exactly what ATS mode
/// exists to remove.
#[test]
fn monogram_device_drops_under_ats_mode_without_losing_words() {
    let design = letter_lower(LetterLayout::Monogram, LETTER_FIXTURE_US, "us", false);
    let ats = letter_lower(LetterLayout::Monogram, LETTER_FIXTURE_US, "us", true);

    assert!(
        design.contains("js jane smith"),
        "design-mode Monogram must render the initials device immediately before the name:\n{design}"
    );
    assert!(
        !ats.contains("js jane smith"),
        "ATS-mode Monogram must NOT emit the initials device — they extract as noise before \
         the name:\n{ats}"
    );
    for needle in [
        "jane smith",
        "jane@example.com",
        "dear hiring manager",
        "distributed systems",
        "sincerely",
    ] {
        assert!(
            ats.contains(needle),
            "ATS-mode Monogram dropped {needle:?} — degradation must lose the device, not words:\n{ats}"
        );
    }
}

/// (B4) The same discipline, applied to the layout that predates it: Banded's
/// accent band is decorative and must go under ATS mode too. Before ATS mode was
/// threaded into the letter path at all, the toggle silently did nothing to a
/// cover letter — a band the user had asked to remove was still exported.
#[test]
fn banded_band_drops_under_ats_mode() {
    let t = Template::get(TemplateId::SwissMinimal);
    let svg = |ats: bool| {
        render_letter_svg_pages(
            LETTER_FIXTURE_US,
            &t,
            None,
            Some("Jane Smith"),
            LetterRender {
                market: "us",
                lang: "en",
                layout: LetterLayout::Banded,
                ats,
            },
        )
        .expect("banded SVG render")
    };
    let design_fills = svg_fill_colors(&svg(false)[0]);
    let ats_fills = svg_fill_colors(&svg(true)[0]);
    assert!(
        !design_fills
            .difference(&ats_fills)
            .collect::<Vec<_>>()
            .is_empty(),
        "the Banded band fill must be present in design mode and absent under ATS mode;\n\
         design={design_fills:?}\nats={ats_fills:?}"
    );
}

/// (S4/M4) Structural elements gate on `data.opts`, NEVER on the layout id: the
/// subject line appears for a DE letter (DIN `Betreff`, `subject_line_used`) and
/// is absent for a US one whose market convention omits it — identical layout,
/// opposite outcome, decided entirely by the market.
#[test]
fn new_letter_layouts_gate_the_subject_line_on_market_opts_not_layout() {
    for layout in [LetterLayout::Sidebar, LetterLayout::Monogram] {
        let de = letter_lower(layout, LETTER_FIXTURE_DE, "de", false);
        // EXACTLY once, not merely present. `contains` is satisfied by one label
        // and by two, and two is what these layouts shipped: `parse_cover_letter`
        // publishes `data.subject` verbatim ("Betreff: Bewerbung …") and the
        // caption printed the label again on top of it, so a DE letter read
        // "BETREFF / Betreff: Bewerbung …". The four shipped layouts strip the
        // label first; these two did not, and a presence-only assertion could
        // not tell the difference.
        let betreff_count = de.matches("betreff").count();
        assert_eq!(
            betreff_count, 1,
            "{layout:?}: the DE market label must render exactly once, found \
             {betreff_count} — the caption is duplicating the label already carried \
             by data.subject:\n{de}"
        );
        // …and it must be the CAPTION that survives, not the raw prefix. The
        // count alone cannot tell those apart: stripping the label without a
        // caption, and a caption without stripping, both yield exactly one.
        // The colon is the tell — it only exists in the unstripped body.
        assert!(
            !de.contains("betreff: bewerbung"),
            "{layout:?}: the label was left on the subject body — `data.subject` must be \
             stripped before rendering, the way letter_refined/letter_navy and the DOCX \
             `strip_market_label` all do it:\n{de}"
        );
        assert!(
            de.contains("bewerbung als software engineer"),
            "{layout:?}: the DE subject body went missing:\n{de}"
        );
        assert!(
            de.contains("sehr geehr") && de.contains("freundlichen"),
            "{layout:?}: German salutation / sign-off missing:\n{de}"
        );
        assert!(
            de.contains("max") && de.contains("müller"),
            "{layout:?}: DE signature name missing:\n{de}"
        );

        let us = letter_lower(layout, LETTER_FIXTURE_US_SUBJECT, "us", false);
        assert!(
            !us.contains("px-2291"),
            "{layout:?}: the subject must NOT render when the market omits it \
             (subject_line_used=false):\n{us}"
        );
    }
}

/// (S6) The rail placement is `place`d with hand-computed offsets
/// (`dx: -(rail-w + rail-gutter - rail-pad)`), so it is measured against the
/// RENDERED page rather than trusted. A sign error or a stale constant would put
/// the letterhead off the left edge or on top of the body, and every text-only
/// assertion above would still pass.
///
/// Geometry under test (`letter_sidebar.typ`): rail 52 mm wide, text inset
/// 7 mm, gutter 10 mm, so the body column starts at 62 mm.
#[test]
fn sidebar_letterhead_is_measurably_inside_the_rail() {
    const MM: f64 = 72.0 / 25.4;
    let rail_pad = 7.0 * MM;
    let rail_text_right = (7.0 + 38.0) * MM;
    // Outer edge of the tinted panel. Used to bound the "rail zone" when
    // measuring type size: the body column's own left edge is NOT safe for
    // that, because a body glyph lands a float-hair below 62mm and would be
    // read as the rail's topmost line, collapsing the measurement to zero.
    let rail_right = 52.0 * MM;
    let body_left = 62.0 * MM;
    let ats_margin = 25.4 * MM;

    // A matrix, because the single "Jane Smith" case fitted the 38 mm block by
    // luck. Every row below is a real name/e-mail length against a template
    // whose `name_pt` is large enough to matter; the long ones overflowed the
    // rail into the gutter and the body column before shrink-to-fit existed.
    // "Alex Li" and "Jane Smith" stay in so the fitter cannot pass by simply
    // shrinking everything.
    // `fits_at_base` marks the rows short enough that the fitter must be a
    // NO-OP: they are the half that makes this test discriminating. Without
    // them, forcing `fit-size` to always return its 6pt floor passes the whole
    // matrix — every assertion here is a glyph POSITION, and shrinking
    // everything only ever produces less overflow, never more.
    let cases: &[(TemplateId, &str, &str, bool)] = &[
        (
            TemplateId::SwissMinimal,
            "Jane Smith",
            "jane@example.com",
            true,
        ),
        (TemplateId::Aria, "Alex Li", "alex@example.com", true),
        (
            TemplateId::Aria,
            "Àlvaro Papadopoulos",
            "alvaro.papadopoulos@example.com",
            false,
        ),
        (
            TemplateId::Cadence,
            "Wojciech Wojciechowski",
            "w.wojciechowski@example.com",
            false,
        ),
        (
            TemplateId::Deedy,
            "Anne Vandenberghe",
            "anne.vandenberghe@example.co.uk",
            false,
        ),
    ];

    for (template_id, name, email, fits_at_base) in cases {
        let t = Template::get(*template_id);
        let profile = crate::contact_profile::ContactProfile {
            full_name: Some((*name).to_string()),
            email: Some((*email).to_string()),
            ..Default::default()
        };
        let page = |ats: bool| {
            render_letter_svg_pages(
                LETTER_FIXTURE_US,
                &t,
                Some(&profile),
                Some(*name),
                LetterRender {
                    market: "us",
                    lang: "en",
                    layout: LetterLayout::Sidebar,
                    ats,
                },
            )
            .unwrap_or_else(|e| panic!("{template_id:?}/{name}: sidebar SVG render: {e}"))[0]
                .clone()
        };

        let design = glyph_positions(&page(false));
        assert!(
            !design.is_empty(),
            "{template_id:?}/{name}: design-mode page 1 rendered no glyphs"
        );

        let leftmost = design
            .iter()
            .map(|(x, _, _)| *x)
            .fold(f64::INFINITY, f64::min);
        assert!(
            (leftmost - rail_pad).abs() < 1.5,
            "{template_id:?}/{name}: the rail text must start exactly at the 7 mm rail              padding ({rail_pad:.1}pt); leftmost glyph is at {leftmost:.1}pt — the `place`              dx arithmetic is off"
        );

        // NOTHING may start between the end of the rail's 38 mm text block and
        // the body column. That span is the rail's right padding plus the 10 mm
        // gutter — it is where an unbreakable token too wide for the block ends
        // up, and it is the only visible symptom, since `place` neither wraps
        // nor clips.
        let overflow: Vec<f64> = design
            .iter()
            .map(|(x, _, _)| *x)
            .filter(|x| *x > rail_text_right + 0.5 && *x < body_left - 0.5)
            .collect();
        assert!(
            overflow.is_empty(),
            "{template_id:?}/{name}: glyphs at {overflow:?} sit past the rail text block              ({rail_text_right:.1}pt) and before the body column ({body_left:.1}pt) — the              letterhead is spilling out of the rail and across the gutter"
        );

        assert!(
            design.iter().any(|(x, _, _)| *x >= body_left - 0.5),
            "{template_id:?}/{name}: no glyph reaches the body column at {body_left:.1}pt —              the widened left margin is not being applied"
        );

        // ATS mode is the full-width single column: no rail, so no fitting, and
        // nothing may sit left of the ordinary margin.
        let ats = glyph_positions(&page(true));
        let ats_leftmost = ats.iter().map(|(x, _, _)| *x).fold(f64::INFINITY, f64::min);
        assert!(
            ats_leftmost >= ats_margin - 1.5,
            "{template_id:?}/{name}: ATS-mode Sidebar put a glyph at {ats_leftmost:.1}pt,              left of the {ats_margin:.1}pt margin — the rail placement is still active"
        );

        // The discriminating half. A name that already fits must render at the
        // rail's BASE size, so the fitter has to leave it alone.
        //
        // Expressed as a ratio against the same name in ATS mode — which always
        // renders at the template's full `name_pt` — so it calibrates itself
        // per template instead of hardcoding point sizes. The rail's base is
        // `name_pt - 4pt`, so the two extents must stand in exactly that ratio.
        // A fitter stuck at its 6pt floor collapses the ratio (6/20 = 0.30
        // against an expected 0.80) and this goes red, while every
        // position-only assertion above still passes it: shrinking everything
        // only ever produces LESS overflow, never more.
        if *fits_at_base {
            let base_ratio = (t.name_pt as f64 - 4.0) / t.name_pt as f64;
            let design_extent = top_line_extent(&design, rail_right);
            let ats_extent = top_line_extent(&ats, f64::INFINITY);
            assert!(
                design_extent > 1.0 && ats_extent > 1.0,
                "{template_id:?}/{name}: measured a degenerate name line \
                 (design={design_extent:.1}pt, ats={ats_extent:.1}pt)"
            );
            let actual = design_extent / ats_extent;
            assert!(
                (actual - base_ratio).abs() < 0.04,
                "{template_id:?}/{name}: the rail name renders at {actual:.3}x the ATS-mode \
                 name, expected {base_ratio:.3}x (= (name_pt - 4)/name_pt). A name this short \
                 already fits the 38 mm rail, so shrink-to-fit must be a NO-OP for it — this \
                 ratio is what separates 'fits what needs it' from 'shrinks everything'."
            );
        }
    }
}

/// (S7) Sidebar's contact line contains real `link()`s, and in design mode the
/// whole letterhead goes through `place()` with a NEGATIVE `dx` into the page
/// margin. Placed-and-negatively-offset content is the documented
/// annotation-loss shape (see the two-column header precedent above), and a
/// dropped `/Annots` entry is invisible to every text assertion — the words
/// still extract, they just stop being clickable.
///
/// Asserted in BOTH modes: design mode is the placed path, ATS mode is the
/// ordinary in-flow path, and only comparing the two shows that placement is
/// what would have cost the annotation.
#[test]
fn sidebar_contact_links_survive_the_placed_rail() {
    let t = Template::get(TemplateId::SwissMinimal);
    let profile = crate::contact_profile::ContactProfile {
        full_name: Some("Jane Smith".to_string()),
        email: Some("jane@example.com".to_string()),
        linkedin: Some("https://linkedin.com/in/janesmith".to_string()),
        ..Default::default()
    };

    for ats in [false, true] {
        let bytes = render_letter_pdf(
            LETTER_FIXTURE_US,
            &t,
            Some(&profile),
            Some("Jane Smith"),
            LetterRender {
                market: "us",
                lang: "en",
                layout: LetterLayout::Sidebar,
                ats,
            },
        )
        .unwrap_or_else(|e| panic!("sidebar (ats={ats}) render failed: {e}"));

        let uris = link_uris(&bytes);
        assert!(
            uris.iter().any(|u| u.contains("linkedin.com/in/janesmith")),
            "sidebar (ats={ats}): the LinkedIn link annotation was dropped — in design mode the \
             letterhead is `place`d into the margin with a negative dx, which is exactly the \
             shape that loses /Annots. found {uris:?}"
        );
        assert!(
            uris.iter().any(|u| u.contains("jane@example.com")),
            "sidebar (ats={ats}): the mailto link annotation was dropped; found {uris:?}"
        );
    }
}

/// (S5) Sidebar keeps its wide left margin and rail on EVERY page (a page-1-only
/// rail would leave page 2 with a 62 mm margin and nothing in it), and the body
/// still flows onto a second page — `place` must not have swallowed the content.
#[test]
fn sidebar_rail_repeats_on_every_page() {
    let t = Template::get(TemplateId::SwissMinimal);
    let pages = render_letter_svg_pages(
        LETTER_FIXTURE_LONG_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Sidebar,
            ats: false,
        },
    )
    .expect("sidebar long-letter SVG render");
    assert!(
        pages.len() >= 2,
        "the long fixture must reflow onto ≥2 pages to exercise the rail; got {}",
        pages.len()
    );

    let ats_pages = render_letter_svg_pages(
        LETTER_FIXTURE_LONG_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Sidebar,
            ats: true,
        },
    )
    .expect("sidebar long-letter ATS SVG render");

    // Compared against the ATS render's last page rather than against page 1,
    // so the assertion is "the rail is still here" and not merely "this page has
    // some fill" — every page has text fills.
    let design_last = svg_fill_colors(&pages[pages.len() - 1]);
    let ats_last = svg_fill_colors(&ats_pages[ats_pages.len() - 1]);
    assert!(
        design_last.difference(&ats_last).next().is_some(),
        "the Sidebar rail must be drawn on the LAST page too, not just page 1;\n\
         design last-page fills={design_last:?}\nats last-page fills={ats_last:?}"
    );
}

// (B3) The Banded accent band is drawn on page 1 ONLY. On a multi-page letter,
// Banded's page-1 SVG carries a filled band colour that (a) the no-band Classic
// layout lacks on its own page 1, and (b) is absent from Banded's later pages.
// typst-svg rasterises the `polygon` as a filled `<path>`, so we detect the band
// by its fill colour rather than a `<polygon>` tag.
#[test]
fn letter_banded_band_draws_on_page_one_only() {
    let t = Template::get(TemplateId::SwissMinimal);
    let banded = render_letter_svg_pages(
        LETTER_FIXTURE_LONG_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Banded,
            ats: false,
        },
    )
    .expect("banded long-letter SVG render");
    let classic = render_letter_svg_pages(
        LETTER_FIXTURE_LONG_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("classic long-letter SVG render");

    assert!(
        banded.len() >= 2,
        "the long fixture must reflow onto ≥2 pages to exercise page-1-only band; got {}",
        banded.len()
    );

    let banded_p1 = svg_fill_colors(&banded[0]);
    let classic_p1 = svg_fill_colors(&classic[0]);
    let banded_last = svg_fill_colors(&banded[banded.len() - 1]);

    // Fills that Banded adds on page 1 relative to the no-band Classic layout —
    // this set contains the decorative band tint.
    let band_only: Vec<&String> = banded_p1.difference(&classic_p1).collect();
    assert!(
        !band_only.is_empty(),
        "Banded page 1 must add a band fill the no-band Classic layout lacks;\n\
         banded p1={banded_p1:?}\nclassic p1={classic_p1:?}"
    );
    // …and that band fill must NOT repeat on later pages (band is page-1 only).
    assert!(
        band_only.iter().any(|f| !banded_last.contains(*f)),
        "the Banded band fill must be absent from later pages;\n\
         band-only p1 fills={band_only:?}\nbanded last-page fills={banded_last:?}"
    );
}

// (P1) Palette inheritance: a non-default résumé template (Regent, burgundy)
// yields different letter output than SwissMinimal for the same layout — the
// template's accent/palette reaches the letter via `style_from_template`.
#[test]
fn letter_layout_inherits_resume_template_accent() {
    let regent = Template::get(TemplateId::Regent);
    let swiss = Template::get(TemplateId::SwissMinimal);

    // Every decorated layout, not just the first two: the palette reaches
    // Sidebar's rail tint and Monogram's device fill through the same
    // `style_from_template` seam, and a hardcoded colour in either would show up
    // here as two identical renders.
    for layout in [
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ] {
        let a = render_letter_pdf(
            LETTER_FIXTURE_US,
            &regent,
            None,
            Some("Jane Smith"),
            LetterRender {
                market: "us",
                lang: "en",
                layout,
                ats: false,
            },
        )
        .expect("regent letter render");
        let b = render_letter_pdf(
            LETTER_FIXTURE_US,
            &swiss,
            None,
            Some("Jane Smith"),
            LetterRender {
                market: "us",
                lang: "en",
                layout,
                ats: false,
            },
        )
        .expect("swiss letter render");
        assert!(
            a != b,
            "{layout:?}: Regent and SwissMinimal must produce different output \
             (accent/palette must reach the letter)"
        );
    }
}

// (D1) EVERY layout dispatches to a distinct source: same data, different bytes.
// Classic remains a valid PDF (default-path regression).
//
// Pairwise over the whole roster rather than three hand-written comparisons —
// the hand-written form is how a new layout ends up silently rendering as
// another one (the DOCX side shipped exactly that bug: Navy rendered as Banded
// because a single boolean sent it down Banded's branch).
#[test]
fn letter_layouts_dispatch_to_distinct_sources() {
    let t = Template::get(TemplateId::SwissMinimal);
    let layouts = [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
        LetterLayout::Navy,
        LetterLayout::Sidebar,
        LetterLayout::Monogram,
    ];
    let rendered: Vec<(LetterLayout, Vec<u8>)> = layouts
        .iter()
        .map(|&layout| {
            let bytes = render_letter_pdf(
                LETTER_FIXTURE_US,
                &t,
                None,
                Some("Jane Smith"),
                LetterRender {
                    market: "us",
                    lang: "en",
                    layout,
                    ats: false,
                },
            )
            .unwrap_or_else(|e| panic!("{layout:?} render failed: {e}"));
            assert!(
                bytes.starts_with(b"%PDF"),
                "{layout:?} must produce a valid PDF"
            );
            (layout, bytes)
        })
        .collect();

    for (i, (a_id, a)) in rendered.iter().enumerate() {
        for (b_id, b) in rendered.iter().skip(i + 1) {
            assert!(
                a != b,
                "{a_id:?} and {b_id:?} must produce different output"
            );
        }
    }
}

// ── Phase 3a: Meridian, Throughline, Quanta — premium single-column ───────────
//
// For each template:
//   (a) Render produces a valid PDF.
//   (b) ATS harness: reading order + word boundaries + content present.
//   (c) Sample PDF written to target/ for human review (informational, always passes).
//
// For Throughline additionally:
//   (d) EXPERIENCE entries + bullets all survive extraction (timeline decoration
//       must not drop any text).

fn opts_p3a() -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats: false,
    }
}

// ── Meridian ──────────────────────────────────────────────────────────────────

#[test]
fn meridian_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("render_pdf(meridian) should succeed");
    assert!(!bytes.is_empty(), "Meridian PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Meridian output must start with %PDF"
    );
}

#[test]
fn meridian_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("render_pdf(meridian) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on meridian output");

    // Normalise whitespace — band layout can introduce line breaks inside
    // the header content (name, contact line placed in page background).
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // (c) Content present.
    assert!(
        lower.contains("jane doe"),
        "meridian ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "meridian ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "meridian ATS: bullet fragment 'distributed task scheduler' missing\n---\n{lower}"
    );

    // (b) Word boundaries.
    assert!(
        lower.contains("state university"),
        "meridian ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // (a) Reading order: summary → experience → education → skills.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("meridian ATS: '{h}' not found in extracted text"));
        assert!(
            pos >= last,
            "meridian ATS: '{h}' ({pos}) appeared before previous heading ({last})\n---\n{lower}"
        );
        last = pos;
    }
}

#[test]
fn meridian_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("render_pdf(meridian) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("meridian_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("meridian_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Meridian sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "meridian_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// ── Throughline ───────────────────────────────────────────────────────────────

#[test]
fn throughline_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) should succeed");
    assert!(!bytes.is_empty(), "Throughline PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Throughline output must start with %PDF"
    );
}

#[test]
fn throughline_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on throughline output");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // (c) Content present.
    assert!(
        lower.contains("jane doe"),
        "throughline ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "throughline ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "throughline ATS: bullet fragment missing\n---\n{lower}"
    );

    // (b) Word boundaries.
    assert!(
        lower.contains("state university"),
        "throughline ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // (a) Reading order.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("throughline ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "throughline ATS: '{h}' ({pos}) appeared before previous ({last})\n---\n{lower}"
        );
        last = pos;
    }
}

// (d) Throughline-specific: EXPERIENCE entries + bullets must all survive
// text extraction — the timeline decoration (nodes/spine) must not drop content.
#[test]
fn throughline_timeline_entries_and_bullets_survive_extraction() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) for timeline integrity");

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Entry titles from the fixture's EXPERIENCE section.
    for title in &["acme corp", "beta inc"] {
        assert!(
            lower.contains(title),
            "throughline timeline: entry title '{title}' missing — timeline may have dropped text\n---\n{lower}"
        );
    }

    // Bullet fragments from EXPERIENCE entries.
    assert!(
        lower.contains("distributed task scheduler"),
        "throughline timeline: bullet 'distributed task scheduler' missing\n---\n{lower}"
    );
    assert!(
        lower.contains("real-time data pipeline"),
        "throughline timeline: bullet 'real-time data pipeline' missing\n---\n{lower}"
    );
}

#[test]
fn throughline_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("throughline_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("throughline_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Throughline sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "throughline_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

// ── (Quanta removed) ──────────────────────────────────────────────────────────

// ── Phase 3b-i: Portrait + Lebenslauf — photo templates ──────────────────────
//
// Tests cover, per template:
//   (1) Render with fixture photo → valid PDF.
//   (2) Render without photo (no-photo fallback) → valid PDF.
//   (3) ATS harness: reading order + word boundaries + content.
//   (4) Sample PDFs written to target/ for human review.
//
// A fixture photo is generated in-test via the `image` crate (240×240 solid
// PNG → base64 data URL) — no committed binary needed.

use crate::export::typst_engine::resolve_photo;

/// Generate a 240×240 solid RGBA PNG as a base64 data URL, for use as a
/// fixture photo in the photo-template tests.
fn fixture_photo_data_url() -> String {
    use base64::Engine;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    // Gradient-ish: top-left warm orange, bottom-right deep blue-slate.
    let img = ImageBuffer::from_fn(240, 240, |x, y| {
        let r = (200u8).saturating_sub((x as u8).saturating_mul(170u8 / 240u8));
        let g = (100u8).saturating_add(y as u8 / 3);
        let b = (50u8).saturating_add(x as u8 / 3);
        Rgba([r, g, b, 255])
    });
    let dyn_img = DynamicImage::ImageRgba8(img);
    let mut buf = Vec::new();
    dyn_img
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .expect("fixture_photo: encode png");
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    format!("data:image/png;base64,{b64}")
}

fn opts_photo(ats: bool) -> RenderOpts {
    RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats,
    }
}

// ── Portrait ──────────────────────────────────────────────────────────────────

// (1a) Portrait with fixture photo → valid PDF.
#[test]
fn portrait_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    assert!(photo_png.is_some(), "fixture photo must resolve");

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(portrait) should succeed");

    assert!(!bytes.is_empty(), "Portrait PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Portrait output must start with %PDF"
    );
}

// (1b) Portrait without photo (no-photo fallback) → valid PDF.
#[test]
fn portrait_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(portrait, no-photo) should succeed");

    assert!(!bytes.is_empty(), "Portrait no-photo PDF must not be empty");
    assert!(bytes.starts_with(b"%PDF"));
}

// (1b-multibyte) Portrait's no-photo monogram fallback slices the candidate's
// first name to build initials. A byte-offset `.slice(0, 1)` panics Typst
// whenever the first character is multi-byte in UTF-8 (plausible DACH/EU
// names) — this pins the grapheme-safe fix (`.clusters().first()`).
#[test]
fn portrait_no_photo_monogram_is_grapheme_safe_for_multibyte_names() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let text = "Über Ödegaard\nuber@example.com\n\nSUMMARY\nEngineer.\n";
    let model = model_from_resume_text(text);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("portrait no-photo render with a multi-byte first character must not panic");
    assert!(bytes.starts_with(b"%PDF"));
}

// (1c) Portrait ATS mode → valid PDF with linear reading order.
#[test]
fn portrait_ats_harness() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(portrait, ats) should succeed");

    assert!(bytes.starts_with(b"%PDF"));

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on portrait ATS output");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Content present.
    assert!(
        lower.contains("jane doe"),
        "portrait ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "portrait ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "portrait ATS: bullet fragment missing\n---\n{lower}"
    );

    // Word boundaries.
    assert!(
        lower.contains("state university"),
        "portrait ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // Reading order.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("portrait ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "portrait ATS: '{h}' ({pos}) before previous ({last})"
        );
        last = pos;
    }
}

// (1d) Write Portrait sample PDFs to target/ for human review (with and without photo).
#[test]
fn portrait_write_sample_pdfs_for_review() {
    use crate::export::typst_engine::render_pdf_with_photo;
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    let _ = fs::create_dir_all(&target);

    // With photo.
    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let bytes_with = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("portrait with photo");
    match fs::write(target.join("portrait_sample.pdf"), &bytes_with) {
        Ok(()) => eprintln!("Portrait (with photo) sample written to target/portrait_sample.pdf"),
        Err(e) => eprintln!("portrait_write: could not write portrait_sample.pdf: {e}"),
    }
    assert!(bytes_with.starts_with(b"%PDF"));

    // Without photo (no-photo fallback).
    let bytes_nophoto = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("portrait no-photo");
    match fs::write(target.join("portrait_nophoto_sample.pdf"), &bytes_nophoto) {
        Ok(()) => {
            eprintln!("Portrait (no-photo) sample written to target/portrait_nophoto_sample.pdf")
        }
        Err(e) => eprintln!("portrait_write: could not write portrait_nophoto_sample.pdf: {e}"),
    }
    assert!(bytes_nophoto.starts_with(b"%PDF"));
}

// ── Lebenslauf ────────────────────────────────────────────────────────────────

/// German Lebenslauf fixture — uses typical DACH names and section content.
const LEBENSLAUF_FIXTURE: &str = "\
Max Müller
max.mueller@example.de | https://linkedin.com/in/maxmueller

BERUFSERFAHRUNG
Senior Software Engineer | Musterfirma GmbH | 2020 – Heute
- Entwicklung einer hochverfügbaren Microservices-Architektur mit Kubernetes
- Einführung von CI/CD-Pipelines und Reduktion der Deployment-Zeit um 60 Prozent

Software Engineer | Tech AG | 2017 – 2020
- Aufbau einer Echtzeit-Datenplattform für zwei Millionen tägliche Nutzer
- Mentoring von drei Junior-Entwicklern im Bereich Rust und TypeScript

AUSBILDUNG
M.Sc. Informatik | Technische Universität Berlin | 2015 – 2017

KENNTNISSE
Rust, Go, TypeScript, Kubernetes, AWS, PostgreSQL, Kafka

SPRACHEN
Deutsch (Muttersprache), Englisch (fließend)
";

// (2a) Lebenslauf with fixture photo → valid PDF.
#[test]
fn lebenslauf_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    assert!(photo_png.is_some(), "fixture photo must resolve");

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(lebenslauf) should succeed");

    assert!(!bytes.is_empty(), "Lebenslauf PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Lebenslauf output must start with %PDF"
    );
}

// (2b) Lebenslauf without photo → valid PDF.
#[test]
fn lebenslauf_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(lebenslauf, no-photo) should succeed");

    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"%PDF"));
}

// (2c) Lebenslauf ATS harness.
#[test]
fn lebenslauf_ats_harness() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(lebenslauf, ats) should succeed");

    assert!(bytes.starts_with(b"%PDF"));

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on lebenslauf ATS output");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Content present.
    assert!(
        lower.contains("jane doe"),
        "lebenslauf ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "lebenslauf ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "lebenslauf ATS: bullet fragment missing\n---\n{lower}"
    );

    // Word boundaries.
    assert!(
        lower.contains("state university"),
        "lebenslauf ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // Reading order.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("lebenslauf ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "lebenslauf ATS: '{h}' ({pos}) before previous ({last})"
        );
        last = pos;
    }
}

// (2c-tier) ATS mode drops the Lebenslauf photo.
//
// Verifies `lebenslauf.typ`'s `#if not is-ats and has-photo` branch through the
// render path: with a real photo supplied, the non-ATS render embeds it (Typst
// emits the raster as an SVG `<image>` element) but the ATS render omits it. The
// SVG emit shares the exact world/data as the PDF path, so this is the cheapest
// reliable assertion — no PDF-internals parsing needed.
#[test]
fn lebenslauf_ats_mode_drops_photo() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");

    // Non-ATS + photo → the raster photo is embedded as an SVG <image>.
    let pages_shown = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png.clone(),
    )
    .expect("render_resume_svg_pages_with_photo(lebenslauf, non-ats) should succeed");
    assert!(
        pages_shown.join("").contains("<image"),
        "non-ATS Lebenslauf with a photo must embed it as an <image> element"
    );

    // ATS + the same photo → `#if not is-ats and has-photo` is false → no image.
    let pages_ats = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(true),
        Some(&t),
        photo_png,
    )
    .expect("render_resume_svg_pages_with_photo(lebenslauf, ats) should succeed");
    assert!(
        !pages_ats.join("").contains("<image"),
        "ATS-mode Lebenslauf must drop the photo (no <image> element)"
    );
}

// (2d) Write Lebenslauf sample PDFs to target/ for human review.
#[test]
fn lebenslauf_write_sample_pdfs_for_review() {
    use crate::export::typst_engine::render_pdf_with_photo;
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    let _ = fs::create_dir_all(&target);

    // With photo.
    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let bytes_with = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("lebenslauf with photo");
    match fs::write(target.join("lebenslauf_sample.pdf"), &bytes_with) {
        Ok(()) => {
            eprintln!("Lebenslauf (with photo) sample written to target/lebenslauf_sample.pdf")
        }
        Err(e) => eprintln!("lebenslauf_write: could not write lebenslauf_sample.pdf: {e}"),
    }
    assert!(bytes_with.starts_with(b"%PDF"));
}

// ── Aria / Saffron (PR4 design two-column photo templates) ────────────────────
//
// Both are photo-capable two-column templates rendered through bespoke `.typ`
// sources.  Per template we assert: valid PDF with + without a photo (fallback
// path), ATS mode drops the photo (SVG `<image>` assert like Lebenslauf), the
// document-accent override changes the output, `is_two_column` is true, a 2-page
// fixture keeps the sidebar band to page 1, and the per-template placement
// override lands the moved section in the main column.

/// Fixture with distinct EDUCATION + CERTIFICATIONS + SKILLS sections so the
/// per-template placement override can be asserted at the serialized-JSON level.
const PLACEMENT_FIXTURE: &str = "\
Jane Doe
jane@example.com | https://linkedin.com/in/janedoe

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Built a distributed task scheduler

EDUCATION
State University  2013 - 2017
BSc Computer Science

SKILLS
- Rust, Go, TypeScript

CERTIFICATIONS
- AWS Certified Solutions Architect
";

/// Serialized column placement (`"main"` / `"sidebar"`) for the section with the
/// given canonical `kind`, as produced by `prepare` for `template_id`. This is
/// the single substrate that both the PDF and DOCX two-column splits consume.
fn placement_of(template_id: TemplateId, kind: &str) -> String {
    use super::render::{prepare, PreparedRender};
    let model = model_from_resume_text(PLACEMENT_FIXTURE);
    let t = Template::get(template_id);
    let source = TypstTemplate::from_template(&t).source_with_scale();
    let PreparedRender { data_json, .. } =
        prepare(&model, &source, &opts_a4(), Some(&t)).expect("prepare should succeed");
    let v: serde_json::Value =
        serde_json::from_slice(&data_json).expect("data.json must be valid JSON");
    let sections = v["sections"].as_array().expect("sections array");
    let sec = sections
        .iter()
        .find(|s| s["kind"] == kind)
        .unwrap_or_else(|| panic!("section kind {kind:?} not found in {sections:?}"));
    sec["placement"]
        .as_str()
        .expect("placement string")
        .to_string()
}

// ── Aria ────────────────────────────────────────────────────────────────────────

#[test]
fn aria_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(aria) should succeed");
    assert!(!bytes.is_empty(), "Aria PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Aria output must start with %PDF"
    );
}

#[test]
fn aria_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(aria, no-photo) should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Aria no-photo must start with %PDF"
    );
}

#[test]
fn aria_ats_mode_drops_photo() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");

    // Non-ATS + photo → embedded as an SVG <image>.
    let shown = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        photo_png.clone(),
    )
    .expect("aria non-ats svg");
    assert!(
        shown.join("").contains("<image"),
        "non-ATS Aria with a photo must embed it as an <image> element"
    );

    // ATS + same photo → linear, no image.
    let ats = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(true),
        Some(&t),
        photo_png,
    )
    .expect("aria ats svg");
    assert!(
        !ats.join("").contains("<image"),
        "ATS-mode Aria must drop the photo (no <image> element)"
    );
}

#[test]
fn aria_ats_mode_linearizes_reading_order() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let mut model = model_from_resume_text(PLACEMENT_FIXTURE);
    // Export path linearizes for ATS; replicate it here for the reading-order check.
    crate::model::transform::linearize(&mut model);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("aria ats pdf");
    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    let lower: String = extracted
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    // ATS linearization (`model::transform::linearize`) reorders `data.sections`
    // to the fixed canonical ATS reading order (Summary, Experience, Skills,
    // Projects, Education, Certifications, Languages, Awards, Publications) — it
    // ignores column `placement` entirely, which only shapes the two-column
    // VISUAL layout and never applies in ATS mode. So the Aria placement override
    // (Education → main column) has no bearing here: the expected order is the
    // canonical ATS order, not the placement-projected one.
    let exp = lower.find("experience").expect("experience present");
    let skl = lower.find("skills").expect("skills present");
    let edu = lower.find("education").expect("education present");
    let cert = lower
        .find("certifications")
        .expect("certifications present");
    assert!(
        exp < skl && skl < edu && edu < cert,
        "aria ATS reading order wrong (expected canonical ATS order: \
         experience < skills < education < certifications): {lower}"
    );
}

#[test]
fn aria_accent_override_changes_output() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);

    let base = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("aria base svg")
    .join("");

    let mut accented_opts = opts_photo(false);
    accented_opts.accent = Some("#FF00AA".to_string());
    let accented = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &accented_opts,
        Some(&t),
        None,
    )
    .expect("aria accent svg")
    .join("");

    assert_ne!(
        base, accented,
        "a document-accent override must change Aria's rendered output"
    );
    assert!(
        accented.to_lowercase().contains("ff00aa"),
        "the accent hex should appear in Aria's SVG fills"
    );
}

#[test]
fn aria_is_two_column() {
    assert!(crate::theme::is_two_column(TemplateId::Aria));
}

#[test]
fn aria_multipage_sidebar_renders_once() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(aria, multipage) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
    assert!(
        count_pdf_pages(&bytes) >= 2,
        "multi-page fixture must produce ≥2 pages"
    );
    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract")
        .to_lowercase();
    assert!(
        lower.contains("grafana"),
        "sidebar skill missing\n---\n{lower}"
    );
    assert_eq!(
        lower.matches("grafana").count(),
        1,
        "Aria sidebar must render once across pages\n---\n{lower}"
    );
}

#[test]
fn aria_moves_education_to_main_column() {
    assert_eq!(
        placement_of(TemplateId::Aria, "education"),
        "main",
        "Aria: Education must be placed in the main column"
    );
    // The rest of the sidebar set is unchanged for Aria.
    assert_eq!(placement_of(TemplateId::Aria, "skills"), "sidebar");
    assert_eq!(placement_of(TemplateId::Aria, "certifications"), "sidebar");
}

// ── Saffron ─────────────────────────────────────────────────────────────────────

#[test]
fn saffron_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(saffron) should succeed");
    assert!(!bytes.is_empty(), "Saffron PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Saffron output must start with %PDF"
    );
}

#[test]
fn saffron_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(saffron, no-photo) should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Saffron no-photo must start with %PDF"
    );
}

// Saffron's no-photo monogram fallback shares Portrait's slicing logic (copied
// pattern) — same grapheme-safety pin as
// `portrait_no_photo_monogram_is_grapheme_safe_for_multibyte_names`.
#[test]
fn saffron_no_photo_monogram_is_grapheme_safe_for_multibyte_names() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let text = "Über Ödegaard\nuber@example.com\n\nSUMMARY\nEngineer.\n";
    let model = model_from_resume_text(text);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("saffron no-photo render with a multi-byte first character must not panic");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn saffron_ats_mode_drops_photo() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");

    let shown = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        photo_png.clone(),
    )
    .expect("saffron non-ats svg");
    assert!(
        shown.join("").contains("<image"),
        "non-ATS Saffron with a photo must embed it as an <image> element"
    );

    let ats = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(true),
        Some(&t),
        photo_png,
    )
    .expect("saffron ats svg");
    assert!(
        !ats.join("").contains("<image"),
        "ATS-mode Saffron must drop the photo (no <image> element)"
    );
}

#[test]
fn saffron_ats_mode_linearizes_reading_order() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let mut model = model_from_resume_text(PLACEMENT_FIXTURE);
    crate::model::transform::linearize(&mut model);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("saffron ats pdf");
    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    let lower: String = extracted
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    // Same semantics as `aria_ats_mode_linearizes_reading_order`: ATS mode uses
    // the canonical ATS order (Summary, Experience, Skills, Projects, Education,
    // Certifications, …) regardless of Saffron's placement override
    // (Certifications → main column), which is visual-only and never applies in
    // ATS mode. Include `education` so the expected order isn't coincidentally
    // satisfied by only checking two of the four sections.
    let exp = lower.find("experience").expect("experience present");
    let skl = lower.find("skills").expect("skills present");
    let edu = lower.find("education").expect("education present");
    let cert = lower
        .find("certifications")
        .expect("certifications present");
    assert!(
        exp < skl && skl < edu && edu < cert,
        "saffron ATS reading order wrong (expected canonical ATS order: \
         experience < skills < education < certifications): {lower}"
    );
}

#[test]
fn saffron_accent_override_changes_output() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);

    let base = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("saffron base svg")
    .join("");

    let mut accented_opts = opts_photo(false);
    accented_opts.accent = Some("#FF00AA".to_string());
    let accented = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &accented_opts,
        Some(&t),
        None,
    )
    .expect("saffron accent svg")
    .join("");

    assert_ne!(
        base, accented,
        "a document-accent override must change Saffron's rendered output"
    );
    assert!(
        accented.to_lowercase().contains("ff00aa"),
        "the accent hex should appear in Saffron's SVG fills"
    );
}

#[test]
fn saffron_is_two_column() {
    assert!(crate::theme::is_two_column(TemplateId::Saffron));
}

#[test]
fn saffron_multipage_sidebar_renders_once() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(saffron, multipage) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
    assert!(
        count_pdf_pages(&bytes) >= 2,
        "multi-page fixture must produce ≥2 pages"
    );
    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract")
        .to_lowercase();
    assert!(
        lower.contains("grafana"),
        "sidebar skill missing\n---\n{lower}"
    );
    assert_eq!(
        lower.matches("grafana").count(),
        1,
        "Saffron sidebar must render once across pages\n---\n{lower}"
    );
}

#[test]
fn saffron_moves_certifications_to_main_column() {
    assert_eq!(
        placement_of(TemplateId::Saffron, "certifications"),
        "main",
        "Saffron: Certifications must be placed in the main column"
    );
    // Education stays in the sidebar for Saffron (unlike Aria).
    assert_eq!(placement_of(TemplateId::Saffron, "education"), "sidebar");
    assert_eq!(placement_of(TemplateId::Saffron, "skills"), "sidebar");
}

#[test]
fn portrait_placement_is_unchanged_by_the_refactor() {
    // Control: the default table (Portrait) keeps Education + Certifications in
    // the sidebar — the per-template id parameter must not shift it.
    assert_eq!(placement_of(TemplateId::Portrait, "education"), "sidebar");
    assert_eq!(
        placement_of(TemplateId::Portrait, "certifications"),
        "sidebar"
    );
}

// ── resolve_photo unit tests (already in photo.rs; re-exercised here for ──────
//    integration-layer confidence that the export module re-exports correctly)

#[test]
fn resolve_photo_valid_data_url_returns_png_bytes() {
    let data_url = fixture_photo_data_url();
    let result = resolve_photo(&data_url);
    assert!(
        result.is_some(),
        "resolve_photo must return Some for a valid PNG data URL"
    );
    let bytes = result.unwrap();
    assert!(
        bytes.starts_with(b"\x89PNG"),
        "resolve_photo output must be PNG; got {:?}",
        &bytes[..4.min(bytes.len())]
    );
}

#[test]
fn resolve_photo_oversized_returns_none() {
    let huge: String = "A".repeat(15 * 1024 * 1024);
    let data_url = format!("data:image/png;base64,{huge}");
    assert!(
        resolve_photo(&data_url).is_none(),
        "oversized data URL must return None"
    );
}

#[test]
fn resolve_photo_non_image_returns_none() {
    use base64::Engine;
    let garbage = b"not an image at all";
    let b64 = base64::engine::general_purpose::STANDARD.encode(garbage);
    let data_url = format!("data:image/png;base64,{b64}");
    assert!(
        resolve_photo(&data_url).is_none(),
        "non-image bytes must return None"
    );
}

#[test]
fn resolve_photo_bogus_path_returns_none() {
    assert!(
        resolve_photo("/nonexistent/path/photo.png").is_none(),
        "nonexistent path must return None"
    );
}

// ── Stray-Typst-code guard ────────────────────────────────────────────────────
//
// Renders a fixture through EVERY template (classic, swiss-minimal,
// academic, atelier, meridian, throughline, portrait, lebenslauf, letter) and
// asserts that the extracted PDF text contains NONE of the following
// case-sensitive substrings — these are Typst code tokens that would appear as
// literal printed text when a `#` prefix is accidentally omitted from a
// top-level call in markup context.
//
// Caught by this guard:
//   - `line(length` / `stroke:` / `block(above` / `block(below` / `grid(columns`
//   - `#let` / `pad(left` / `place(`
//
// This guard caught the `lebenslauf.typ` Bug 2 (missing `#` before `line` and
// `block` in the header section) and will catch any future regression across
// all templates.

const STRAY_TOKENS: &[&str] = &[
    "line(length",
    "stroke:",
    "block(above",
    "block(below",
    "grid(columns",
    "#let",
    "pad(left",
    "place(",
    "tracking:",
    "smallcaps(",
];

/// Render `bytes` through pdf-extract and assert no stray Typst tokens appear.
fn assert_no_stray_tokens(label: &str, bytes: &[u8]) {
    let extracted = pdf_extract::extract_text_from_mem(bytes)
        .unwrap_or_else(|e| panic!("stray-token guard: pdf-extract failed for {label}: {e}"));

    for token in STRAY_TOKENS {
        assert!(
            !extracted.contains(token),
            "stray-token guard [{label}]: found leaked Typst code token {token:?} \
             in extracted text — a `#` prefix is likely missing in the template source.\n\
             Extracted snippet (first 2000 chars):\n{:.2000}",
            extracted,
        );
    }
}

#[test]
fn stray_typst_code_guard_classic() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("stray-token guard: classic render failed");
    assert_no_stray_tokens("classic", &bytes);
}

#[test]
fn stray_typst_code_guard_swiss_minimal() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::SwissMinimal);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("stray-token guard: swiss-minimal render failed");
    assert_no_stray_tokens("swiss-minimal", &bytes);
}

#[test]
fn stray_typst_code_guard_academic() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Academic);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("stray-token guard: academic render failed");
    assert_no_stray_tokens("academic", &bytes);
}

#[test]
fn stray_typst_code_guard_atelier() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("stray-token guard: atelier render failed");
    assert_no_stray_tokens("atelier", &bytes);
}

#[test]
fn stray_typst_code_guard_meridian() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("stray-token guard: meridian render failed");
    assert_no_stray_tokens("meridian", &bytes);
}

#[test]
fn stray_typst_code_guard_throughline() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("stray-token guard: throughline render failed");
    assert_no_stray_tokens("throughline", &bytes);
}

#[test]
fn stray_typst_code_guard_portrait_with_photo() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("stray-token guard: portrait (with photo) render failed");
    assert_no_stray_tokens("portrait-with-photo", &bytes);
}

#[test]
fn stray_typst_code_guard_portrait_no_photo() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("stray-token guard: portrait (no-photo) render failed");
    assert_no_stray_tokens("portrait-no-photo", &bytes);
}

#[test]
fn stray_typst_code_guard_lebenslauf_with_photo() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("stray-token guard: lebenslauf (with photo) render failed");
    assert_no_stray_tokens("lebenslauf-with-photo", &bytes);
}

#[test]
fn stray_typst_code_guard_lebenslauf_no_photo() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("stray-token guard: lebenslauf (no-photo) render failed");
    assert_no_stray_tokens("lebenslauf-no-photo", &bytes);
}

#[test]
fn stray_typst_code_guard_letter() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("stray-token guard: letter render failed");
    assert_no_stray_tokens("letter", &bytes);
}

// ── PR3: heading_tracking / link_underline / rule_thickness knobs ──────────────
// (backward-compat proof)
//
// Every pre-PR3 template ships heading_tracking: 0.0, link_underline: false, and
// (for the ones whose rule is actually drawn) rule_thickness: 0.5 — the house
// default. By construction:
//   - `heading-run(...)` only emits `tracking: …` when `heading-tracking != 0.0`;
//     at 0.0 it falls through to the exact `text(size:, weight:, fill:, font:,
//     content)` call that existed before the knob (byte-for-byte, verified by
//     reading the branch).
//   - `render-runs(...)` only wraps a link in `underline(…)` when `link-underline`
//     is true; at `false` the branch reduces to the bare `styled` value — the
//     same `link(r.link, text(fill: c-accent, t))` call as before.
//   - the rule stroke resolves `(rule-thickness * 1pt) + c-rule`, and every
//     pre-PR3 ruled template ships `rule_thickness: 0.5` — `0.5 * 1pt == 0.5pt`,
//     the same literal stroke as before. (SwissMinimal ships `0.0`, but its
//     `section_style` is `BoldOnly`, so the ruled-bottom `line(...)` call — the
//     only reader of `rule-thickness` — never executes for it either way.)
//
// Rather than re-deriving that proof at test time by rendering the same code
// path twice and diffing the bytes (tautological — it would always pass), this
// renders ONCE per template and checks two INDEPENDENT anchors decoded from the
// compiled PDF: (a) no stray Typst source token leaked into the extracted text
// (`assert_no_stray_tokens`, which also now guards the new `tracking:` /
// `smallcaps(` syntax), and (b) the known section headings still appear intact,
// in the expected reading order.

#[test]
fn pr3_knob_defaults_leave_pre_pr3_headings_and_content_intact() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    for id in [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
    ] {
        let t = Template::get(id);
        assert_eq!(
            t.heading_tracking, 0.0,
            "{id:?}: heading_tracking must be 0.0"
        );
        assert!(!t.link_underline, "{id:?}: link_underline must be false");

        let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
            .unwrap_or_else(|e| panic!("{id:?}: render failed: {e:?}"));
        assert_no_stray_tokens(&format!("{id:?}-knob-defaults"), &bytes);

        let extracted = pdf_extract::extract_text_from_mem(&bytes)
            .unwrap_or_else(|e| panic!("{id:?}: pdf-extract failed: {e}"));
        let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
        let lower = normalised.to_lowercase();

        assert!(
            lower.contains("jane doe"),
            "{id:?}: candidate name missing after knob threading\n---\n{lower}"
        );
        let order = ["summary", "experience", "education", "skills"];
        let mut last = 0usize;
        for h in &order {
            let pos = lower.find(h).unwrap_or_else(|| {
                panic!("{id:?}: heading '{h}' missing after knob threading\n---\n{lower}")
            });
            assert!(
                pos >= last,
                "{id:?}: '{h}' ({pos}) appeared before previous heading ({last})\n---\n{lower}"
            );
            last = pos;
        }
    }
}

// ── Cadence ───────────────────────────────────────────────────────────────────

#[test]
fn cadence_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Cadence);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(cadence) should succeed");
    assert!(!bytes.is_empty(), "Cadence PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Cadence output must start with %PDF"
    );
}

#[test]
fn cadence_accent_override_applies() {
    let base = Template::get(TemplateId::Cadence);
    let overridden = Template::get(TemplateId::Cadence).with_accent_override(Some("#00AA33"));
    assert_ne!(base.accent_color, overridden.accent_color);
    assert_eq!(overridden.accent_color, (0, 170, 51));
    assert_eq!(overridden.emphasis_color, (0, 170, 51));
}

#[test]
fn cadence_tracking_and_underline_change_the_rendered_svg() {
    // Cadence sets heading_tracking 0.08 and link_underline true — prove the
    // knobs actually perturb the rendered SVG (not just config plumbing) by
    // diffing against a neutral (0.0 / false) variant of the same template.
    let model = model_from_resume_text(FIXTURE_RESUME);
    let cadence = Template::get(TemplateId::Cadence);
    assert_eq!(cadence.heading_tracking, 0.08);
    assert!(cadence.link_underline);

    let neutral = Template {
        heading_tracking: 0.0,
        link_underline: false,
        ..Template::get(TemplateId::Cadence)
    };

    let with_knobs = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&cadence),
    )
    .expect("cadence render should succeed");
    let without_knobs = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&neutral),
    )
    .expect("cadence-neutral render should succeed");

    assert_ne!(
        with_knobs, without_knobs,
        "heading_tracking/link_underline must visibly change the rendered SVG"
    );
}

#[test]
fn cadence_rule_thickness_changes_the_rendered_stroke() {
    // Cadence specs a 0.75pt section rule (vs the house 0.5pt default) — prove
    // `rule_thickness` is actually threaded into the rendered stroke width, not
    // just pinned config (the finding this test exists to close: the field was
    // previously dead in every renderer). Isolate it from the other PR3 knobs by
    // holding heading_tracking/link_underline fixed and varying only the stroke.
    let model = model_from_resume_text(FIXTURE_RESUME);
    let cadence = Template::get(TemplateId::Cadence);
    assert_eq!(cadence.rule_thickness, 0.75);

    let half_pt_rule = Template {
        rule_thickness: 0.5,
        ..Template::get(TemplateId::Cadence)
    };

    let with_075 = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&cadence),
    )
    .expect("cadence (0.75pt rule) render should succeed");
    let with_05 = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&half_pt_rule),
    )
    .expect("cadence (0.5pt rule) render should succeed");

    assert_ne!(
        with_075, with_05,
        "rule_thickness must visibly change the rendered SVG stroke"
    );
}

// ── Regent ────────────────────────────────────────────────────────────────────

#[test]
fn regent_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Regent);
    let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
        .expect("render_pdf(regent) should succeed");
    assert!(!bytes.is_empty(), "Regent PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Regent output must start with %PDF"
    );
}

#[test]
fn regent_accent_override_applies() {
    let base = Template::get(TemplateId::Regent);
    let overridden = Template::get(TemplateId::Regent).with_accent_override(Some("#123456"));
    assert_ne!(base.accent_color, overridden.accent_color);
    assert_eq!(overridden.accent_color, (18, 52, 86));
    assert_eq!(overridden.emphasis_color, (18, 52, 86));
}

#[test]
fn regent_maps_to_serif_small_caps_burgundy_style() {
    use super::render::style_from_template;

    let regent = Template::get(TemplateId::Regent);
    let style = style_from_template(&regent);

    // Source Serif 4 throughout, burgundy accent, small-caps (not all-caps)
    // headings, light heading tracking, no link underline.
    assert_eq!(style.font_heading, "Source Serif 4");
    assert_eq!(style.font_name, "Source Serif 4");
    assert_eq!(style.font_body, "Source Serif 4");
    assert!(
        style.section_small_caps,
        "Regent headings must be small-caps"
    );
    assert!(
        !style.section_all_caps,
        "Regent headings must not be all-caps"
    );
    assert_eq!(style.c_accent, "#6E1E2B");
    assert!((style.heading_tracking - 0.04).abs() < f32::EPSILON);
    assert!(!style.link_underline);
    assert_eq!(style.rule_thickness, 0.5);

    // Exercise the actual render path too (not just the JsonStyle mapping) and
    // guard the new `smallcaps(…)` call site — the bundled Source Serif 4 TTF
    // has neither `smcp` nor `c2sc`, and Typst 0.15's `smallcaps` does not yet
    // synthesize small caps for fonts lacking those features, so this renders
    // headings at 0.85× size in their original case rather than visually
    // distinct small-caps glyphs today; `smallcaps(…)` still keeps the PDF text
    // layer's characters unmodified (extraction-safe) and is forward-compatible
    // with a future smcp-capable font swap.
    let model = model_from_resume_text(FIXTURE_RESUME);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_sc(),
        Some(&regent),
    )
    .expect("regent render_pdf should succeed");
    assert!(bytes.starts_with(b"%PDF"));
    assert_no_stray_tokens("regent-small-caps", &bytes);
}

// ── README showcase banner generator ─────────────────────────────────────────
//
// Renders all twelve templates, rasterises the first page of each at 2× DPI
// (144 px/pt), thumbnails each to 300 px wide, and composes a single wide
// row (1×12) — a banner-proportioned strip like the project hero — on a
// #F4F4F5 background with 20 px border-padding and 14 px gaps, writing the
// result to docs/assets/templates-showcase.png.
//
// As a side output it also writes one per-template preview SVG to
// apps/desktop/src/renderer/features/ai-generate/assets/template-previews/<id>.svg,
// which the AI-Generate option previews show in the result panel. SVG (vector)
// replaces the old PNGs — crisp at any zoom and a fraction of the bundle size.
//
// This test is `#[ignore]`d so it never runs in the normal CI suite.
// Run it explicitly with (the crate is a binary, so target the bin, not --lib):
//   cargo test --bin ajh-tauri -- --ignored generate_templates_showcase_banner
//
// No personal data — synthetic fixture only.  No text-caption rendering dep.

/// Full showcase fixture — richer than FIXTURE_RESUME so templates show premium
/// styling: summary paragraph, multi-entry experience with bullets, skills,
/// education, languages.  Synthetic identity (Alex Carter, example.com contacts).
const SHOWCASE_FIXTURE: &str = "\
Alex Carter
alex.carter@example.com | https://linkedin.com/in/alexcarter | https://alexcarter.dev

SUMMARY
Versatile engineering leader with ten years building high-performance distributed
systems across fintech, healthcare, and cloud infrastructure. Known for bridging
deep technical expertise with product intuition to ship reliable platforms at scale.

EXPERIENCE
Staff Engineer | Apex Technologies | 2021 – Present
- Designed a multi-region event-sourcing platform processing 800 k events per second
- Led architectural review programme adopted by forty backend teams company-wide
- Reduced P99 API latency from 420 ms to 18 ms through adaptive connection pooling
- Mentored six engineers to senior level; two subsequently promoted to staff

Senior Engineer | Meridian Cloud | 2018 – 2021
- Built a zero-downtime schema-migration pipeline managing a 12 TB customer dataset
- Delivered the real-time collaboration layer used by 350 k daily active users
- Cut infrastructure spend by 38 percent via spot-instance scheduling and auto-scaling
- Shipped an internal observability platform reducing mean time-to-resolve by 70 percent

Software Engineer | Cobalt Labs | 2015 – 2018
- Implemented end-to-end encryption for all user-generated content at rest and in transit
- Rebuilt the search-indexing pipeline; ingestion lag dropped from six minutes to nine seconds
- Contributed core modules to four open-source libraries with a combined 12 k GitHub stars

PROJECTS
Distributed Rate Limiter | Open Source | 2022
- Redis-backed token-bucket rate limiter with sub-millisecond overhead per request
- Published on crates.io; adopted by twenty organisations within four months of launch

EDUCATION
M.Sc. Computer Science | Westbrook University | 2013 – 2015
B.Sc. Software Engineering | Coastal College | 2009 – 2013

SKILLS
Rust, Go, TypeScript, Python, Kubernetes, AWS, GCP, Kafka, PostgreSQL, Redis, Terraform

LANGUAGES
English (native), Spanish (professional), German (conversational)

CERTIFICATIONS
AWS Solutions Architect Professional
Certified Kubernetes Administrator
";

#[test]
#[ignore]
fn generate_templates_showcase_banner() {
    use image::{DynamicImage, GenericImage, ImageBuffer, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;
    use std::path::Path;
    use typst_layout::PagedDocument;
    use typst_render::{render as typst_rasterise, RenderOptions};

    use super::engine::TypstTemplate;
    use super::render::{prepare, prepare_with_photo, PreparedRender};
    use super::world::ResumeWorld;
    use crate::export::templates::Template;
    use crate::export::types::TemplateId;
    use crate::locale::PageGeometry;
    use crate::model::adapter::model_from_resume_text;

    // ── Layout constants ──────────────────────────────────────────────────────

    /// Pixels per Typst point at "2×" / 144 dpi.
    /// One Typst point = 1/72 inch → 144 dpi = 2.0 px/pt.
    const PIXEL_PER_PT: f32 = 2.0;

    /// Each thumbnail is scaled to exactly this width (px); height is derived
    /// from the original A4 aspect ratio.
    const CELL_W: u32 = 300;

    // Layout: a single wide row — one column per template × 1 row (banner
    // proportions). `ROWS` is 1, so every template must fit on that row; a
    // template landing at row-index >= 1 would write pixels beyond `canvas_h`
    // (an out-of-bounds `put_pixel` panic below). One row keeps the grid math
    // trivial (`col = idx % cols`, `row = idx / cols = 0`).
    //
    // Derived, not a literal: a hardcoded 12 here made the canvas one cell too
    // narrow the moment a 13th template landed, and the composition panicked
    // with an out-of-bounds pixel write rather than saying so.
    let cols: u32 = canonical_template_ids().len() as u32;
    const ROWS: u32 = 1;

    /// Outer border padding (px) and gap between cells (px).
    const PADDING: u32 = 20;
    const GAP: u32 = 14;

    /// Thin 1 px border drawn around each cell (colour: #C8C8CA mid-grey).
    const BORDER: u32 = 1;
    const BORDER_R: u8 = 200;
    const BORDER_G: u8 = 200;
    const BORDER_B: u8 = 202;

    /// Background colour: #F4F4F5 (very light warm grey).
    const BG_R: u8 = 0xF4;
    const BG_G: u8 = 0xF4;
    const BG_B: u8 = 0xF5;

    // ── A4 page geometry for rendering ────────────────────────────────────────

    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats: false,
    };

    // ── Template list (must be exactly 12, matching the canonical TemplateId set) ──

    // (TemplateId, human label, kebab slug). The slug MUST match the renderer's
    // `TemplateId` wire ids so the per-template preview files line up with the UI.
    let templates: &[(TemplateId, &str, &str)] = &[
        (TemplateId::Classic, "Classic", "classic"),
        (TemplateId::SwissMinimal, "SwissMinimal", "swiss-minimal"),
        (TemplateId::Academic, "Academic", "academic"),
        (TemplateId::Atelier, "Atelier", "atelier"),
        (TemplateId::Meridian, "Meridian", "meridian"),
        (TemplateId::Throughline, "Throughline", "throughline"),
        (TemplateId::Portrait, "Portrait", "portrait"),
        (TemplateId::Lebenslauf, "Lebenslauf", "lebenslauf"),
        (TemplateId::Cadence, "Cadence", "cadence"),
        (TemplateId::Regent, "Regent", "regent"),
        (TemplateId::Aria, "Aria", "aria"),
        (TemplateId::Saffron, "Saffron", "saffron"),
        (TemplateId::CologneNavy, "CologneNavy", "cologne-navy"),
        (TemplateId::Jake, "Jake", "jake"),
        (TemplateId::Awesome, "Awesome", "awesome"),
        (TemplateId::Deedy, "Deedy", "deedy"),
    ];
    assert_eq!(
        templates.len(),
        canonical_template_ids().len(),
        "showcase must cover every canonical template"
    );

    // ── Helper: compile a World to a PagedDocument ────────────────────────────

    let compile_world = |world: &ResumeWorld| -> PagedDocument {
        let warned = typst::compile::<PagedDocument>(world);
        for w in &warned.warnings {
            eprintln!("showcase typst warning [{w:?}]");
        }
        warned.output.unwrap_or_else(|diags| {
            let msg: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
            panic!("showcase: typst compile error: {}", msg.join("; "));
        })
    };

    // ── Helper: Pixmap → RgbaImage ────────────────────────────────────────────
    //
    // `typst_render::render` returns a `tiny_skia::Pixmap` whose `.data()`
    // is a flat &[u8] in premultiplied RGBA byte order.  Resume templates
    // render on a white background so virtually all pixels are fully opaque
    // (alpha = 255), meaning premultiplied == straight for those pixels.
    // For the handful of anti-aliased edge pixels the visual difference is
    // imperceptible at 420 px thumbnail width, so we copy the raw bytes
    // directly without the overhead of a per-pixel un-premultiply pass.
    // This also avoids a direct `tiny_skia` dev-dependency.

    let pixmap_to_rgba = |pxw: u32, pxh: u32, raw: Vec<u8>| -> RgbaImage {
        RgbaImage::from_raw(pxw, pxh, raw).expect("showcase: pixmap_to_rgba: buffer size mismatch")
    };

    // ── Render + rasterise each template ─────────────────────────────────────

    let model = model_from_resume_text(SHOWCASE_FIXTURE);

    // A4 at 2 px/pt → height of one cell thumbnail.
    // A4: 210 mm wide × 297 mm tall. Typst uses 1pt = 0.352778 mm,
    // so 210 mm = ~595.28 pt → 595.28 * 2 ≈ 1190 px wide before thumbnail.
    // After thumbnail to CELL_W=300: height = 300 * (297/210) ≈ 424 px.
    let a4_aspect = 297.0_f32 / 210.0_f32;
    let cell_h = (CELL_W as f32 * a4_aspect).round() as u32;

    // Per-template preview SVGs for the AI-Generate option previews. Written into
    // the renderer's feature assets (the UI imports them via a Vite glob).
    let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/renderer/features/ai-generate/assets/template-previews");
    std::fs::create_dir_all(&preview_dir)
        .unwrap_or_else(|e| panic!("showcase: create_dir_all template-previews: {e}"));

    let mut thumbnails: Vec<RgbaImage> = Vec::with_capacity(12);

    for (id, label, slug) in templates {
        eprintln!("showcase: rendering {label}...");

        let t = Template::get(*id);
        let typst_tmpl = TypstTemplate::from_template(&t);
        let source = typst_tmpl.source_with_scale();

        // Photo templates (Portrait, Lebenslauf, Aria, Saffron) take the photo-
        // capable prepare path but render their no-photo fallback so the showcase
        // generator has no binary dependency.
        let has_photo = matches!(
            id,
            TemplateId::Portrait | TemplateId::Lebenslauf | TemplateId::Aria | TemplateId::Saffron
        );

        let PreparedRender {
            source: compiled_source,
            data_json,
        } = if has_photo {
            prepare_with_photo(&model, &source, &opts, Some(&t), false)
                .unwrap_or_else(|e| panic!("showcase: prepare_with_photo({label}) failed: {e}"))
        } else {
            prepare(&model, &source, &opts, Some(&t))
                .unwrap_or_else(|e| panic!("showcase: prepare({label}) failed: {e}"))
        };

        let world = ResumeWorld::with_data(&compiled_source, Some(data_json));
        let document = compile_world(&world);

        assert!(
            !document.pages().is_empty(),
            "showcase: {label} produced zero pages"
        );

        // `render` gained an options parameter in typst 0.15; `pixel_per_pt`
        // moved onto `RenderOptions` (its default is already 2.0 = this scale).
        let render_opts = RenderOptions {
            pixel_per_pt: typst::utils::Scalar::new(f64::from(PIXEL_PER_PT)),
            render_bleed: false,
        };
        let pixmap = typst_rasterise(&document.pages()[0], &render_opts);
        let (pxw, pxh) = (pixmap.width(), pixmap.height());
        let raw = pixmap.data().to_vec();
        let rgba = pixmap_to_rgba(pxw, pxh, raw);

        // Per-template preview SVG (vector page-1 export) for the UI picker —
        // crisp at any zoom, a fraction of the old PNG's size, and self-contained
        // (Typst exports glyphs as paths, so there is no font dependency at display time).
        let svg: String = typst_svg::svg(&document.pages()[0], &typst_svg::SvgOptions::default());
        assert!(
            svg.contains("<svg"),
            "showcase: {label} preview SVG missing <svg root element"
        );
        let preview_path = preview_dir.join(format!("{slug}.svg"));
        std::fs::write(&preview_path, svg.as_bytes())
            .unwrap_or_else(|e| panic!("showcase: write preview {}: {e}", preview_path.display()));

        // Thumbnail to CELL_W × cell_h.
        let thumb = DynamicImage::ImageRgba8(rgba)
            .thumbnail(CELL_W, cell_h)
            .to_rgba8();

        let (tw_cur, th_cur) = (thumb.width(), thumb.height());
        thumbnails.push(thumb);
        eprintln!("  → thumbnail {tw_cur}×{th_cur}");
    }

    assert_eq!(
        thumbnails.len(),
        canonical_template_ids().len(),
        "must have one thumbnail per canonical template"
    );

    // ── Compose single wide row (1×10) ────────────────────────────────────────

    // Use the actual thumbnail dimensions (thumbnail() preserves aspect, so
    // width should be CELL_W and height close to cell_h).
    let tw = thumbnails[0].width();
    let th = thumbnails[0].height();

    // Canvas size:
    //   width  = PADDING + COLS*(BORDER + tw + BORDER) + (COLS-1)*GAP + PADDING
    //   height = PADDING + ROWS*(BORDER + th + BORDER) + (ROWS-1)*GAP + PADDING
    let canvas_w = PADDING + cols * (2 * BORDER + tw) + (cols - 1) * GAP + PADDING;
    let canvas_h = PADDING + ROWS * (2 * BORDER + th) + (ROWS - 1) * GAP + PADDING;

    let bg_pixel = Rgba([BG_R, BG_G, BG_B, 255u8]);
    let border_pixel = Rgba([BORDER_R, BORDER_G, BORDER_B, 255u8]);

    let mut canvas: RgbaImage = ImageBuffer::from_pixel(canvas_w, canvas_h, bg_pixel);

    for (idx, thumb) in thumbnails.iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;

        // Top-left of the border box for this cell.
        let bx = PADDING + col * (2 * BORDER + tw + GAP);
        let by = PADDING + row * (2 * BORDER + th + GAP);

        // Draw the 1 px border rectangle (top, bottom, left, right edges).
        for x in bx..bx + 2 * BORDER + tw {
            canvas.put_pixel(x, by, border_pixel);
            canvas.put_pixel(x, by + 2 * BORDER + th - 1, border_pixel);
        }
        for y in by..by + 2 * BORDER + th {
            canvas.put_pixel(bx, y, border_pixel);
            canvas.put_pixel(bx + 2 * BORDER + tw - 1, y, border_pixel);
        }

        // Copy thumbnail pixels into the canvas (inside the border).
        let inner_x = bx + BORDER;
        let inner_y = by + BORDER;
        canvas
            .copy_from(thumb, inner_x, inner_y)
            .unwrap_or_else(|e| panic!("showcase: copy_from cell {idx}: {e}"));
    }

    // ── Write PNG ─────────────────────────────────────────────────────────────

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let out_dir = Path::new(manifest_dir).join("../../../docs/assets");

    std::fs::create_dir_all(&out_dir)
        .unwrap_or_else(|e| panic!("showcase: create_dir_all docs/assets: {e}"));

    let out_path = out_dir.join("templates-showcase.png");

    let mut png_buf: Vec<u8> = Vec::new();
    DynamicImage::ImageRgba8(canvas.clone())
        .write_to(&mut Cursor::new(&mut png_buf), ImageFormat::Png)
        .unwrap_or_else(|e| panic!("showcase: PNG encode failed: {e}"));

    std::fs::write(&out_path, &png_buf)
        .unwrap_or_else(|e| panic!("showcase: write to {}: {e}", out_path.display()));

    // ── Verify: decode back and check dimensions ──────────────────────────────

    let verified = image::open(&out_path)
        .unwrap_or_else(|e| panic!("showcase: re-open PNG for verification failed: {e}"));

    assert_eq!(
        verified.width(),
        canvas_w,
        "showcase PNG width mismatch after write+re-open"
    );
    assert_eq!(
        verified.height(),
        canvas_h,
        "showcase PNG height mismatch after write+re-open"
    );

    let file_size = png_buf.len();
    assert!(
        file_size >= 80_000,
        "showcase PNG suspiciously small ({file_size} bytes); expected ≥80 KB"
    );
    assert!(
        file_size <= 4_000_000,
        "showcase PNG suspiciously large ({file_size} bytes); expected ≤4 MB"
    );

    eprintln!(
        "templates-showcase.png written: {}×{} px, {} bytes ({} KB)",
        canvas_w,
        canvas_h,
        file_size,
        file_size / 1024,
    );
    eprintln!("  path: {}", out_path.display());

    // ── Verify: all ten per-template previews exist and are non-trivial ───────

    for (_, label, slug) in templates {
        let p = preview_dir.join(format!("{slug}.svg"));
        let meta = std::fs::metadata(&p)
            .unwrap_or_else(|e| panic!("showcase: preview {slug}.svg missing ({label}): {e}"));
        assert!(
            meta.len() >= 1_000,
            "showcase: preview {slug}.svg suspiciously small ({} bytes)",
            meta.len()
        );
    }
    eprintln!(
        "template previews written: {} SVG → {}",
        templates.len(),
        preview_dir.display()
    );
}

/// Offline generator: one **cover-letter** style preview per résumé template.
///
/// `#[ignore]`d — an asset generator, not an assertion of behaviour. Run with:
///
/// ```text
/// cargo test --bin ajh-tauri -- --ignored generate_cover_template_previews
/// ```
///
/// This is the cover-letter analog of `generate_templates_showcase_banner`'s
/// per-template previews. For each of the same ten résumé templates it builds
/// the exact cover-letter Typst world that [`super::engine::render_letter_pdf`]
/// produces — `letter_style_from_template` derives the palette + fonts from the
/// résumé [`Template`], so the rendered letter *inherits that template's visual
/// style* — compiles page 1, and exports it to **SVG** (vector, no rasteriser,
/// no `image` crate, no thumbnailing). The ten `.svg` files feed the
/// AI-Generate cover-letter template picker (fetched lazily by the UI via a Vite
/// glob, mirroring the résumé `template-previews/` PNGs).
///
/// Offline hard-wall is respected: all `typst` / `typst_svg` types stay confined
/// to this test fn (same posture as the showcase test, which also imports typst
/// directly) — they never appear in production signatures. `typst-svg` is a
/// dev-dependency, never shipped in the binary.
#[test]
#[ignore]
fn generate_cover_template_previews() {
    use std::path::Path;
    use typst_layout::PagedDocument;

    use super::engine::{letter_scale_source, letter_source_for};
    use super::letter::{parse_cover_letter, style_from_template as letter_style_from_template};
    use super::world::ResumeWorld;

    // Same twelve templates as the showcase generator. Slugs MUST match the
    // renderer's `TemplateId` wire ids so the preview files line up with the UI.
    let templates: &[(TemplateId, &str, &str)] = &[
        (TemplateId::Classic, "Classic", "classic"),
        (TemplateId::SwissMinimal, "SwissMinimal", "swiss-minimal"),
        (TemplateId::Academic, "Academic", "academic"),
        (TemplateId::Atelier, "Atelier", "atelier"),
        (TemplateId::Meridian, "Meridian", "meridian"),
        (TemplateId::Throughline, "Throughline", "throughline"),
        (TemplateId::Portrait, "Portrait", "portrait"),
        (TemplateId::Lebenslauf, "Lebenslauf", "lebenslauf"),
        (TemplateId::Cadence, "Cadence", "cadence"),
        (TemplateId::Regent, "Regent", "regent"),
        (TemplateId::Aria, "Aria", "aria"),
        (TemplateId::Saffron, "Saffron", "saffron"),
        (TemplateId::CologneNavy, "CologneNavy", "cologne-navy"),
        (TemplateId::Jake, "Jake", "jake"),
        (TemplateId::Awesome, "Awesome", "awesome"),
        (TemplateId::Deedy, "Deedy", "deedy"),
    ];
    assert_eq!(
        templates.len(),
        canonical_template_ids().len(),
        "cover previews must cover every canonical template"
    );

    // Embedded letter Typst sources, reused verbatim from production so the
    // preview matches `render_letter_pdf`. The gallery preview renders the
    // Classic layout — these previews answer "what does a letter styled like
    // THIS RÉSUMÉ TEMPLATE look like", which is the `style_from_template` axis;
    // the layout axis is orthogonal and is chosen separately in the picker.
    // Routed through the production picker (`letter_source_for`) rather than a
    // fixed tuple, which had gone stale at three layouts.
    let scale_typ = letter_scale_source();
    let letter_typ = letter_source_for(LetterLayout::Classic);

    let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/renderer/features/ai-generate/assets/cover-template-previews");
    std::fs::create_dir_all(&preview_dir)
        .unwrap_or_else(|e| panic!("cover previews: create_dir_all cover-template-previews: {e}"));

    let mut written = 0usize;

    for (id, label, slug) in templates {
        eprintln!("cover previews: rendering {label}...");

        // Build the letter world exactly like `render_letter_pdf`, inline.
        let t = Template::get(*id);
        let style = letter_style_from_template(&t);
        let model = parse_cover_letter(
            LETTER_FIXTURE_US,
            None,
            Some("Jane Smith"),
            "intl",
            "en",
            style,
            // Design mode: the gallery advertises what the layout looks like.
            false,
        );
        let data_json = serde_json::to_vec(&model)
            .unwrap_or_else(|e| panic!("cover previews: JSON serialise ({label}) failed: {e}"));

        let source = format!(
            "// Auto-generated cover-letter entry — do not edit.\n\
             #let data = json(\"data.json\")\n\
             {scale_typ}\n\
             {letter_typ}"
        );

        let world = ResumeWorld::with_data(&source, Some(data_json));

        // Compile to a PagedDocument (same pattern as the showcase generator).
        let warned = typst::compile::<PagedDocument>(&world);
        for w in &warned.warnings {
            eprintln!("cover previews typst warning [{w:?}]");
        }
        let document = warned.output.unwrap_or_else(|diags| {
            let msg: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
            panic!(
                "cover previews: typst compile error ({label}): {}",
                msg.join("; ")
            );
        });

        assert!(
            !document.pages().is_empty(),
            "cover previews: {label} produced zero pages"
        );

        // Export page 1 to SVG (vector — no rasterisation, no thumbnail).
        let svg: String = typst_svg::svg(&document.pages()[0], &typst_svg::SvgOptions::default());
        assert!(
            !svg.is_empty(),
            "cover previews: {label} produced an empty SVG"
        );
        assert!(
            svg.contains("<svg"),
            "cover previews: {label} SVG missing <svg root element"
        );

        let preview_path = preview_dir.join(format!("{slug}.svg"));
        std::fs::write(&preview_path, svg.as_bytes())
            .unwrap_or_else(|e| panic!("cover previews: write {}: {e}", preview_path.display()));

        written += 1;
        eprintln!("  → {} ({} bytes)", preview_path.display(), svg.len());
    }

    // Derived, not a literal: this assertion read `written == 10` while the list
    // already held twelve, so the generator could not be run at all without
    // editing it first — and being `#[ignore]`d, CI never noticed. Deriving it
    // means adding a template can never leave it stale again.
    assert_eq!(
        written,
        templates.len(),
        "cover previews: expected one SVG per template"
    );

    // Verify each exists and is non-trivial.
    for (_, label, slug) in templates {
        let p = preview_dir.join(format!("{slug}.svg"));
        let meta = std::fs::metadata(&p)
            .unwrap_or_else(|e| panic!("cover previews: {slug}.svg missing ({label}): {e}"));
        assert!(
            meta.len() > 0,
            "cover previews: {slug}.svg is empty ({label})"
        );
    }
    eprintln!(
        "cover-letter template previews written: {} → {}",
        written,
        preview_dir.display()
    );
}
