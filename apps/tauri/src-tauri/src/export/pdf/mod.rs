use anyhow::{Context, Result};
use printpdf::*;

use super::{
    parser::{parse_resume, strip_md},
    templates::{DatePosition, ParagraphIndent, Template},
    types::{DocumentType, ExportRequest, GenerationMeta, LineKind},
};

use super::pdf_renderer::*;
// Explicit import to resolve ambiguity with printpdf::PageState
use super::pdf_renderer::PageState;

// ─── Resume PDF (single-column) ───────────────────────────────────────────────

/// Legacy resume renderer. `pub(crate)` so the layout-engine parity gate
/// (`layout::tests`) can compare its output against the new backend regardless
/// of the `layout_pdf` feature; the public entry point stays [`generate_pdf`].
pub(crate) fn generate_resume_pdf(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    ats_mode: bool,
) -> Result<Vec<u8>> {
    // ATS linearization: collapse two-column templates to single-column
    let effective_template = if ats_mode && template.two_column.is_some() {
        let mut t = template.clone();
        t.two_column = None;
        t.margin_in = 1.0;
        t
    } else {
        template.clone()
    };

    if effective_template.two_column.is_some() {
        return generate_two_column_resume_pdf(text, meta, &effective_template);
    }

    let parsed = parse_resume(text);
    let mut doc = PdfDocument::new("Resume");
    let used = collect_codepoints(
        std::iter::once(text).chain(meta.and_then(|m| m.candidate_name.as_deref())),
    );
    let fonts = load_all_fonts(&mut doc, &used)?;
    let layout = setup_layout(&effective_template);
    let colors = setup_colors(&effective_template);

    let mut page_state = PageState::new(&layout);
    let mut previous_kind: Option<LineKind> = None;

    let two_line_gap = layout.line_height * 2.0;
    let three_line_gap = layout.line_height * 3.0;

    for line in &parsed.lines {
        let spacing = super::templates::calculate_spacing(&line.kind, previous_kind.as_ref());
        page_state.y -= pt_to_mm(spacing.0);

        match line.kind {
            LineKind::Blank => {
                page_state.y -= pt_to_mm(3.0);
            }

            LineKind::Name => {
                let (line_ops, new_y) = render_name_line(
                    &line.text,
                    meta,
                    &effective_template,
                    &layout,
                    &colors,
                    &fonts,
                    page_state.y,
                );
                page_state.current_ops.extend(line_ops);
                page_state.y = new_y;
            }

            LineKind::Contact => {
                let (line_ops, new_y) = render_contact_line(
                    &line.text,
                    &effective_template,
                    &layout,
                    &colors,
                    &fonts,
                    page_state.y,
                );
                page_state.current_ops.extend(line_ops);
                page_state.y = new_y;
            }

            LineKind::SectionHeader => {
                // Orphan guard: need at least 3 lines below the header
                maybe_break_before(
                    &mut page_state,
                    pt_to_mm(effective_template.section_pt) * 1.5,
                    three_line_gap,
                );
                let (line_ops, new_y) = render_section_header(
                    &line.text,
                    &effective_template,
                    &layout,
                    &colors,
                    &fonts,
                    page_state.y,
                );
                page_state.current_ops.extend(line_ops);
                page_state.y = new_y;
            }

            LineKind::JobEntry => {
                maybe_break_before(&mut page_state, layout.line_height, two_line_gap);
                let (line_ops, new_y) = render_job_entry(
                    &line.segments,
                    line.right_text.as_deref(),
                    &effective_template,
                    &layout,
                    &colors,
                    &fonts,
                    page_state.y,
                );
                page_state.current_ops.extend(line_ops);
                page_state.y = new_y;
            }

            LineKind::JobTitle => {
                maybe_break_before(&mut page_state, layout.line_height, 0.0);
                let (line_ops, new_y) = render_job_title(
                    &line.text,
                    &effective_template,
                    &layout,
                    &colors,
                    &fonts,
                    page_state.y,
                );
                page_state.current_ops.extend(line_ops);
                page_state.y = new_y;
            }

            LineKind::Bullet => {
                if page_state.needs_break() {
                    page_state.new_page();
                }
                let (line_ops, new_y) = render_bullet_line(
                    &line.segments,
                    &effective_template,
                    &layout,
                    &colors,
                    &fonts,
                    page_state.y,
                );
                page_state.current_ops.extend(line_ops);
                page_state.y = new_y;
            }

            LineKind::Text => {
                if page_state.needs_break() {
                    page_state.new_page();
                }
                let (line_ops, new_y) = render_text_line(
                    &line.segments,
                    &effective_template,
                    &layout,
                    &colors,
                    &fonts,
                    page_state.y,
                );
                page_state.current_ops.extend(line_ops);
                page_state.y = new_y;
            }
        }

        page_state.y -= pt_to_mm(spacing.1);
        previous_kind = Some(line.kind);
    }

    assemble_pdf(doc, page_state, &layout)
}

