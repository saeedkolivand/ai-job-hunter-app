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
//! Phase 2c introduces the engine for the **single-column** path with no
//! consumer yet (additive, behind the module's `dead_code` allow, like the other
//! migration foundations). True multi-page two-column layout is layered on in
//! the next phase; the PDF backend is wired onto this — behind a feature flag,
//! with golden-snapshot parity — after that.
#![allow(dead_code)]

use crate::export::templates::{SectionStyle, Template};
use crate::export::types::FontFamily;
use crate::locale::PageGeometry;
use crate::measure::MeasureText;
use crate::model::document::{Block, DocumentModel, EntryBlock, HeaderBlock, Section};
use crate::model::rich::{RichText, TextRun};

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

// ─── The engine ───────────────────────────────────────────────────────────────

/// Lay a resume [`DocumentModel`] onto `geom` using `template`'s styling and real
/// font metrics, producing a single-column [`LaidOutDoc`].
pub fn layout_document(
    model: &DocumentModel,
    template: &Template,
    geom: PageGeometry,
    m: &dyn MeasureText,
) -> LaidOutDoc {
    let mut engine = Engine::new(template, geom, m);
    engine.lay_header(&model.header);
    for section in &model.sections {
        engine.lay_section(section);
    }
    engine.finish()
}

struct Engine<'a> {
    t: &'a Template,
    m: &'a dyn MeasureText,
    geom: PageGeometry,
    content_left: f32,
    content_right: f32,
    content_width: f32,
    top_baseline: f32,
    bottom_limit: f32,
    pages: Vec<Page>,
    cur: Page,
    /// Current baseline, mm from page top.
    y: f32,
}

impl<'a> Engine<'a> {
    fn new(t: &'a Template, geom: PageGeometry, m: &'a dyn MeasureText) -> Self {
        let margin = t.margin_in * 25.4;
        let top_baseline = pt_to_mm(TOP_MARGIN_PT);
        Self {
            t,
            m,
            geom,
            content_left: margin,
            content_right: geom.width_mm - margin,
            content_width: geom.width_mm - 2.0 * margin,
            top_baseline,
            bottom_limit: geom.height_mm - margin,
            pages: Vec::new(),
            cur: Page::default(),
            y: top_baseline,
        }
    }

    /// One body line's vertical advance in mm (point size × line spacing).
    fn body_line(&self) -> f32 {
        pt_to_mm(self.t.body_pt) * self.t.line_spacing
    }

    /// True when placing `needed` more mm would cross the bottom margin.
    fn would_overflow(&self, needed: f32) -> bool {
        self.y + needed > self.bottom_limit
    }

    fn new_page(&mut self) {
        self.pages.push(std::mem::take(&mut self.cur));
        self.y = self.top_baseline;
    }

    fn rule_thickness(&self) -> f32 {
        if self.t.rule_thickness > 0.0 {
            self.t.rule_thickness
        } else {
            0.5
        }
    }

