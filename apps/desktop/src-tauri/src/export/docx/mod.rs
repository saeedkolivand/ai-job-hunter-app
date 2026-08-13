use anyhow::{Context, Result};
use docx_rs::*;

use super::{
    parser::strip_md,
    templates::Template,
    types::{DocumentType, ExportRequest, GenerationMeta, LetterLayout},
};
use crate::locale::LocaleProfile;

use super::docx_renderer::*;

/// Page size (in DOCX `dxa`) for the active locale. Defaults to the `en` profile
/// (A4), keeping DOCX on the same page geometry as the PDF backends. Set
/// explicitly rather than relying on the docx-rs default so the source of truth
/// is `LocaleProfile`/`PageGeometry`; per-request locale sizing arrives in a
/// later phase.
fn page_size_dxa() -> (u32, u32) {
    let geom = LocaleProfile::default().page_geometry();
    (mm_to_dxa(geom.width_mm), mm_to_dxa(geom.height_mm))
}

// ─── Cover letter DOCX ────────────────────────────────────────────────────────

/// Dispatch to the requested [`LetterLayout`]'s DOCX renderer. `Classic` keeps
/// calling the original, unmodified renderer (`_classic`) so its output stays
/// byte-identical to the pre-layout-picker DOCX — the Refined/Banded arm is
/// entirely new code, never touched by a Classic request.
#[allow(clippy::too_many_arguments)]
fn generate_cover_letter_docx(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    contact: Option<&crate::contact_profile::ContactProfile>,
    lang: &str,
    market: &str,
    layout: LetterLayout,
    ats: bool,
) -> Result<Docx> {
    match layout {
        LetterLayout::Classic => {
            generate_cover_letter_docx_classic(text, meta, template, contact, lang)
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
        | LetterLayout::Monogram => generate_cover_letter_docx_layout(
            text, meta, template, contact, lang, market, layout, ats,
        ),
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

    for raw_line in &lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            // Blank lines — DOCX flow handles spacing; skip blank para emission
            continue;
        }

        let clean = strip_md(trimmed);
        let _segments = super::parser::parse_inline_md(trimmed);

        // Locale-aware: recognize salutations/sign-offs across every supported
        // market (was English/German only).
        let is_salutation = crate::locale::letter::is_salutation(&clean);
        let is_signoff = crate::locale::letter::is_signoff(&clean);
        let is_subject_line = crate::locale::letter::is_subject_line(&clean);

        // First line is name — but ONLY a real letterhead name, not a letter that
        // opens straight at the salutation, sign-off, or subject/reference line
        // (e.g. German "Betreff: …"). Without this guard a letterhead-less
        // letter's "Dear …"/"Betreff: …" was consumed as the name (replaced with
        // the candidate name), the salutation/subject arm never ran, `in_body`
        // was never set, and the whole body rendered in the muted addressee style.
        if !header_done
            && docx.document.children.is_empty()
            && !is_salutation
            && !is_signoff
            && !is_subject_line
        {
            let name_text = meta
                .and_then(|m| m.candidate_name.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(&clean);

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
                    .line_spacing(LineSpacing::new().after(60)),
            );
            // Emit the profile-derived contact line right after the name, in place
            // of the scraped contact lines. `render_contact_line` runs `split_urls`,
            // so `[LinkedIn](url)` markdown and bare emails become real hyperlinks.
            if let Some(md) = &profile_contact_md {
                docx = docx.add_paragraph(
                    super::docx_renderer::render_contact_line(md, template, &colors)
                        .line_spacing(LineSpacing::new().after(40)),
                );
            }
            continue;
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

        // Body paragraphs — use proper spacing via pPr, no blank-paragraph spacers
        let para = render_cover_letter_paragraph(&clean, template, &colors, body_family);
        docx = docx.add_paragraph(para);
    }

    let (page_w, page_h) = page_size_dxa();
    docx = docx.page_size(page_w, page_h).page_margin(page_margin);
    Ok(docx)
}

// ─── Refined / Banded cover-letter DOCX (PR5) ──────────────────────────────────
//
// Shares the Classic renderer's line-by-line scan (same salutation/signoff/
// subject/date-ish detection — DOCX has no structured `LetterModel`, so this
// stays the single source of truth for that classification) but restyles each
// recognized zone per the chosen layout. Word/docx-rs has no angled-polygon or
// width-limited-rule primitive, so several `.typ` arrangement details are
// **documented approximations** here rather than faithful reproductions:
//
// - Refined's full-width horizontal rule under the header → a paragraph
//   BOTTOM BORDER (docx-rs borders are always full-paragraph-width, which
//   already matches "full-width" — no approximation needed there).
// - Refined's role line reads the `.typ`'s parsed `signature_title` (only
//   known after the sign-off, in a single forward pass over the model). DOCX's
//   flat scanner doesn't have that lookahead, so it uses `meta.job_title` —
//   the immediately-available equivalent — as the role text instead.
// - Banded's angled accent-tint polygon is NOT reproducible in DOCX at all;
//   approximated as a full-width PARAGRAPH SHADING band (lightened accent)
//   behind the (uppercased) name paragraph.
// - Banded's short (~28%-width) rule footer → docx-rs paragraph borders can't
//   be width-limited without a table, so it's approximated as a full-width
//   bottom border on the final paragraph.
// - Sidebar's tinted full-height LEFT MARGIN rail is not expressible in DOCX at
//   all (no margin-anchored frame that ATS parse safely, and a text box or
//   two-column table would be exactly the multi-column trap the whole export
//   avoids). Approximated the same way Banded's band is: paragraph SHADING in
//   the same `band_tint_hex` accent tint behind the (left-aligned) name
//   paragraph, with the contact stacked under it at the left margin rather than
//   pulled right. Same tint, same words, one column.
// - Monogram's initials device (a pale square before the name lockup) becomes a
//   shaded RUN carrying the same initials at the head of the name paragraph, so
//   both formats extract the identical "JS Jane Smith" — a separate shaded
//   paragraph would have put the initials on their own line, which the PDF
//   never does.
//
// ATS mode: every tint above is decorative and is suppressed when the request
// sets `ats_mode`, mirroring the `.typ` side's `data.opts.ats` gate so the two
// formats degrade together instead of one keeping a band the other dropped.
/// Per-layout DOCX styling decisions, stated once instead of being re-derived
/// from `matches!(layout, …)` at each of a dozen call sites.
///
/// Why this exists: the renderer used a single `is_refined` boolean and let
/// everything else fall through to Banded's treatment. Adding `Navy` therefore
/// silently gave it Banded's shaded band, footer rule and bolding while
/// stripping Refined's title and subject caption — a letter that rendered as
/// Navy in PDF and Banded in DOCX. Three review rounds each found a different
/// surviving branch, including two contact paths that disagreed with each other
/// depending on whether a `ContactProfile` happened to be attached.
///
/// Every field is filled in per layout in [`Self::for_layout`], so a new layout
/// cannot inherit another's look by omission — it has to say what it does.
struct LetterDocxStyle {
    /// Name in caps (Banded, Navy) vs. as written (Refined).
    uppercase_name: bool,
    /// Points added to the template's name size.
    name_pt_bonus: f32,
    /// Shaded block behind the name (Banded's band, Sidebar's rail).
    /// Decorative — suppressed under ATS mode.
    header_band: bool,
    /// Shaded run carrying the letterhead initials at the head of the name
    /// paragraph (Monogram only). Decorative — suppressed under ATS mode, which
    /// is also what `letter_monogram.typ` does with the device it approximates.
    monogram_device: bool,
    /// Name/title centred (Navy only).
    centred_letterhead: bool,
    /// Contact-line alignment. BOTH contact paths — profile-backed and the
    /// no-profile fallback — read this ONE field, so they cannot drift apart
    /// again; they did once, and the same letter then rendered differently
    /// depending on whether a `ContactProfile` happened to be attached. It is a
    /// stored value rather than a function of `centred_letterhead` because
    /// Sidebar and Monogram left-align the contact while leaving the name
    /// left-aligned too — a derived "centred or right" could not express that.
    contact_align: AlignmentType,
    /// Role line under the name (Refined, Navy).
    shows_title: bool,
    /// Role line rendered uppercase + letter-spaced in the accent colour
    /// (Refined), vs. plain case in the muted date colour (Navy). Presence and
    /// STYLE are separate decisions: the first pass made `shows_title` layout
    /// aware but left the styling hardcoded to Refined's, so Navy's DOCX title
    /// was accent/uppercase/tracked while its `.typ` renders muted plain text.
    title_emphasised: bool,
    /// Subject caption colour: accent for Refined (`letter_refined.typ` uses
    /// `c-accent`), the name colour for Navy (`letter_navy.typ` uses `c-name`).
    caption_uses_name_colour: bool,
    /// Small-caps subject caption (Refined, Navy).
    shows_subject_caption: bool,
    /// Bold date + recipient blocks (Banded only — see each `.typ`'s emit blocks).
    bolds_addressing: bool,
    /// Rule under the header block (Refined, Navy).
    header_rule: bool,
    /// Short rule at the foot of the letter (Banded only).
    footer_rule: bool,
    /// Empty paragraphs before the signature (Refined only).
    signature_gap: bool,
    /// Space after the contact paragraph, in twentieths of a point.
    contact_space_after: u32,
    /// Space after the sign-off, in twentieths of a point.
    signoff_space_after: u32,
}

impl LetterDocxStyle {
    fn for_layout(layout: LetterLayout) -> Self {
        match layout {
            // Classic never reaches this renderer (see the debug_assert below);
            // it is listed so the match stays exhaustive and a future layout
            // fails to compile until someone states its style.
            LetterLayout::Classic | LetterLayout::Refined => Self {
                title_emphasised: true,
                caption_uses_name_colour: false,
                uppercase_name: false,
                name_pt_bonus: 4.0,
                header_band: false,
                monogram_device: false,
                centred_letterhead: false,
                contact_align: AlignmentType::Right,
                shows_title: true,
                shows_subject_caption: true,
                bolds_addressing: false,
                header_rule: true,
                footer_rule: false,
                signature_gap: true,
                contact_space_after: 80,
                signoff_space_after: 40,
            },
            LetterLayout::Banded => Self {
                title_emphasised: false,
                caption_uses_name_colour: false,
                uppercase_name: true,
                name_pt_bonus: 0.0,
                header_band: true,
                monogram_device: false,
                centred_letterhead: false,
                contact_align: AlignmentType::Right,
                shows_title: false,
                shows_subject_caption: false,
                bolds_addressing: true,
                header_rule: false,
                footer_rule: true,
                signature_gap: false,
                contact_space_after: 40,
                signoff_space_after: 480,
            },
            LetterLayout::Navy => Self {
                title_emphasised: false,
                caption_uses_name_colour: true,
                uppercase_name: true,
                name_pt_bonus: 0.0,
                header_band: false,
                monogram_device: false,
                centred_letterhead: true,
                contact_align: AlignmentType::Center,
                shows_title: true,
                shows_subject_caption: true,
                bolds_addressing: false,
                header_rule: true,
                footer_rule: false,
                signature_gap: false,
                contact_space_after: 40,
                signoff_space_after: 480,
            },
            // Sidebar — read off `letter_sidebar.typ`, feature by feature:
            // name as written (no `upper()`), role line in the muted date
            // colour and plain case, small-caps accent subject caption,
            // unbolded date/recipient, no rule in design mode (the rail plays
            // that part), no footer rule, no signature gap. The rail itself has
            // no DOCX equivalent, so it becomes `header_band` — the same accent
            // tint, behind the name — and the contact stays at the LEFT margin
            // because the rail stacks it under the name rather than pulling it
            // to the right edge.
            LetterLayout::Sidebar => Self {
                title_emphasised: false,
                caption_uses_name_colour: false,
                uppercase_name: false,
                name_pt_bonus: 0.0,
                header_band: true,
                monogram_device: false,
                centred_letterhead: false,
                contact_align: AlignmentType::Left,
                shows_title: true,
                shows_subject_caption: true,
                bolds_addressing: false,
                header_rule: false,
                footer_rule: false,
                signature_gap: false,
                contact_space_after: 80,
                signoff_space_after: 480,
            },
            // Monogram — read off `letter_monogram.typ`: name as written with a
            // +1pt lockup, muted plain-case role line, subject caption in the
            // NAME colour (`c-name`, as the `.typ` uses), unbolded addressing,
            // an accent rule under the header block, and the initials device as
            // a shaded run instead of a shaded paragraph.
            LetterLayout::Monogram => Self {
                title_emphasised: false,
                caption_uses_name_colour: true,
                uppercase_name: false,
                name_pt_bonus: 1.0,
                header_band: false,
                monogram_device: true,
                centred_letterhead: false,
                contact_align: AlignmentType::Left,
                shows_title: true,
                shows_subject_caption: true,
                bolds_addressing: false,
                header_rule: true,
                footer_rule: false,
                signature_gap: false,
                contact_space_after: 80,
                signoff_space_after: 480,
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_cover_letter_docx_layout(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    contact: Option<&crate::contact_profile::ContactProfile>,
    lang: &str,
    market: &str,
    layout: LetterLayout,
    ats: bool,
) -> Result<Docx> {
    debug_assert!(
        !matches!(layout, LetterLayout::Classic),
        "generate_cover_letter_docx_layout serves every non-Classic layout"
    );
    let sty = LetterDocxStyle::for_layout(layout);
    // Decorative tints, dropped together with the `.typ` side's under ATS mode.
    // Derived once here rather than `&& !ats` at each call site, so a future
    // decoration cannot be added to only one of them.
    let show_band = sty.header_band && !ats;
    let show_device = sty.monogram_device && !ats;

    let mut docx = Docx::new();

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
    let accent_hex = rgb_to_hex(template.accent_color);
    let rule_hex = rgb_to_hex(template.rule_color);
    let band_hex = band_tint_hex(template.accent_color);

    // Market convention — consulted here only for Refined's reference-line
    // label, so it matches the same market convention `letter_refined.typ`
    // reads from `data.opts.subject_line_label` (DOCX's scanner is otherwise
    // market-agnostic, same as Classic).
    let subj_label = crate::locale::letter::conventions(market)
        .subject_line
        .label
        .clone();

    // A single full-width bottom-border rule, built fresh per call (docx-rs's
    // `ParagraphBorders::default()` pre-fills all four sides — `with_empty()`
    // avoids drawing an unwanted box).
    let bottom_rule = |color: &str, size: usize| {
        ParagraphBorders::with_empty().set(
            ParagraphBorder::new(ParagraphBorderPosition::Bottom)
                .val(BorderType::Single)
                .size(size)
                .color(color),
        )
    };

    let lines: Vec<&str> = text.lines().collect();
    let mut header_done = false;
    let mut in_body = false;

    for raw_line in &lines {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let clean = strip_md(trimmed);

        let is_salutation = crate::locale::letter::is_salutation(&clean);
        let is_signoff = crate::locale::letter::is_signoff(&clean);
        let is_subject_line = crate::locale::letter::is_subject_line(&clean);

        // First line is name — but ONLY a real letterhead name, not a letter that
        // opens straight at the salutation, sign-off, or subject/reference line
        // (e.g. German "Betreff: …"). Without this guard a letterhead-less
        // letter's "Dear …"/"Betreff: …" was consumed as the name (replaced with
        // the candidate name), the salutation/subject arm never ran, `in_body`
        // was never set, and the whole body rendered in the muted addressee style.
        if !header_done
            && docx.document.children.is_empty()
            && !is_salutation
            && !is_signoff
            && !is_subject_line
        {
            let name_text = meta
                .and_then(|m| m.candidate_name.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(&clean);
            // Banded: uppercase name — the same small-caps→uppercase precedent
            // `render_section_header` uses for the résumé DOCX path.
            let display_name = if !sty.uppercase_name {
                name_text.to_string()
            } else {
                name_text.to_uppercase()
            };
            let name_pt = template.name_pt + sty.name_pt_bonus;

            let mut name_para = Paragraph::new();
            // Monogram device approximation: the initials as a SHADED RUN at
            // the head of the name paragraph, from the same
            // `monogram_initials` the `.typ` reads via `LetterHead.initials`,
            // so the two formats can never disagree about what it says. A run
            // rather than its own paragraph because `letter_monogram.typ` sets
            // the square BESIDE the name — extraction must read
            // "JS Jane Smith", not "JS" on a line of its own.
            if show_device {
                let initials = crate::export::typst_engine::monogram_initials(name_text);
                if !initials.is_empty() {
                    name_para = name_para.add_run(
                        Run::new()
                            .add_text(format!("{initials}  "))
                            .size(pt_to_half_points(name_pt))
                            .bold()
                            .color(&accent_hex)
                            .fonts(docx_run_fonts(name_family))
                            .shading(
                                Shading::new()
                                    .shd_type(ShdType::Clear)
                                    .color("auto")
                                    .fill(&band_hex),
                            ),
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
                .line_spacing(LineSpacing::new().after(60));
            if sty.centred_letterhead {
                // `letter_navy.typ` centres name, title and contact, then rules
                // UNDER the lot. The rule therefore goes on the contact
                // paragraph below, not here; drawing it on the name would put a
                // line between the name and its own contact line.
                name_para = name_para.align(AlignmentType::Center);
            }
            if show_band {
                // Banded's band / Sidebar's rail approximation — see the
                // module-level doc comment. Navy and Monogram are deliberately
                // excluded: neither design has a tinted block behind the name.
                name_para.property = name_para.property.shading(
                    Shading::new()
                        .shd_type(ShdType::Clear)
                        .color("auto")
                        .fill(&band_hex),
                );
            }
            docx = docx.add_paragraph(name_para);

            // Role line right after the name (see approximation note above re:
            // `meta.job_title` vs the `.typ`'s `signature_title`). Refined and
            // Navy both render it; Banded does not.
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
                            &accent_hex
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
                    docx = docx.add_paragraph(title_para);
                }
            }

            if let Some(md) = &profile_contact_md {
                let mut contact_para =
                    super::docx_renderer::render_contact_line(md, template, &colors)
                        .align(sty.contact_align)
                        .line_spacing(LineSpacing::new().after(sty.contact_space_after));
                if sty.header_rule {
                    // Full-width rule under the header — see approximation note.
                    // For Navy this is the letterhead rule, which is why it sits
                    // here (after name + title + contact) rather than on the name.
                    contact_para.property =
                        contact_para.property.set_borders(bottom_rule(&rule_hex, 6));
                }
                docx = docx.add_paragraph(contact_para);
            }
            continue;
        }

        // Contact/address lines (only reached when no ContactProfile was
        // supplied — mirrors Classic's fallback).
        if !header_done
            && (clean.contains('@')
                || clean.contains('|')
                || clean.contains('·')
                || clean.chars().filter(|c| c.is_numeric()).count() > 5)
        {
            if profile_contact_md.is_some() {
                continue;
            }
            let mut run = Run::new()
                .add_text(&clean)
                .size(pt_to_half_points(9.0))
                .color(&colors.date)
                .fonts(docx_run_fonts(body_family));
            if sty.bolds_addressing {
                // Banded bolds date/address-ish lines, mirroring
                // `letter_banded.typ`'s bold `emit-date-block`. Navy does not —
                // its `emit-date-block` is regular weight.
                run = run.bold();
            }
            // Same style source as the profile-backed contact path above. These
            // two used to disagree: this fallback right-aligned Navy with no
            // rule while the other centred it and ruled it, so the SAME letter
            // rendered differently depending on whether a ContactProfile
            // happened to be attached.
            let mut para = Paragraph::new()
                .add_run(run)
                .align(sty.contact_align)
                .line_spacing(LineSpacing::new().after(40));
            if sty.header_rule {
                para.property = para.property.set_borders(bottom_rule(&rule_hex, 6));
            }
            docx = docx.add_paragraph(para);
            continue;
        }

        // Salutation — unchanged styling in both layouts (mirrors `.typ`,
        // which never bolds or recolors the salutation).
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

        // Sign-off.
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
                    .line_spacing(
                        LineSpacing::new()
                            .before(240)
                            .after(sty.signoff_space_after),
                    ),
            );
            if sty.signature_gap {
                // Extra empty paragraphs to leave room for a handwritten
                // signature — approximates the `.typ`'s `#v(34pt)` gap
                // (docx-rs has no direct vertical-space primitive outside
                // paragraph spacing/empty paragraphs).
                for _ in 0..2 {
                    docx = docx.add_paragraph(
                        Paragraph::new().add_run(
                            Run::new()
                                .add_text("")
                                .size(pt_to_half_points(template.body_pt)),
                        ),
                    );
                }
            }
            continue;
        }

        // Subject / reference line (Betreff/Objet/Re/…) — before the salutation.
        if !in_body && is_subject_line {
            if sty.shows_subject_caption {
                // The always-on JOB REFERENCE line: caption suppressed when the
                // subject already opens with the market's own label or "Re:" —
                // same content rule as `letter_refined.typ`'s
                // `strip-subject-label` + `has-own-label` check.
                let subj_body = strip_market_label(&clean, &subj_label);
                let subj_body_lower = subj_body.to_lowercase();
                let has_own_label = (!subj_label.is_empty()
                    && subj_body_lower.starts_with(&subj_label.to_lowercase()))
                    || subj_body_lower.starts_with("re:");

                if !has_own_label {
                    let caption = if subj_label.is_empty() {
                        "Subject".to_string()
                    } else {
                        subj_label.clone()
                    };
                    docx = docx.add_paragraph(
                        Paragraph::new()
                            .add_run(
                                Run::new()
                                    .add_text(caption.to_uppercase())
                                    .size(pt_to_half_points((template.body_pt - 1.5).max(6.0)))
                                    .bold()
                                    .color(if sty.caption_uses_name_colour {
                                        &colors.name
                                    } else {
                                        &accent_hex
                                    })
                                    .character_spacing(24)
                                    .fonts(docx_run_fonts(body_family)),
                            )
                            .line_spacing(LineSpacing::new().after(20)),
                    );
                }
                docx = docx.add_paragraph(
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
                docx = docx.add_paragraph(
                    Paragraph::new()
                        .add_run(
                            Run::new()
                                .add_text(&clean)
                                .size(pt_to_half_points(template.body_pt))
                                .bold()
                                .color(&accent_hex)
                                .fonts(docx_run_fonts(body_family)),
                        )
                        .line_spacing(LineSpacing::new().before(120).after(120)),
                );
            }
            continue;
        }

        // Addressee block (before salutation).
        if !in_body {
            let mut run = Run::new()
                .add_text(&clean)
                .size(pt_to_half_points(template.body_pt))
                .color(&colors.date)
                .fonts(docx_run_fonts(body_family));
            if sty.bolds_addressing {
                // Banded bolds the recipient block, mirroring
                // `letter_banded.typ`'s bold `emit-recipient-block`. Navy's is
                // regular weight.
                run = run.bold();
            }
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(run)
                    .line_spacing(LineSpacing::new().after(40)),
            );
            continue;
        }

        // Body paragraphs — unchanged in both layouts.
        let para = render_cover_letter_paragraph(&clean, template, &colors, body_family);
        docx = docx.add_paragraph(para);
    }

    if sty.footer_rule {
        // Banded's short rule footer — see approximation note above. Navy's
        // rule is under the letterhead, not at the foot, so it is excluded.
        let mut footer = Paragraph::new()
            .add_run(
                Run::new()
                    .add_text("")
                    .size(pt_to_half_points(template.body_pt)),
            )
            .line_spacing(LineSpacing::new().before(160));
        footer.property = footer.property.set_borders(bottom_rule(&accent_hex, 18));
        docx = docx.add_paragraph(footer);
    }

    let (page_w, page_h) = page_size_dxa();
    docx = docx.page_size(page_w, page_h).page_margin(page_margin);
    Ok(docx)
}

/// Shading fill for every DOCX "header band" approximation: the template
/// accent lightened 85 % toward white.
///
/// DOCX has no page-background primitive, so a PDF band becomes paragraph
/// shading — and paragraph shading is only legible behind the normal dark ink
/// if the fill is a pale tint. Both users (this file's Banded cover letter and
/// [`super::model_docx`]'s Awesome résumé header) call this, so the two can
/// never drift to different tints of the same accent.
pub(super) fn band_tint_hex(accent: (u8, u8, u8)) -> String {
    rgb_to_hex(lighten_rgb(accent, 0.85))
}

/// Lighten an RGB colour toward white by `amount` (0.0..=1.0), mirroring
/// Typst's `color.lighten(pct)` used for `letter_banded.typ`'s band tint, so
/// the DOCX approximation matches the same pale accent the PDF renders.
fn lighten_rgb(rgb: (u8, u8, u8), amount: f32) -> (u8, u8, u8) {
    let blend = |c: u8| -> u8 {
        let c = c as f32;
        (c + (255.0 - c) * amount).round().clamp(0.0, 255.0) as u8
    };
    (blend(rgb.0), blend(rgb.1), blend(rgb.2))
}

/// Strip a leading "`<label>`[:]" prefix from a subject line (case-insensitive),
/// mirroring `letter_refined.typ`'s `strip-subject-label`. Labels are ASCII, so
/// slicing by the label's byte length removes exactly the prefix.
fn strip_market_label(s: &str, label: &str) -> String {
    let t = s.trim();
    if !label.is_empty() && t.to_lowercase().starts_with(&label.to_lowercase()) {
        let rest = t[label.len().min(t.len())..].trim_start();
        let rest = rest.strip_prefix(':').unwrap_or(rest).trim_start();
        rest.to_string()
    } else {
        t.to_string()
    }
}

// ─── Extract section helper ───────────────────────────────────────────────────

fn extract_section<'a>(text: &'a str, start_marker: &str, end_marker: Option<&str>) -> &'a str {
    let start = if let Some(idx) = text.find(start_marker) {
        let after = &text[idx + start_marker.len()..];
        after
            .find('\n')
            .map(|i| idx + start_marker.len() + i + 1)
            .unwrap_or(idx + start_marker.len())
    } else {
        return text;
    };
    let end = if let Some(em) = end_marker {
        text[start..]
            .find(em)
            .map(|i| start + i)
            .unwrap_or(text.len())
    } else {
        text.len()
    };
    text[start..end].trim()
}

// ─── Public entry point ───────────────────────────────────────────────────────

pub fn generate_docx(request: &ExportRequest) -> Result<Vec<u8>> {
    // Document accent (ADR 0004): recolor the template's accent-derived fields
    // when a valid override is present. `setup_colors` reads `emphasis_color`, so
    // the accent surfaces on emphasized runs for both the résumé and cover-letter
    // DOCX paths (both derive from this `template`). No-op when absent/malformed.
    let template =
        Template::get(request.template_id).with_accent_override(request.accent.as_deref());

    // The DOCX path collapses two-column templates to a single column since
    // DOCX doesn't replicate the sidebar layout.
    let single_column = || {
        let mut t = template.clone();
        if crate::theme::is_two_column(request.template_id) {
            t.two_column = None;
            t.margin_in = 1.0;
        }
        t
    };

    let docx = match request.document_type {
        DocumentType::Resume => {
            let text = extract_section(
                &request.text,
                "### CANDIDATE RESUME ###",
                Some("### JOB ADVERTISEMENT ###"),
            );
            let text = if text.is_empty() {
                request.text.as_str()
            } else {
                text
            };
            // model_docx is the sole résumé-DOCX path. The legacy flat-parser arm
            // has been removed — it was only reachable with `--no-default-features`
            // and diverged from the model path.
            crate::export::model_docx::generate_resume_docx_in(
                text,
                request.meta.as_ref(),
                &template,
                request.ats_mode,
                request.page_geometry(),
                request.contact.as_ref(),
                &request.target_lang(),
            )
            .context("Failed to generate resume DOCX")?
        }
        DocumentType::CoverLetter => {
            let text = extract_section(&request.text, "### COMPLETE COVER LETTER ###", None);
            let text = if text.is_empty() {
                request.text.as_str()
            } else {
                text
            };
            // `market` drives only the Refined reference-line label (see
            // `generate_cover_letter_docx_layout`'s doc comment); mirrors the
            // PDF cover-letter path's `market` computation in `pdf/mod.rs`.
            let market = request.locale.as_deref().unwrap_or("intl");
            generate_cover_letter_docx(
                text,
                request.meta.as_ref(),
                &single_column(),
                request.contact.as_ref(),
                &request.target_lang(),
                market,
                request.letter_layout,
                // Mirrors the PDF cover-letter path: ATS mode drops each
                // layout's decorative tint in BOTH formats, so a golden-parity
                // check can't find a band in one and not the other.
                request.ats_mode,
            )
            .context("Failed to generate cover letter DOCX")?
        }
    };

    let mut buffer = std::io::Cursor::new(Vec::new());
    docx.build()
        .pack(&mut buffer)
        .context("Failed to pack DOCX")?;
    Ok(buffer.into_inner())
}

#[cfg(test)]
mod test;