// ─── Two-column resume PDF ────────────────────────────────────────────────────

/// Renders the two-column layout with content stream ordered as
/// header → main column → sidebar column. This produces correct reading order
/// for stream-based extractors (pdf-extract, pdftotext, older ATS parsers).
///
/// Position-based parsers (some modern ATS like Workday) may still interleave
/// columns. For important applications, users should enable ATS mode, which
/// linearizes the layout to a true single column.
fn generate_two_column_resume_pdf(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
) -> Result<Vec<u8>> {
    let tc = template
        .two_column
        .as_ref()
        .expect("two_column config required");
    let parsed = parse_resume(text);
    let mut doc = PdfDocument::new("Resume");
    let used = collect_codepoints(
        std::iter::once(text).chain(meta.and_then(|m| m.candidate_name.as_deref())),
    );
    let fonts = load_all_fonts(&mut doc, &used)?;
    let layout = setup_layout(template);
    let colors = setup_colors(template);

    let sidebar_section_names: std::collections::HashSet<&str> =
        tc.sidebar_sections.iter().copied().collect();

    // Classify lines
    let mut header_lines = Vec::new();
    let mut main_lines = Vec::new();
    let mut sidebar_lines = Vec::new();
    let mut in_sidebar_section = false;

    for line in &parsed.lines {
        match line.kind {
            LineKind::Name | LineKind::Contact => {
                header_lines.push(line);
            }
            LineKind::SectionHeader => {
                let is_sidebar = sidebar_section_names.contains(line.text.as_str())
                    || sidebar_section_names.contains(line.text.to_uppercase().as_str());
                in_sidebar_section = is_sidebar;
                if is_sidebar {
                    sidebar_lines.push(line);
                } else {
                    main_lines.push(line);
                }
            }
            _ => {
                if in_sidebar_section {
                    sidebar_lines.push(line);
                } else {
                    main_lines.push(line);
                }
            }
        }
    }

    let mut ops: Vec<Op> = Vec::new();
    let mut y = layout.page_height - pt_to_mm(layout.top_margin_pt);

    // Render header
    for line in &header_lines {
        match line.kind {
            LineKind::Name => {
                let (line_ops, new_y) =
                    render_name_line(&line.text, meta, template, &layout, &colors, &fonts, y);
                ops.extend(line_ops);
                y = new_y;
            }
            LineKind::Contact => {
                let (line_ops, new_y) =
                    render_contact_line(&line.text, template, &layout, &colors, &fonts, y);
                ops.extend(line_ops);
                y = new_y;
            }
            _ => {}
        }
    }

    let header_bottom_y = y - pt_to_mm(2.0);

    // Draw sidebar background
    ops.extend(draw_sidebar_bg(
        &layout,
        tc.sidebar_bg_color,
        header_bottom_y,
        layout.margin_in * 25.4,
    ));

    // Render main column (experience etc.)
    let main_layout = LayoutConfig {
        main_x: layout.main_x,
        main_width: layout.main_width,
        ..layout
    };
    let mut main_y = header_bottom_y;
    for line in &main_lines {
        let spacing = super::templates::calculate_spacing(&line.kind, None);
        main_y -= pt_to_mm(spacing.0);
        match line.kind {
            LineKind::SectionHeader => {
                let (lo, ny) = render_section_header(
                    &line.text,
                    template,
                    &main_layout,
                    &colors,
                    &fonts,
                    main_y,
                );
                ops.extend(lo);
                main_y = ny;
            }
            LineKind::JobEntry => {
                let (lo, ny) = render_job_entry(
                    &line.segments,
                    line.right_text.as_deref(),
                    template,
                    &main_layout,
                    &colors,
                    &fonts,
                    main_y,
                );
                ops.extend(lo);
                main_y = ny;
            }
            LineKind::JobTitle => {
                let (lo, ny) =
                    render_job_title(&line.text, template, &main_layout, &colors, &fonts, main_y);
                ops.extend(lo);
                main_y = ny;
            }
            LineKind::Bullet => {
                let (lo, ny) = render_bullet_line(
                    &line.segments,
                    template,
                    &main_layout,
                    &colors,
                    &fonts,
                    main_y,
                );
                ops.extend(lo);
                main_y = ny;
            }
            LineKind::Text => {
                let (lo, ny) = render_text_line(
                    &line.segments,
                    template,
                    &main_layout,
                    &colors,
                    &fonts,
                    main_y,
                );
                ops.extend(lo);
                main_y = ny;
            }
            LineKind::Blank => {
                main_y -= pt_to_mm(3.0);
            }
            _ => {}
        }
        main_y -= pt_to_mm(spacing.1);
    }

    // Render sidebar column (skills/education)
    let sidebar_layout = LayoutConfig {
        main_x: layout.margin_left,
        main_width: layout.sidebar_width - 3.0,
        ..layout
    };
    let mut sidebar_y = header_bottom_y;
    for line in &sidebar_lines {
        let spacing = super::templates::calculate_spacing(&line.kind, None);
        sidebar_y -= pt_to_mm(spacing.0);
        match line.kind {
            LineKind::SectionHeader => {
                let (lo, ny) = render_section_header(
                    &line.text,
                    template,
                    &sidebar_layout,
                    &colors,
                    &fonts,
                    sidebar_y,
                );
                ops.extend(lo);
                sidebar_y = ny;
            }
            LineKind::Bullet => {
                let (lo, ny) = render_bullet_line(
                    &line.segments,
                    template,
                    &sidebar_layout,
                    &colors,
                    &fonts,
                    sidebar_y,
                );
                ops.extend(lo);
                sidebar_y = ny;
            }
            LineKind::Text | LineKind::JobTitle => {
                let (lo, ny) = render_text_line(
                    &line.segments,
                    template,
                    &sidebar_layout,
                    &colors,
                    &fonts,
                    sidebar_y,
                );
                ops.extend(lo);
                sidebar_y = ny;
            }
            LineKind::Blank => {
                sidebar_y -= pt_to_mm(3.0);
            }
            _ => {}
        }
        sidebar_y -= pt_to_mm(spacing.1);
    }

    let page = PdfPage::new(Mm(layout.page_width), Mm(layout.page_height), ops);
    let mut save_warnings = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut save_warnings);
    Ok(pdf_bytes)
}