    fn lay_header(&mut self, h: &HeaderBlock) {
        let t = self.t;
        let m = self.m;

        if !h.name.is_empty() {
            let adv = m.advance_mm(&h.name, t.fonts.name_family, true, t.name_pt);
            let x = if t.name_centered {
                (self.geom.width_mm - adv) / 2.0
            } else {
                self.content_left
            };
            self.cur.texts.push(PlacedText {
                x_mm: x,
                baseline_y_mm: self.y,
                text: h.name.clone(),
                family: t.fonts.name_family,
                bold: true,
                italic: false,
                size_pt: t.name_pt,
                color: t.name_color,
            });
            self.y += pt_to_mm(t.name_pt) * 1.2;
        }

        if !h.contact.is_empty() {
            let fam = t.fonts.body_family;
            let total = line_width(&h.contact, fam, CONTACT_PT, m);
            let x = if t.name_centered {
                (self.geom.width_mm - total) / 2.0
            } else {
                self.content_left
            };
            let style = LineStyle {
                family: fam,
                size_pt: CONTACT_PT,
                normal: t.date_color,
                bold: t.date_color,
            };
            place_line(&mut self.cur, &h.contact, x, self.y, style, m);
            self.y += pt_to_mm(CONTACT_PT) * 1.2;
        }

        self.cur.rules.push(RuleLine {
            x1_mm: self.content_left,
            x2_mm: self.content_right,
            y_mm: self.y,
            thickness_pt: self.rule_thickness(),
            color: t.rule_color,
        });
        self.y += pt_to_mm(9.0);
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
        let before = pt_to_mm(t.section_spacing_before);
        // Orphan guard: keep the heading with at least two body lines below it.
        let needed = before + pt_to_mm(t.section_pt) * 1.2 + self.body_line() * 2.0;
        if self.would_overflow(needed) && !self.cur.texts.is_empty() {
            self.new_page();
        } else {
            self.y += before;
        }

        let (text, size) = if t.section_small_caps {
            (heading.to_uppercase(), t.section_pt * 0.85)
        } else if t.section_all_caps {
            (heading.to_uppercase(), t.section_pt)
        } else {
            (heading.to_string(), t.section_pt)
        };

        self.cur.texts.push(PlacedText {
            x_mm: self.content_left,
            baseline_y_mm: self.y,
            text,
            family: t.fonts.heading_family,
            bold: true,
            italic: false,
            size_pt: size,
            color: t.section_color,
        });
        self.y += pt_to_mm(size) * 1.2;

        match t.section_style {
            SectionStyle::RuledBottom | SectionStyle::Underline => {
                self.cur.rules.push(RuleLine {
                    x1_mm: self.content_left,
                    x2_mm: self.content_right,
                    y_mm: self.y,
                    thickness_pt: self.rule_thickness(),
                    color: t.rule_color,
                });
            }
            SectionStyle::BoldOnly => {}
        }
        self.y += pt_to_mm(6.0);
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
        for line in wrap_runs(rt, self.content_width, fam, t.body_pt, m) {
            if self.would_overflow(self.body_line()) {
                self.new_page();
            }
            place_line(&mut self.cur, &line, self.content_left, self.y, style, m);
            self.y += self.body_line();
        }
        self.y += pt_to_mm(4.0);
    }

    fn lay_bullet(&mut self, rt: &RichText) {
        let t = self.t;
        let m = self.m;
        let fam = t.fonts.body_family;
        let indent = 4.0_f32;
        let text_x = self.content_left + indent;
        let wrap_w = (self.content_right - text_x).max(10.0);
        let style = LineStyle {
            family: fam,
            size_pt: t.body_pt,
            normal: t.body_color,
            bold: t.emphasis_color,
        };
        for (i, line) in wrap_runs(rt, wrap_w, fam, t.body_pt, m).iter().enumerate() {
            if self.would_overflow(self.body_line()) {
                self.new_page();
            }
            if i == 0 {
                self.cur.texts.push(PlacedText {
                    x_mm: self.content_left + 1.3,
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
        self.y += pt_to_mm(2.0);
    }

    fn lay_entry(&mut self, e: &EntryBlock) {
        let t = self.t;
        let m = self.m;
        let fam = t.fonts.body_family;

        // Keep the title with at least its subtitle / first bullet.
        if self.would_overflow(self.body_line() * 2.0) && !self.cur.texts.is_empty() {
            self.new_page();
        }

        let style = LineStyle {
            family: fam,
            size_pt: t.body_pt,
            normal: t.body_color,
            bold: t.emphasis_color,
        };
        place_line(&mut self.cur, &e.title, self.content_left, self.y, style, m);
        if let Some(date) = &e.date {
            let adv = m.advance_mm(date, fam, false, DATE_PT);
            self.cur.texts.push(PlacedText {
                x_mm: self.content_right - adv,
                baseline_y_mm: self.y,
                text: date.clone(),
                family: fam,
                bold: false,
                italic: false,
                size_pt: DATE_PT,
                color: t.date_color,
            });
        }
        self.y += self.body_line();

        if let Some(sub) = &e.subtitle {
            if self.would_overflow(self.body_line()) {
                self.new_page();
            }
            let sub_text: String = sub.iter().map(|r| r.text.as_str()).collect();
            self.cur.texts.push(PlacedText {
                x_mm: self.content_left,
                baseline_y_mm: self.y,
                text: sub_text,
                family: fam,
                bold: false,
                italic: t.job_title_italic,
                size_pt: t.body_pt - 0.5,
                color: t.date_color,
            });
            self.y += self.body_line();
        }

        for bullet in &e.bullets {
            self.lay_bullet(bullet);
        }
        self.y += pt_to_mm(3.0);
    }

    fn finish(mut self) -> LaidOutDoc {
        self.pages.push(self.cur);
        LaidOutDoc {
            page_width_mm: self.geom.width_mm,
            page_height_mm: self.geom.height_mm,
            pages: self.pages,
        }
    }
}

#[cfg(test)]
mod tests;
