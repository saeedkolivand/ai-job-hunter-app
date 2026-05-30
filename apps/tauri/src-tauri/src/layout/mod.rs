//! Layout engine — turns a [`DocumentModel`] into a backend-agnostic
//! [`LaidOutDoc`] *display list*: pages of positioned text, link rectangles,
//! background fills, and rules, all in millimetres from each page's top-left.
//!
//! This is the format-agnostic core the spec calls for: the PDF backend (and,
//! later, DOCX) translates a `LaidOutDoc` to its own primitives and does **no**
//! layout itself. All horizontal geometry — wrapping, centering, right-aligned
//! dates, link-rect placement — uses real glyph advances from [`MeasureText`]
//! (the `measure/` layer), never a character-count estimate.
//!
//! Vertical rhythm matches the legacy renderer exactly. The single-column path
//! reproduces `templates::calculate_spacing` (the shared before/after gaps keyed
//! on line kind) plus the legacy render functions' internal offsets (the gap
//! above a section heading, between its rule and the first line, the entry
//! nudge), so each template keeps its intended spacing — the airiness comes from
//! the template's `line_spacing` flowing through the line height, exactly as
//! before. The two-column path keeps its own (newly written, reviewed) spacing.
//!
//! Both single-column and true multi-page two-column layouts are built from one
//! shared [`Flow`] (a paginating column cursor): a single-column document is one
//! full-width flow; a two-column document lays the header full width, then runs
//! an independent sidebar flow and main flow and merges their pages, drawing the
//! sidebar background band per page. Section → column assignment is the canonical
//! [`theme::placement_for`] decision.
//!
//! There is no consumer of this module's output by default; the PDF backend
//! (`export::layout_pdf`) renders through it under the `layout_pdf` feature.
#![allow(dead_code)]

use crate::export::templates::{calculate_spacing, SectionStyle, Template};
use crate::export::types::{FontFamily, LineKind};
use crate::locale::PageGeometry;
use crate::measure::MeasureText;
use crate::model::document::{Block, DocumentModel, EntryBlock, HeaderBlock, Placement, Section};
use crate::model::rich::{RichText, TextRun};
use crate::theme;

/// PostScript points per millimetre.
const PT_PER_MM: f32 = 2.834_645_7;

/// Top margin (baseline of the first line), in points — 0.75 in, matching the
/// existing renderer's `top_margin_pt`.
const TOP_MARGIN_PT: f32 = 54.0;

/// Contact line font size (pt) — the renderer draws the contact line smaller
/// than body copy.
const CONTACT_PT: f32 = 9.0;

/// Right-aligned entry date font size (pt).
const DATE_PT: f32 = 9.0;

/// Gap between the sidebar and main columns, in mm.
const COLUMN_GAP_MM: f32 = 5.0;

/// Horizontal padding inside the sidebar band, in mm.
const SIDEBAR_PAD_MM: f32 = 3.0;

// Legacy single-column internal offsets (points), mirrored from the legacy
// `pdf_renderer` render functions so the engine reproduces their vertical rhythm.
/// Extra gap above a section heading's baseline (legacy `y_before = y - 4`).
const SECTION_HEAD_LEAD_PT: f32 = 4.0;
/// Gap below a section heading's rule before the first content line.
const SECTION_HEAD_TRAIL_PT: f32 = 10.0;
/// Entry title nudge — legacy advances `line_height - 2pt` after a job entry.
const ENTRY_TITLE_NUDGE_PT: f32 = 2.0;
/// Trailing gap after the header rule before the first section/paragraph.
const HEADER_TRAIL_PT: f32 = 9.0;
/// Folded structural spacer (points) added before each section heading. The
/// legacy renderer drew a ~7pt blank-line spacer (3pt explicit + 4pt after-gap)
/// between a section and the content above it; the canonical model carries no
/// presentation blanks, so that height is folded in here deterministically to
/// keep the rendered space-before-section-header equal to legacy (~30pt).
const SECTION_SPACER_PT: f32 = 7.0;
/// Folded structural spacer (points) added between consecutive entries (the
/// legacy inter-entry blank line), so a job/education block is separated from the
/// previous one. Tuned so the engine's rendered inter-entry baseline gap equals
/// the legacy renderer's exactly: at a 2pt spacer the engine measured a uniform
/// 3pt short of legacy across all single-column templates, hence 5pt.
const ENTRY_SPACER_PT: f32 = 5.0;
/// Nudge (points) between the name and contact lines, matching legacy.
const NAME_CONTACT_NUDGE_PT: f32 = 2.0;
/// Nudge (points) between the contact line / header rule and the first content
/// line (summary), matching legacy.
const CONTACT_SUMMARY_NUDGE_PT: f32 = 3.0;
/// Bullet glyph horizontal offset from the content left, in mm.
const BULLET_GLYPH_DX_MM: f32 = 1.3;
/// Bullet text indent from the content left, in mm.
const BULLET_TEXT_DX_MM: f32 = 4.0;