// ─── Cover letter PDF ─────────────────────────────────────────────────────────

/// Does this cleaned line look like a contact line (e.g. `City | a@b.com | +49 …
/// | LinkedIn`)? Detected structurally — an email `@` or ≥2 `|` separators — so a
/// contact line the generated letter still carries is dropped from the body
/// regardless of what the profile-derived letterhead renders. Plain recipient
/// lines ("JAKALA", "Hiring Team") and the date have neither, so they survive.
fn looks_like_contact_line(clean: &str) -> bool {
    clean.contains('@') || clean.matches('|').count() >= 2
}

fn generate_cover_letter_pdf(
    text: &str,
    meta: Option<&GenerationMeta>,
    template: &Template,
    contact: Option<&crate::contact_profile::ContactProfile>,
    lang: &str,
    // Resolved job-market id (`us`, `de`, …) — drives the subject line + date
    // placement per market. Defaults to the intl baseline for an unknown market.
    market: &str,
) -> Result<Vec<u8>> {
    let mut doc = PdfDocument::new("Cover Letter");
    // Seed the subset with every text input the letter can draw: the body, the
    // candidate name override, and the profile-derived contact header.
    let header = contact.map(|c| c.header_markdown(lang));
    let used = collect_codepoints(
        std::iter::once(text)
            .chain(meta.and_then(|m| m.candidate_name.as_deref()))
            .chain(header.as_deref()),
    );
    let fonts = load_all_fonts(&mut doc, &used)?;
    let layout = setup_layout(template);
    let colors = setup_colors(template);
    let cl = &template.cover_letter;

    let mut page_state = PageState::new(&layout);
    let content_width = layout.page_width - layout.margin_left - layout.margin_right;
    let x = layout.margin_left;

    // Candidate name from meta or first line of text
    let name_text = meta
        .and_then(|m| m.candidate_name.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("");

    // Contact line: the named profile fields are the source of truth (shared with
    // the résumé header, so both documents show identical correctly-named links).
    // Fall back to scraping the generated text only when no profile is supplied.
    let raw_lines: Vec<&str> = text.lines().collect();
    let contact_line: String = match contact {
        Some(profile) if !profile.is_effectively_empty() => profile.header_markdown(lang),
        _ => raw_lines
            .iter()
            .take(4)
            .filter(|l| {
                let c = strip_md(l.trim());
                c.contains('@') || c.contains('|') || c.contains('·')
            })
            .map(|l| strip_md(l.trim()))
            .collect::<Vec<_>>()
            .join(" · "),
    };

    // Render letterhead
    let render_ctx = RenderCtx {
        template,
        layout: &layout,
        colors: &colors,
        fonts: &fonts,
    };
    let (lh_ops, new_y) = render_letterhead(
        &render_ctx,
        name_text,
        &contact_line,
        cl.header_style,
        page_state.y,
    );
    page_state.current_ops.extend(lh_ops);
    page_state.y = new_y;

    // Find body sections by scanning lines
    let mut body_started = false;
    let mut skip_lines = 0usize; // lines consumed by header
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current_para = String::new();
    let mut date_str: Option<String> = None;
    let mut recipient_lines: Vec<String> = Vec::new();
    let mut subject_line: Option<String> = None;
    let mut salutation_line: Option<String> = None;
    let mut closing_line: Option<String> = None;
    let mut after_closing = false;
    let mut signature_title: Option<String> = None;

    for raw_line in &raw_lines {
        let trimmed = raw_line.trim();
        let clean = strip_md(trimmed);

        // Skip lines that were part of the header (name + blank lines + the
        // profile-derived contact line if it echoes here).
        if skip_lines < 3
            && (clean.is_empty() || trimmed == name_text || contact_line.contains(&clean))
        {
            skip_lines += 1;
            continue;
        }
        // Drop any stray contact line the generated text still carries before the
        // body proper — the letterhead already renders the contact from the
        // profile, so leaving it here would duplicate it and leak raw markdown
        // (e.g. `[Dribbble](…`) into the recipient block.
        if !body_started && looks_like_contact_line(&clean) {
            continue;
        }

        // Empty line → paragraph break
        if clean.is_empty() {
            if !current_para.is_empty() {
                paragraphs.push(std::mem::take(&mut current_para));
            }
            continue;
        }

        // Locale-aware: recognize salutations/sign-offs across every supported
        // market (was English/German only, which dumped FR/ES/IT/JP/… greetings
        // into the recipient block).
        let is_salutation = crate::locale::letter::is_salutation(&clean);
        let is_signoff = crate::locale::letter::is_signoff(&clean);

        if is_salutation {
            if !current_para.is_empty() {
                paragraphs.push(std::mem::take(&mut current_para));
            }
            salutation_line = Some(clean.clone());
            body_started = true;
            continue;
        }

        if is_signoff {
            if !current_para.is_empty() {
                paragraphs.push(std::mem::take(&mut current_para));
            }
            closing_line = Some(clean.clone());
            after_closing = true;
            continue;
        }

        if after_closing {
            // Lines after closing: first non-blank is signature title
            if signature_title.is_none() && !clean.is_empty() && clean != name_text {
                signature_title = Some(clean.clone());
            }
            continue;
        }

        if !body_started {
            // Pre-salutation: subject line (Betreff/Objet/…), date, or recipient.
            if crate::locale::letter::is_subject_line(&clean) && subject_line.is_none() {
                subject_line = Some(clean.clone());
            } else if DATE_PATTERN.is_match(&clean) && date_str.is_none() {
                date_str = Some(clean.clone());
            } else {
                recipient_lines.push(clean.clone());
            }
        } else {
            // Body paragraphs
            if !current_para.is_empty() {
                current_para.push(' ');
            }
            current_para.push_str(&clean);
        }
    }
    if !current_para.is_empty() {
        paragraphs.push(current_para);
    }

    let line_height = layout.line_height;

    // The job market can override the template's date placement (e.g. DACH/DIN
    // wants the date top-right); unknown markets keep the template default.
    let market_conv = crate::locale::letter::conventions(market);
    let date_pos = map_date_position(&market_conv.date_position).unwrap_or(cl.date_position);

    // Date line
    if let Some(date) = &date_str {
        match date_pos {
            DatePosition::TopRight | DatePosition::AboveSalutation => {
                let (reg_id, _, _) = resolve_fonts(&fonts, template.fonts.body_family);
                let date_x = layout.page_width
                    - layout.margin_right
                    - (date.len() as f32 * pt_to_mm(10.0) * 0.5);
                let pos = Point {
                    x: Mm(date_x).into(),
                    y: Mm(page_state.y).into(),
                };
                page_state.current_ops.extend([
                    Op::StartTextSection,
                    Op::SetFillColor {
                        col: colors.date.clone(),
                    },
                    Op::SetTextCursor { pos },
                    Op::SetFont {
                        font: PdfFontHandle::External(reg_id.clone()),
                        size: Pt(10.0),
                    },
                    Op::ShowText {
                        items: vec![TextItem::Text(date.clone())],
                    },
                    Op::EndTextSection,
                ]);
            }
            DatePosition::BelowHeader => {
                let (reg_id, _, _) = resolve_fonts(&fonts, template.fonts.body_family);
                let pos = Point {
                    x: Mm(x).into(),
                    y: Mm(page_state.y).into(),
                };
                page_state.current_ops.extend([
                    Op::StartTextSection,
                    Op::SetFillColor {
                        col: colors.date.clone(),
                    },
                    Op::SetTextCursor { pos },
                    Op::SetFont {
                        font: PdfFontHandle::External(reg_id.clone()),
                        size: Pt(10.0),
                    },
                    Op::ShowText {
                        items: vec![TextItem::Text(date.clone())],
                    },
                    Op::EndTextSection,
                ]);
                page_state.y -= line_height + pt_to_mm(2.0);
            }
            DatePosition::Omitted => {}
        }
    }

    // Recipient block
    if cl.recipient_block && !recipient_lines.is_empty() {
        page_state.y -= pt_to_mm(6.0);
        let (reg_id, _, _) = resolve_fonts(&fonts, template.fonts.body_family);
        for rline in &recipient_lines {
            let pos = Point {
                x: Mm(x).into(),
                y: Mm(page_state.y).into(),
            };
            page_state.current_ops.extend([
                Op::StartTextSection,
                Op::SetFillColor {
                    col: colors.body.clone(),
                },
                Op::SetTextCursor { pos },
                Op::SetFont {
                    font: PdfFontHandle::External(reg_id.clone()),
                    size: Pt(template.body_pt),
                },
                Op::ShowText {
                    items: vec![TextItem::Text(rline.clone())],
                },
                Op::EndTextSection,
            ]);
            page_state.y -= line_height;
        }
        page_state.y -= pt_to_mm(4.0);
    }

    // Subject line (Betreff/Objet/Oggetto/…) — rendered bold between the
    // recipient block and the salutation, as formal markets expect.
    if let Some(subject) = &subject_line {
        page_state.y -= pt_to_mm(2.0);
        let (_, bold_id, _) = resolve_fonts(&fonts, template.fonts.body_family);
        let pos = Point {
            x: Mm(x).into(),
            y: Mm(page_state.y).into(),
        };
        page_state.current_ops.extend([
            Op::StartTextSection,
            Op::SetFillColor {
                col: colors.body.clone(),
            },
            Op::SetTextCursor { pos },
            Op::SetFont {
                font: PdfFontHandle::External(bold_id.clone()),
                size: Pt(template.body_pt),
            },
            Op::ShowText {
                items: vec![TextItem::Text(subject.clone())],
            },
            Op::EndTextSection,
        ]);
        page_state.y -= line_height + pt_to_mm(2.0);
    }

    // Salutation
    let sal = salutation_line.unwrap_or_else(|| cl.closing_phrase_default.to_string()); // fallback
    {
        page_state.y -= pt_to_mm(4.0);
        let (reg_id, _, _) = resolve_fonts(&fonts, template.fonts.body_family);
        let pos = Point {
            x: Mm(x).into(),
            y: Mm(page_state.y).into(),
        };
        page_state.current_ops.extend([
            Op::StartTextSection,
            Op::SetFillColor {
                col: colors.body.clone(),
            },
            Op::SetTextCursor { pos },
            Op::SetFont {
                font: PdfFontHandle::External(reg_id.clone()),
                size: Pt(template.body_pt),
            },
            Op::ShowText {
                items: vec![TextItem::Text(sal)],
            },
            Op::EndTextSection,
        ]);
        page_state.y -= line_height + pt_to_mm(4.0);
    }

    // Body paragraphs with widow/orphan control
    for para_text in &paragraphs {
        let estimated_h = estimate_paragraph_height(
            para_text,
            content_width,
            template.body_pt,
            line_height,
            if cl.paragraph_indent == ParagraphIndent::FirstLine {
                6.35
            } else {
                0.0
            },
        );
        // Widow/orphan: push whole paragraph to next page if < 2 lines would fit
        let min_tail = line_height * 2.0;
        maybe_break_before(
            &mut page_state,
            estimated_h.min(line_height * 2.0),
            min_tail,
        );

        let (para_ops, new_y) =
            render_cover_letter_paragraph(&render_ctx, para_text, page_state.y, content_width, x);
        page_state.current_ops.extend(para_ops);
        page_state.y = new_y;
    }

    // Closing phrase
    let closing = closing_line.as_deref().unwrap_or(cl.closing_phrase_default);
    {
        page_state.y -= pt_to_mm(8.0);
        let (reg_id, _, _) = resolve_fonts(&fonts, template.fonts.body_family);
        let pos = Point {
            x: Mm(x).into(),
            y: Mm(page_state.y).into(),
        };
        page_state.current_ops.extend([
            Op::StartTextSection,
            Op::SetFillColor {
                col: colors.body.clone(),
            },
            Op::SetTextCursor { pos },
            Op::SetFont {
                font: PdfFontHandle::External(reg_id.clone()),
                size: Pt(template.body_pt),
            },
            Op::ShowText {
                items: vec![TextItem::Text(closing.to_string())],
            },
            Op::EndTextSection,
        ]);
        // 3 baselines of gap — room for a real signature
        page_state.y -= line_height + pt_to_mm(14.0);
    }

    // Signature block
    let sig_name = meta
        .and_then(|m| m.candidate_name.as_ref())
        .map(|s| s.as_str())
        .unwrap_or(name_text);
    let (sig_ops, new_y) = render_signature(
        template,
        &layout,
        &colors,
        &fonts,
        sig_name,
        signature_title.as_deref(),
        page_state.y,
    );
    page_state.current_ops.extend(sig_ops);
    page_state.y = new_y;

    assemble_pdf(doc, page_state, &layout)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Map a fixture `datePosition` string to the renderer's [`DatePosition`].
/// Returns `None` for an unrecognized value so the caller keeps the template default.
fn map_date_position(s: &str) -> Option<DatePosition> {
    match s {
        "top-right" => Some(DatePosition::TopRight),
        "below-header" => Some(DatePosition::BelowHeader),
        "above-salutation" => Some(DatePosition::AboveSalutation),
        _ => None,
    }
}

/// Simple date-line pattern — month names or 4-digit years.
static DATE_PATTERN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(
        r"(?i)\b(January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|Jun|Jul|Aug|Sep|Oct|Nov|Dec|\d{1,2}\s+\w+\s+\d{4}|\d{4})"
    ).unwrap()
});

