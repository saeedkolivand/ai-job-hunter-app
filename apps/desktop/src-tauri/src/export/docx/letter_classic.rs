//! Cover-letter DOCX: the layout dispatcher plus the original (`Classic`)
//! renderer, unmodified since before the layout picker existed.

use anyhow::Result;
use docx_rs::*;

use super::super::{
    docx_renderer::*,
    parser::strip_md,
    templates::Template,
    types::{GenerationMeta, LetterLayout, LetterRender},
};
use super::{letter_layout::generate_cover_letter_docx_layout, page_size_dxa};

/// Dispatch to the requested [`LetterLayout`]'s DOCX renderer. `Classic` keeps
/// calling the original, unmodified renderer (`_classic`) so its output stays
/// byte-identical to the pre-layout-picker DOCX — the Refined/Banded arm is
/// entirely new code, never touched by a Classic request.
pub(super) fn generate_cover_letter_docx(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    contact: Option<&crate::contact_profile::ContactProfile>,
    req: LetterRender<'_>,
) -> Result<Docx> {
    match req.layout {
        LetterLayout::Classic => {
            generate_cover_letter_docx_classic(text, meta, template, contact, req.lang)
        }
        // Every non-Classic layout is an arrangement over the same model, and
        // DOCX cannot express any of them literally (no angled polygon, no
        // margin rail, no inline device box) — the shared layout renderer
        // degrades each to the same parser-safe DOCX structure, restyled from
        // its own `LetterDocxStyle` row.
        LetterLayout::Refined
        | LetterLayout::Banded
        | LetterLayout::Navy
        | LetterLayout::Sidebar
        | LetterLayout::Monogram => {
            generate_cover_letter_docx_layout(text, meta, template, contact, req)
        }
    }
}

