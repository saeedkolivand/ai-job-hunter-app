//! Paragraph-building blocks for [`super::letter_layout`]'s non-Classic
//! cover-letter renderer, split out so the orchestrating loop reads as a
//! sequence of named zones rather than one long function.

use docx_rs::*;

use super::super::{
    docx_renderer::*,
    templates::Template,
    types::{FontFamily, GenerationMeta},
};
use super::letter_style::LetterDocxStyle;

/// Render the letterhead name paragraph plus its optional role-title
/// paragraph — the two paragraphs the caller appends together whenever the
/// opening line is a real name (`renders_name` in
/// [`super::letter_layout::generate_cover_letter_docx_layout`]).
/// The per-letter look [`render_layout_header_name`] reads, derived once by
/// the caller (so ATS mode drops every decoration in one place).
pub(super) struct HeaderLook<'a> {
    pub(super) colors: &'a DocxColors,
    pub(super) name_family: FontFamily,
    pub(super) body_family: FontFamily,
    pub(super) accent_hex: &'a str,
    pub(super) band_hex: &'a str,
    pub(super) show_device: bool,
    pub(super) show_band: bool,
}

pub(super) fn render_layout_header_name(
    name_text: &str,
    meta: Option<&GenerationMeta>,
    sty: &LetterDocxStyle,
    template: &Template,
    look: &HeaderLook<'_>,
) -> Vec<Paragraph> {
    let HeaderLook {
        colors,
        name_family,
        body_family,
        accent_hex,
        band_hex,
        show_device,
        show_band,
    } = *look;
    let mut out = Vec::new();

    // Banded: uppercase name — the same small-caps→uppercase precedent
    // `render_section_header` uses for the résumé DOCX path.
    let display_name = if !sty.uppercase_name {
        name_text.to_string()
    } else {
        name_text.to_uppercase()
    };
    let name_pt = template.name_pt + sty.name_pt_bonus;

    let mut name_para = Paragraph::new();
    // Monogram device approximation: the initials as a SHADED RUN at the head
    // of the name paragraph, from the same `letterhead_initials` the `.typ`
    // reads via `LetterHead.initials`, so the two formats can never disagree
    // about what it says. A run rather than its own paragraph because
    // `letter_monogram.typ` sets the square BESIDE the name — extraction must
    // read "JS Jane Smith", not "JS" on a line of its own.
    if show_device {
        let initials = crate::export::typst_engine::letterhead_initials(name_text);
        if !initials.is_empty() {
            // TWO runs. The gap between the device and the name has to be
            // OUTSIDE the shading: docx-rs always writes
            // `xml:space="preserve"`, so spaces inside the shaded run are
            // painted, and the tile visibly ran on past the initials — the
            // `.typ` square stops at the glyphs.
            name_para = name_para
                .add_run(
                    Run::new()
                        .add_text(&initials)
                        .size(pt_to_half_points(name_pt))
                        .bold()
                        .color(accent_hex)
                        .fonts(docx_run_fonts(name_family))
                        .shading(
                            Shading::new()
                                .shd_type(ShdType::Clear)
                                .color("auto")
                                .fill(band_hex),
                        ),
                )
                .add_run(
                    Run::new()
                        .add_text("  ")
                        .size(pt_to_half_points(name_pt))
                        .fonts(docx_run_fonts(name_family)),
                );
        }
    }
    name_para = name_para
        .add_run(
            Run::new()
                .add_text(&display_name)
                .size(pt_to_half_points(name_pt))
                .bold()
                .color(&colors.name)
                .fonts(docx_run_fonts(name_family)),
        )
        // 9pt (#28), matching `_scale.typ`'s `sp-name-below` — see the
        // identical fix in `generate_cover_letter_docx_classic`.
        .line_spacing(LineSpacing::new().after(pt_to_dxa(9.0) as u32));
    if sty.centred_letterhead {
        // `letter_navy.typ` centres name, title and contact, then rules UNDER
        // the lot. The rule therefore goes on the contact paragraph below,
        // not here; drawing it on the name would put a line between the name
        // and its own contact line.
        name_para = name_para.align(AlignmentType::Center);
    }
    if show_band {
        // Banded's band / Sidebar's rail approximation — see the module-level
        // doc comment. Navy and Monogram are deliberately excluded: neither
        // design has a tinted block behind the name.
        name_para.property = name_para.property.shading(
            Shading::new()
                .shd_type(ShdType::Clear)
                .color("auto")
                .fill(band_hex),
        );
    }
    out.push(name_para);

    // Role line right after the name (see approximation note in
    // `letter_layout`'s module doc re: `meta.job_title` vs the `.typ`'s
    // `signature_title`). Refined and Navy both render it; Banded does not.
    if sty.shows_title {
        if let Some(job_title) = meta.and_then(|m| m.job_title.as_deref()) {
            let mut title_run = Run::new()
                .add_text(if sty.title_emphasised {
                    job_title.to_uppercase()
                } else {
                    job_title.to_string()
                })
                .size(pt_to_half_points(template.body_pt))
                .color(if sty.title_emphasised {
                    accent_hex
                } else {
                    // Navy's `.typ` puts the role line in the muted date
                    // colour, not the accent, and does not track it.
                    &colors.date
                })
                .fonts(docx_run_fonts(body_family));
            if sty.title_emphasised {
                title_run = title_run.character_spacing(24);
            }
            let mut title_para = Paragraph::new()
                .add_run(title_run)
                .line_spacing(LineSpacing::new().after(40));
            if sty.centred_letterhead {
                title_para = title_para.align(AlignmentType::Center);
            }
            out.push(title_para);
        }
    }

    out
}