fn assemble_pdf(
    mut doc: PdfDocument,
    page_state: PageState,
    layout: &LayoutConfig,
) -> Result<Vec<u8>> {
    let all_page_ops = page_state.finish();
    let pages: Vec<PdfPage> = all_page_ops
        .into_iter()
        .map(|ops| PdfPage::new(Mm(layout.page_width), Mm(layout.page_height), ops))
        .collect();
    let mut save_warnings = Vec::new();
    let pdf_bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut save_warnings);
    Ok(pdf_bytes)
}

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

pub fn generate_pdf(request: &ExportRequest) -> Result<Vec<u8>> {
    let template = Template::get(request.template_id);

    match request.document_type {
        DocumentType::Resume => {
            let text = extract_section(
                &request.text,
                "### CANDIDATE RESUME ###",
                Some("### JOB ADVERTISEMENT ###"),
            );
            let text = if text.is_empty() { &request.text } else { text };
            // Strangler-fig switch: the canonical layout engine renders resumes by
            // default now that snapshot parity is locked (`layout_pdf` is a default
            // feature). `--no-default-features` falls back to the legacy renderer,
            // which also stays the parity reference. Both arms compile via `cfg!`
            // so neither path rots.
            let result = if cfg!(feature = "layout_pdf") {
                super::layout_pdf::generate_resume_pdf_in(
                    text,
                    request.meta.as_ref(),
                    &template,
                    request.ats_mode,
                    request.page_geometry(),
                    request.contact.as_ref(),
                    &request.target_lang(),
                )
            } else {
                generate_resume_pdf(text, request.meta.as_ref(), &template, request.ats_mode)
            };
            result.context("Failed to generate resume PDF")
        }
        DocumentType::CoverLetter => {
            let text = extract_section(&request.text, "### COMPLETE COVER LETTER ###", None);
            let text = if text.is_empty() { &request.text } else { text };
            generate_cover_letter_pdf(
                text,
                request.meta.as_ref(),
                &template,
                request.contact.as_ref(),
                &request.target_lang(),
                request.locale.as_deref().unwrap_or("intl"),
            )
            .context("Failed to generate cover letter PDF")
        }
    }
}

#[cfg(test)]
mod test;