fn pt_to_mm(pt: f32) -> f32 {
    pt / PT_PER_MM
}

// ─── Display-list output ──────────────────────────────────────────────────────

/// RGB color copied from the template; the backend maps it to its own type.
pub type Rgb = (u8, u8, u8);

/// A positioned single-line text fragment. `baseline_y_mm` is the text baseline,
/// measured in millimetres from the page top (y increases downward).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedText {
    pub x_mm: f32,
    pub baseline_y_mm: f32,
    pub text: String,
    pub family: FontFamily,
    pub bold: bool,
    pub italic: bool,
    pub size_pt: f32,
    pub color: Rgb,
}

/// A clickable hyperlink rectangle (`y_top_mm` is the top edge, mm from page top).
#[derive(Debug, Clone, PartialEq)]
pub struct LinkRect {
    pub x_mm: f32,
    pub y_top_mm: f32,
    pub width_mm: f32,
    pub height_mm: f32,
    pub url: String,
}

/// A filled background rectangle (e.g. a two-column sidebar band).
#[derive(Debug, Clone, PartialEq)]
pub struct FillRect {
    pub x_mm: f32,
    pub y_top_mm: f32,
    pub width_mm: f32,
    pub height_mm: f32,
    pub color: Rgb,
}

/// A horizontal rule (section underline / header hairline).
#[derive(Debug, Clone, PartialEq)]
pub struct RuleLine {
    pub x1_mm: f32,
    pub x2_mm: f32,
    pub y_mm: f32,
    pub thickness_pt: f32,
    pub color: Rgb,
}

/// One laid-out page. Backends draw fills first, then rules, then text, and
/// register links as annotations.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    pub fills: Vec<FillRect>,
    pub rules: Vec<RuleLine>,
    pub texts: Vec<PlacedText>,
    pub links: Vec<LinkRect>,
}

impl Page {
    /// Move every item from `other` into this page (used to merge column flows).
    fn absorb(&mut self, other: Page) {
        self.fills.extend(other.fills);
        self.rules.extend(other.rules);
        self.texts.extend(other.texts);
        self.links.extend(other.links);
    }
}

/// A fully laid-out document: fixed page geometry plus one display list per page.
#[derive(Debug, Clone, PartialEq)]
pub struct LaidOutDoc {
    pub page_width_mm: f32,
    pub page_height_mm: f32,
    pub pages: Vec<Page>,
}

// ─── Line wrapping (real advances) ────────────────────────────────────────────

/// Font and colors for placing a line of mixed runs. Bundled so [`place_line`]
/// stays within the argument limit.
#[derive(Debug, Clone, Copy)]
struct LineStyle {
    family: FontFamily,
    size_pt: f32,
    /// Color for normal runs.
    normal: Rgb,
    /// Color for bold runs (template emphasis).
    bold: Rgb,
}

/// A single word carrying its source run's formatting, for wrapping.
struct Word {
    text: String,
    bold: bool,
    italic: bool,
    link: Option<String>,
}

fn runs_to_words(runs: &[TextRun]) -> Vec<Word> {
    let mut words = Vec::new();
    for r in runs {
        for w in r.text.split_whitespace() {
            words.push(Word {
                text: w.to_string(),
                bold: r.bold,
                italic: r.italic,
                link: r.link.clone(),
            });
        }
    }
    words
}

/// Append `word` to the in-progress line, merging into the last run when the
/// formatting matches so a line stays as few runs as possible.
fn push_word(line: &mut RichText, word: &Word, space_before: bool) {
    let text = if space_before {
        format!(" {}", word.text)
    } else {
        word.text.clone()
    };
    if let Some(last) = line.last_mut() {
        if last.bold == word.bold && last.italic == word.italic && last.link == word.link {
            last.text.push_str(&text);
            return;
        }
    }
    line.push(TextRun {
        text,
        bold: word.bold,
        italic: word.italic,
        link: word.link.clone(),
    });
}