/// Render the subject/reference-line paragraphs (Betreff/Objet/Re/…),
/// before the salutation — the job-reference caption (Refined/Navy/Sidebar/
/// Monogram) or the bold accent-coloured line (Banded, which has no caption/
/// suppression logic of its own).
pub(super) fn render_layout_subject_lines(
    clean: &str,
    sty: &LetterDocxStyle,
    template: &Template,
    colors: &DocxColors,
    body_family: FontFamily,
    accent_hex: &str,
    subj_label: &str,
) -> Vec<Paragraph> {
    let mut out = Vec::new();

    if sty.shows_subject_caption {
        // The always-on JOB REFERENCE line: caption suppressed when the
        // subject already opens with the market's own label or "Re:" — same
        // content rule as `letter_refined.typ`'s `strip-subject-label` +
        // `has-own-label` check.
        let subj_body = super::letter_layout::strip_market_label(clean, subj_label);
        let subj_body_lower = subj_body.to_lowercase();
        let has_own_label = (!subj_label.is_empty()
            && subj_body_lower.starts_with(&subj_label.to_lowercase()))
            || subj_body_lower.starts_with("re:");

        if !has_own_label {
            let caption = if subj_label.is_empty() {
                "Subject".to_string()
            } else {
                subj_label.to_string()
            };
            out.push(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(caption.to_uppercase())
                            .size(pt_to_half_points((template.body_pt - 1.5).max(6.0)))
                            .bold()
                            .color(if sty.caption_uses_name_colour {
                                &colors.name
                            } else {
                                accent_hex
                            })
                            .character_spacing(24)
                            .fonts(docx_run_fonts(body_family)),
                    )
                    .line_spacing(LineSpacing::new().after(20)),
            );
        }
        out.push(
            Paragraph::new()
                .add_run(
                    Run::new()
                        .add_text(&subj_body)
                        .size(pt_to_half_points(template.body_pt))
                        .bold()
                        .color(&colors.body)
                        .fonts(docx_run_fonts(body_family)),
                )
                .line_spacing(LineSpacing::new().before(120).after(120)),
        );
    } else {
        // Banded: bold + accent color, unprocessed text — `.typ` has no
        // caption/suppression logic for this layout, only Refined does.
        out.push(
            Paragraph::new()
                .add_run(
                    Run::new()
                        .add_text(clean)
                        .size(pt_to_half_points(template.body_pt))
                        .bold()
                        .color(accent_hex)
                        .fonts(docx_run_fonts(body_family)),
                )
                .line_spacing(LineSpacing::new().before(120).after(120)),
        );
    }

    out
}