/// The original cover-letter DOCX renderer (pre-PR5). Unmodified — this is the
/// `LetterLayout::Classic` path, kept byte-identical.
fn generate_cover_letter_docx_classic(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    contact: Option<&crate::contact_profile::ContactProfile>,
    lang: &str,
) -> Result<Docx> {
    let mut docx = Docx::new();

    // Named contact profile is the source of truth for the header contact line
    // (shared with the résumé), emitted as real hyperlinks. When present, the
    // scraped contact lines from the generated text are skipped.
    let profile_contact_md: Option<String> = contact
        .filter(|p| !p.is_effectively_empty())
        .map(|p| p.header_markdown(lang));

    let page_margin = PageMargin::new()
        .top(inch_to_dxa(1.0))
        .bottom(inch_to_dxa(1.0))
        .left(inch_to_dxa(template.margin_in + 0.15))
        .right(inch_to_dxa(template.margin_in + 0.15));

    let colors = setup_colors(template);
    let body_family = template.fonts.body_family;
    let name_family = template.fonts.name_family;

    let lines: Vec<&str> = text.lines().collect();
    let mut header_done = false;
    let mut in_body = false;
    // Tracks "have we processed the first non-blank line yet" — NOT the same
    // as `docx.document.children.is_empty()`, which the header block used to
    // rely on. That check silently broke once contact could be emitted
    // without a name: a contact-only header still adds a paragraph, so the
    // "first line" gate would have gone false starting on line 2 for the
    // WRONG reason.
    let mut header_line_seen = false;

    for raw_line in &lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            // Blank lines — DOCX flow handles spacing; skip blank para emission
            continue;
        }

        let clean = strip_md(trimmed);

        // Locale-aware: recognize salutations/sign-offs across every supported
        // market (was English/German only).
        let is_salutation = crate::locale::letter::is_salutation(&clean);
        let is_signoff = crate::locale::letter::is_signoff(&clean);
        let is_subject_line = crate::locale::letter::is_subject_line(&clean);

        let is_first_line = !header_line_seen;
        header_line_seen = true;

        // First line is name — but ONLY a real letterhead name, not a letter
        // that opens straight at the salutation, sign-off, or subject/
        // reference line (e.g. German "Betreff: …"), and not a date either
        // ("12 March 2025", the fourth opening kind) — shared with the PDF
        // parser's identical guard via `is_letterhead_name` (see its doc
        // comment) so the two formats can never disagree about which
        // openings are not names.
        if is_first_line && !header_done {
            let is_header_name_line = !is_salutation && !is_signoff && !is_subject_line;
            // Shared with the PDF parser via `resolve_letterhead_candidate`:
            // an empty-string `candidate_name: Some("")` must fall through to
            // `clean` exactly like `None` does. `meta...unwrap_or(&clean)`
            // alone does NOT do that — `Some("")` is not `None`, so the plain
            // `unwrap_or` chain returned `""` and suppressed a REAL name
            // sitting right there on the letter's own first line.
            let name_text = crate::export::typst_engine::resolve_letterhead_candidate(
                meta.and_then(|m| m.candidate_name.as_deref()),
                || clean.as_str(),
            );
            let renders_name =
                is_header_name_line && crate::export::typst_engine::is_letterhead_name(name_text);

            if renders_name {
                docx = docx.add_paragraph(
                    Paragraph::new()
                        .add_run(
                            Run::new()
                                .add_text(name_text)
                                .size(pt_to_half_points(template.name_pt))
                                .bold()
                                .color(&colors.name)
                                .fonts(docx_run_fonts(name_family)),
                        )
                        // 9pt (#28), matching `_scale.typ`'s `sp-name-below` —
                        // was 60 dxa (3pt), crammed against the contact line
                        // below.
                        .line_spacing(LineSpacing::new().after(pt_to_dxa(9.0) as u32)),
                );
            }

            // Contact renders INDEPENDENT of whether the name did — it comes
            // from a separately-attached `ContactProfile`, not from parsing
            // this line, exactly like the PDF parser's `contact_md` (built
            // unconditionally, before any line is classified). Nesting this
            // inside the name branch was a regression a review round caught:
            // a nameless, date/salutation-opening letter with a real profile
            // attached lost the user's contact info entirely, not just the
            // fabricated name — strictly worse than the pre-guard behaviour,
            // where a garbage name at least kept the contact line alive.
            // `render_contact_line` runs `split_urls`, so `[LinkedIn](url)`
            // markdown and bare emails become real hyperlinks.
            if let Some(md) = &profile_contact_md {
                docx = docx.add_paragraph(
                    super::super::docx_renderer::render_contact_line(md, template, &colors)
                        .line_spacing(LineSpacing::new().after(40)),
                );
            }

            if renders_name {
                continue;
            }
            // else: this line (a date, the salutation itself, a subject
            // line, or any other non-name opening) was NOT consumed as a
            // header — fall through so the block below (contact/address) or
            // a later branch (salutation/subject/body) classifies it
            // normally.
        }

        // Contact/address lines
        if !header_done
            && (clean.contains('@')
                || clean.contains('|')
                || clean.contains('·')
                || clean.chars().filter(|c| c.is_numeric()).count() > 5)
        {
            // Profile is the source of truth — drop the scraped contact line.
            if profile_contact_md.is_some() {
                continue;
            }
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_half_points(9.0))
                            .color(&colors.date)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().after(40)),
            );
            continue;
        }

        // Salutation
        if is_salutation {
            header_done = true;
            in_body = true;
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_half_points(template.body_pt))
                            .color(&colors.body)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().before(160).after(160)),
            );
            continue;
        }

        // Signoff
        if is_signoff {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_half_points(template.body_pt))
                            .color(&colors.body)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().before(240).after(480)),
            );
            continue;
        }

        // Subject line (Betreff/Objet/Oggetto/…) — bold, before the salutation.
        if !in_body && is_subject_line {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_half_points(template.body_pt))
                            .bold()
                            .color(&colors.body)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().before(120).after(120)),
            );
            continue;
        }

        // Addressee block (before salutation)
        if !in_body {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(&clean)
                            .size(pt_to_half_points(template.body_pt))
                            .color(&colors.date)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().after(40)),
            );
            continue;
        }

        // Body paragraphs — use proper spacing via pPr, no blank-paragraph spacers.
        // `trimmed`, NOT `clean`: `render_cover_letter_paragraph` runs its own
        // `parse_inline_md` to turn `**bold**` into a real bold run, but `clean`
        // has already had every `**` stripped by `strip_md` above — passing it
        // here fed `parse_inline_md` text with no markers left to find, so
        // `**microservices**` rendered as plain "microservices" with no bold
        // run at all.
        let para = render_cover_letter_paragraph(trimmed, template, &colors, body_family);
        docx = docx.add_paragraph(para);
    }

    let (page_w, page_h) = page_size_dxa();
    docx = docx.page_size(page_w, page_h).page_margin(page_margin);
    Ok(docx)
}