/// Greedily wrap `runs` to `max_width_mm`, measuring each word by its real glyph
/// advance (bold words are measured in the bold face). Words wider than the line
/// are kept whole (never split mid-word). Always returns at least one line.
fn wrap_runs(
    runs: &[TextRun],
    max_width_mm: f32,
    family: FontFamily,
    size_pt: f32,
    m: &dyn MeasureText,
) -> Vec<RichText> {
    let words = runs_to_words(runs);
    if words.is_empty() {
        return vec![Vec::new()];
    }
    let space_w = m.advance_mm(" ", family, false, size_pt);

    let mut lines: Vec<RichText> = Vec::new();
    let mut cur: RichText = Vec::new();
    let mut cur_w = 0.0_f32;

    for word in &words {
        let ww = m.advance_mm(&word.text, family, word.bold, size_pt);
        if cur.is_empty() {
            push_word(&mut cur, word, false);
            cur_w = ww;
        } else if cur_w + space_w + ww <= max_width_mm {
            push_word(&mut cur, word, true);
            cur_w += space_w + ww;
        } else {
            lines.push(std::mem::take(&mut cur));
            push_word(&mut cur, word, false);
            cur_w = ww;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

/// Total visible advance of a line of runs, in mm.
fn line_width(line: &RichText, family: FontFamily, size_pt: f32, m: &dyn MeasureText) -> f32 {
    line.iter()
        .map(|r| m.advance_mm(&r.text, family, r.bold, size_pt))
        .sum()
}

/// Place a line of runs onto `page` starting at `x_left` on baseline `baseline_y`.
/// Emits one [`PlacedText`] per run and a [`LinkRect`] over every linked run.
fn place_line(
    page: &mut Page,
    line: &RichText,
    x_left: f32,
    baseline_y: f32,
    style: LineStyle,
    m: &dyn MeasureText,
) {
    let mut x = x_left;
    let ascent = pt_to_mm(style.size_pt) * 0.8;
    let height = pt_to_mm(style.size_pt) * 1.1;
    for run in line {
        if run.text.is_empty() {
            continue;
        }
        let w = m.advance_mm(&run.text, style.family, run.bold, style.size_pt);
        page.texts.push(PlacedText {
            x_mm: x,
            baseline_y_mm: baseline_y,
            text: run.text.clone(),
            family: style.family,
            bold: run.bold,
            italic: run.italic,
            size_pt: style.size_pt,
            color: if run.bold { style.bold } else { style.normal },
        });
        if let Some(url) = &run.link {
            page.links.push(LinkRect {
                x_mm: x,
                y_top_mm: baseline_y - ascent,
                width_mm: w,
                height_mm: height,
                url: url.clone(),
            });
        }
        x += w;
    }
}

// ─── Header (full width, page 0 only) ─────────────────────────────────────────

/// A horizontal band a flow lays into: `left`..`right` on a page of `geom`.
#[derive(Debug, Clone, Copy)]
struct Frame {
    left: f32,
    right: f32,
    geom: PageGeometry,
}

impl Frame {
    fn width(&self) -> f32 {
        self.right - self.left
    }
}

fn rule_thickness(t: &Template) -> f32 {
    if t.rule_thickness > 0.0 {
        t.rule_thickness
    } else {
        0.5
    }
}

/// Lay the header (name, contact line, accent rule) into `page`, advancing `y`.
/// Spans the full content width (`frame`), so it is identical for single- and
/// two-column layouts. Returns nothing; `*y` ends just below the rule.
fn lay_header(
    page: &mut Page,
    y: &mut f32,
    h: &HeaderBlock,
    t: &Template,
    frame: Frame,
    m: &dyn MeasureText,
) {
    if !h.name.is_empty() {
        let adv = m.advance_mm(&h.name, t.fonts.name_family, true, t.name_pt);
        let x = if t.name_centered {
            (frame.geom.width_mm - adv) / 2.0
        } else {
            frame.left
        };
        page.texts.push(PlacedText {
            x_mm: x,
            baseline_y_mm: *y,
            text: h.name.clone(),
            family: t.fonts.name_family,
            bold: true,
            italic: false,
            size_pt: t.name_pt,
            color: t.name_color,
        });
        *y += pt_to_mm(t.name_pt) * 1.2 + pt_to_mm(NAME_CONTACT_NUDGE_PT);
    }

    if !h.contact.is_empty() {
        let fam = t.fonts.body_family;
        let total = line_width(&h.contact, fam, CONTACT_PT, m);
        let x = if t.name_centered {
            (frame.geom.width_mm - total) / 2.0
        } else {
            frame.left
        };
        let style = LineStyle {
            family: fam,
            size_pt: CONTACT_PT,
            normal: t.date_color,
            bold: t.date_color,
        };
        place_line(page, &h.contact, x, *y, style, m);
        *y += pt_to_mm(CONTACT_PT) * 1.2;
    }

    page.rules.push(RuleLine {
        x1_mm: frame.left,
        x2_mm: frame.right,
        y_mm: *y,
        thickness_pt: rule_thickness(t),
        color: t.rule_color,
    });
    *y += pt_to_mm(HEADER_TRAIL_PT) + pt_to_mm(CONTACT_SUMMARY_NUDGE_PT);
}

// ─── Flow: a paginating column cursor ─────────────────────────────────────────

/// A single paginating column. Block-laying logic lives here so single-column
/// (one full-width flow) and two-column (a sidebar flow + a main flow) share it.
///
/// `single_col` selects the vertical-spacing model: the single-column path
/// reproduces the legacy `calculate_spacing` gaps + internal offsets (so output
/// matches the legacy renderer); two-column keeps its own reviewed spacing.
struct Flow<'a> {
    t: &'a Template,
    m: &'a dyn MeasureText,
    frame: Frame,
    single_col: bool,
    top_baseline: f32,
    bottom_limit: f32,
    pages: Vec<Page>,
    cur: Page,
    /// Current baseline, mm from page top.
    y: f32,
    /// Previous line kind, for `calculate_spacing` (single-column only).
    prev_kind: Option<LineKind>,
}

impl<'a> Flow<'a> {
    /// New flow over `frame`, first baseline at `start_y`. `cur` is the page the
    /// caller may have already drawn into (e.g. the header on page 0).
    fn new(
        t: &'a Template,
        m: &'a dyn MeasureText,
        frame: Frame,
        single_col: bool,
        start_y: f32,
        cur: Page,
    ) -> Self {
        let margin = t.margin_in * 25.4;
        Self {
            t,
            m,
            frame,
            single_col,
            top_baseline: pt_to_mm(TOP_MARGIN_PT),
            bottom_limit: frame.geom.height_mm - margin,
            pages: Vec::new(),
            cur,
            y: start_y,
            prev_kind: None,
        }
    }

    fn body_line(&self) -> f32 {
        pt_to_mm(self.t.body_pt) * self.t.line_spacing
    }

    fn would_overflow(&self, needed: f32) -> bool {
        self.y + needed > self.bottom_limit
    }

    fn new_page(&mut self) {
        self.pages.push(std::mem::take(&mut self.cur));
        self.y = self.top_baseline;
    }

    /// `calculate_spacing` (before, after) gaps in mm for `kind`, using the
    /// previous kind. Two-column passes `None` (matching the legacy two-column
    /// loop, which never tracked the previous kind).
    fn gaps_mm(&self, kind: LineKind) -> (f32, f32) {
        let prev = if self.single_col {
            self.prev_kind.as_ref()
        } else {
            None
        };
        let (before, after) = calculate_spacing(&kind, prev);
        (pt_to_mm(before), pt_to_mm(after))
    }

    fn lay_sections(&mut self, sections: &[&Section]) {
        for s in sections {
            self.lay_section(s);
        }
    }

    fn lay_section(&mut self, s: &Section) {
        if !s.heading.is_empty() {
            self.lay_heading(&s.heading);
        }
        for block in &s.blocks {
            match block {
                Block::Paragraph(rt) => self.lay_paragraph(rt),
                Block::Bullet(rt) => self.lay_bullet(rt),
                Block::Entry(e) => self.lay_entry(e),
            }
        }
    }

    fn lay_heading(&mut self, heading: &str) {
        let t = self.t;
        let (text, size) = if t.section_small_caps {
            (heading.to_uppercase(), t.section_pt * 0.85)
        } else if t.section_all_caps {
            (heading.to_uppercase(), t.section_pt)
        } else {
            (heading.to_string(), t.section_pt)
        };
        let head_h = pt_to_mm(size) * 1.2;

        if self.single_col {
            // Legacy: 12pt before-gap, then `y_before = y - 4`, header, rule,
            // then `-10`, then 3pt after-gap.
            let (before_base, after) = self.gaps_mm(LineKind::SectionHeader);
            // Fold the legacy blank-line spacer into the deterministic before-gap
            // so the rendered space-before-section-header matches legacy (~30pt).
            let before = before_base + pt_to_mm(SECTION_SPACER_PT);
            let lead = pt_to_mm(SECTION_HEAD_LEAD_PT);
            let trail = pt_to_mm(SECTION_HEAD_TRAIL_PT);
            // Orphan guard: keep the heading with at least two body lines below it.
            let needed = lead + head_h + trail + self.body_line() * 2.0;
            if self.would_overflow(before + needed) && !self.cur.texts.is_empty() {
                self.new_page();
            } else {
                self.y += before;
            }
            self.y += lead;
            self.place_heading(&text, size);
            self.y += head_h;
            self.push_heading_rule();
            self.y += trail + after;
            self.prev_kind = Some(LineKind::SectionHeader);
        } else {
            // Two-column: reviewed spacing — template gap before, 6pt after.
            let before = pt_to_mm(t.section_spacing_before);
            let needed = before + head_h + self.body_line() * 2.0;
            if self.would_overflow(needed) && !self.cur.texts.is_empty() {
                self.new_page();
            } else {
                self.y += before;
            }
            self.place_heading(&text, size);
            self.y += head_h;
            self.push_heading_rule();
            self.y += pt_to_mm(6.0);
        }
    }

    /// Emit the heading text at the current baseline.
    fn place_heading(&mut self, text: &str, size: f32) {
        let t = self.t;
        self.cur.texts.push(PlacedText {
            x_mm: self.frame.left,
            baseline_y_mm: self.y,
            text: text.to_string(),
            family: t.fonts.heading_family,
            bold: true,
            italic: false,
            size_pt: size,
            color: t.section_color,
        });
    }

    /// Emit the section underline rule at the current baseline if the template
    /// uses one.
    fn push_heading_rule(&mut self) {
        let t = self.t;
        match t.section_style {
            SectionStyle::RuledBottom | SectionStyle::Underline => {
                self.cur.rules.push(RuleLine {
                    x1_mm: self.frame.left,
                    x2_mm: self.frame.right,
                    y_mm: self.y,
                    thickness_pt: rule_thickness(t),
                    color: t.rule_color,
                });
            }
            SectionStyle::BoldOnly => {}
        }
    }

    fn lay_paragraph(&mut self, rt: &RichText) {
        if rt.iter().all(|r| r.text.trim().is_empty()) {
            return;
        }
        let t = self.t;
        let m = self.m;
        let fam = t.fonts.body_family;
        let style = LineStyle {
            family: fam,
            size_pt: t.body_pt,
            normal: t.body_color,
            bold: t.emphasis_color,
        };
        // Text kind: legacy (0, 4); the two-column path is the same trailing 4pt.
        let (before, after) = if self.single_col {
            self.gaps_mm(LineKind::Text)
        } else {
            (0.0, pt_to_mm(4.0))
        };
        self.y += before;
        for line in wrap_runs(rt, self.frame.width(), fam, t.body_pt, m) {
            if self.would_overflow(self.body_line()) {
                self.new_page();
            }
            place_line(&mut self.cur, &line, self.frame.left, self.y, style, m);
            self.y += self.body_line();
        }
        self.y += after;
        if self.single_col {
            self.prev_kind = Some(LineKind::Text);
        }
    }

    fn lay_bullet(&mut self, rt: &RichText) {
        let t = self.t;
        let m = self.m;
        let fam = t.fonts.body_family;
        let text_x = self.frame.left + BULLET_TEXT_DX_MM;
        let wrap_w = (self.frame.right - text_x).max(10.0);
        let style = LineStyle {
            family: fam,
            size_pt: t.body_pt,
            normal: t.body_color,
            bold: t.emphasis_color,
        };
        let (before, after) = if self.single_col {
            self.gaps_mm(LineKind::Bullet)
        } else {
            (0.0, pt_to_mm(2.0))
        };
        self.y += before;
        for (i, line) in wrap_runs(rt, wrap_w, fam, t.body_pt, m).iter().enumerate() {
            if self.would_overflow(self.body_line()) {
                self.new_page();
            }
            if i == 0 {
                self.cur.texts.push(PlacedText {
                    x_mm: self.frame.left + BULLET_GLYPH_DX_MM,
                    baseline_y_mm: self.y,
                    text: "•".to_string(),
                    family: fam,
                    bold: false,
                    italic: false,
                    size_pt: t.body_pt,
                    color: t.body_color,
                });
            }
            place_line(&mut self.cur, line, text_x, self.y, style, m);
            self.y += self.body_line();
        }
        self.y += after;
        if self.single_col {
            self.prev_kind = Some(LineKind::Bullet);
        }
    }

    fn lay_entry(&mut self, e: &EntryBlock) {
        let t = self.t;
        let m = self.m;
        let fam = t.fonts.body_family;
        let style = LineStyle {
            family: fam,
            size_pt: t.body_pt,
            normal: t.body_color,
            bold: t.emphasis_color,
        };

        // Entry title (legacy JobEntry: before 6/8, advance line_height - 2, after 1).
        let (before, after) = if self.single_col {
            let (b, a) = self.gaps_mm(LineKind::JobEntry);
            // Inter-entry: fold the legacy blank-line spacer when this entry
            // follows another entry's content (its bullets / subtitle), so the
            // rendered inter-entry gap matches legacy (~18pt). The first entry
            // after a section heading (prev = SectionHeader) gets no extra spacer.
            let spacer = match self.prev_kind {
                Some(LineKind::Bullet) | Some(LineKind::JobTitle) => pt_to_mm(ENTRY_SPACER_PT),
                _ => 0.0,
            };
            (b + spacer, a)
        } else {
            (0.0, 0.0)
        };
        // Keep the title with at least its subtitle / first bullet.
        if self.would_overflow(before + self.body_line() * 2.0) && !self.cur.texts.is_empty() {
            self.new_page();
        } else {
            self.y += before;
        }
        place_line(&mut self.cur, &e.title, self.frame.left, self.y, style, m);
        if let Some(date) = &e.date {
            let adv = m.advance_mm(date, fam, false, DATE_PT);
            self.cur.texts.push(PlacedText {
                x_mm: self.frame.right - adv,
                baseline_y_mm: self.y,
                text: date.clone(),
                family: fam,
                bold: false,
                italic: false,
                size_pt: DATE_PT,
                color: t.date_color,
            });
        }
        if self.single_col {
            self.y += self.body_line() - pt_to_mm(ENTRY_TITLE_NUDGE_PT) + after;
            self.prev_kind = Some(LineKind::JobEntry);
        } else {
            self.y += self.body_line();
        }

        if let Some(sub) = &e.subtitle {
            let (sb, sa) = if self.single_col {
                self.gaps_mm(LineKind::JobTitle)
            } else {
                (0.0, 0.0)
            };
            self.y += sb;
            if self.would_overflow(self.body_line()) {
                self.new_page();
            }
            let sub_text: String = sub.iter().map(|r| r.text.as_str()).collect();
            self.cur.texts.push(PlacedText {
                x_mm: self.frame.left,
                baseline_y_mm: self.y,
                text: sub_text,
                family: fam,
                bold: false,
                italic: t.job_title_italic,
                size_pt: t.body_pt - 0.5,
                color: t.date_color,
            });
            self.y += self.body_line() + sa;
            if self.single_col {
                self.prev_kind = Some(LineKind::JobTitle);
            }
        }

        for bullet in &e.bullets {
            self.lay_bullet(bullet);
        }

        // Two-column keeps its reviewed trailing gap after an entry; single-column
        // relies on the next element's before-gap (matching legacy).
        if !self.single_col {
            self.y += pt_to_mm(3.0);
        }
    }

    /// Flush the in-progress page and return every page this flow produced.
    fn into_pages(mut self) -> Vec<Page> {
        self.pages.push(std::mem::take(&mut self.cur));
        self.pages
    }
}

// ─── Entry points ─────────────────────────────────────────────────────────────

/// Lay a resume [`DocumentModel`] onto `geom` using `template`'s styling and real
/// font metrics, producing a [`LaidOutDoc`]. Two-column when the template defines
/// a two-column config, otherwise single-column.
pub fn layout_document(
    model: &DocumentModel,
    template: &Template,
    geom: PageGeometry,
    m: &dyn MeasureText,
) -> LaidOutDoc {
    if template.two_column.is_some() {
        layout_two_column(model, template, geom, m)
    } else {
        layout_single_column(model, template, geom, m)
    }
}

fn layout_single_column(
    model: &DocumentModel,
    t: &Template,
    geom: PageGeometry,
    m: &dyn MeasureText,
) -> LaidOutDoc {
    let margin = t.margin_in * 25.4;
    let frame = Frame {
        left: margin,
        right: geom.width_mm - margin,
        geom,
    };

    let mut page0 = Page::default();
    let mut y = pt_to_mm(TOP_MARGIN_PT);
    lay_header(&mut page0, &mut y, &model.header, t, frame, m);

    let mut flow = Flow::new(t, m, frame, true, y, page0);
    for s in &model.sections {
        flow.lay_section(s);
    }

    LaidOutDoc {
        page_width_mm: geom.width_mm,
        page_height_mm: geom.height_mm,
        pages: flow.into_pages(),
    }
}

fn layout_two_column(
    model: &DocumentModel,
    t: &Template,
    geom: PageGeometry,
    m: &dyn MeasureText,
) -> LaidOutDoc {
    let tc = t
        .two_column
        .as_ref()
        .expect("layout_two_column requires a two-column config");
    let margin = t.margin_in * 25.4;
    let content_w = geom.width_mm - 2.0 * margin;
    let sidebar_w = content_w * tc.sidebar_width_ratio;
    let sidebar_left = margin;
    let main_left = margin + sidebar_w + COLUMN_GAP_MM;

    let full_frame = Frame {
        left: margin,
        right: geom.width_mm - margin,
        geom,
    };

    // Header spans the full width on page 0.
    let mut page0 = Page::default();
    let mut y = pt_to_mm(TOP_MARGIN_PT);
    lay_header(&mut page0, &mut y, &model.header, t, full_frame, m);
    let header_bottom = y;

    // Split sections by the canonical placement decision.
    let mut main_sections: Vec<&Section> = Vec::new();
    let mut sidebar_sections: Vec<&Section> = Vec::new();
    for s in &model.sections {
        match theme::placement_for(&s.id) {
            Placement::Sidebar => sidebar_sections.push(s),
            Placement::Main => main_sections.push(s),
        }
    }

    let sidebar_frame = Frame {
        left: sidebar_left + SIDEBAR_PAD_MM,
        right: sidebar_left + sidebar_w - SIDEBAR_PAD_MM,
        geom,
    };
    let main_frame = Frame {
        left: main_left,
        right: geom.width_mm - margin,
        geom,
    };

    let mut sidebar_flow = Flow::new(t, m, sidebar_frame, false, header_bottom, Page::default());
    sidebar_flow.lay_sections(&sidebar_sections);
    let sidebar_pages = sidebar_flow.into_pages();

    let mut main_flow = Flow::new(t, m, main_frame, false, header_bottom, Page::default());
    main_flow.lay_sections(&main_sections);
    let main_pages = main_flow.into_pages();

    // Merge page-for-page, drawing the sidebar band behind each page's columns.
    let bottom_limit = geom.height_mm - margin;
    let n = sidebar_pages.len().max(main_pages.len());
    let mut main_it = main_pages.into_iter();
    let mut side_it = sidebar_pages.into_iter();
    let mut pages = Vec::with_capacity(n);

    for i in 0..n {
        let mut page = if i == 0 {
            std::mem::take(&mut page0)
        } else {
            Page::default()
        };
        let band_top = if i == 0 {
            header_bottom - pt_to_mm(t.body_pt)
        } else {
            pt_to_mm(TOP_MARGIN_PT) - pt_to_mm(t.body_pt)
        };
        // Band first so it sits behind the column text.
        page.fills.insert(
            0,
            FillRect {
                x_mm: sidebar_left,
                y_top_mm: band_top,
                width_mm: sidebar_w,
                height_mm: (bottom_limit - band_top).max(0.0),
                color: tc.sidebar_bg_color,
            },
        );
        if let Some(mp) = main_it.next() {
            page.absorb(mp);
        }
        if let Some(sp) = side_it.next() {
            page.absorb(sp);
        }
        pages.push(page);
    }

    LaidOutDoc {
        page_width_mm: geom.width_mm,
        page_height_mm: geom.height_mm,
        pages,
    }
}

#[cfg(test)]
mod tests;
