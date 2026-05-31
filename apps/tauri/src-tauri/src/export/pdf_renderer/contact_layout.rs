//! Contact-line geometry: clickable-rect placement and separator-aware wrapping
//! for the header contact line, shared by the résumé layout and the legacy
//! cover-letter letterhead. Split out of `pdf_renderer/mod.rs` to keep that module
//! under the architecture LOC cap (R8).

use crate::export::links::{display_text, split_urls, Span};
use crate::export::types::FontFamily;
use crate::measure::MeasureText;

use super::text_advance_mm;

/// A clickable link rectangle in page millimetres (x measured from the page left).
pub(crate) struct ContactLinkRect {
    pub x_left_mm: f32,
    pub width_mm: f32,
    pub url: String,
}

/// Exact clickable rects for every link in a contact line. Walks the line's spans
/// accumulating the *visible* text, so each link's left edge is the real advance of
/// everything drawn before it and its width is the real advance of the label —
/// overlapping the glyphs the PDF engine actually paints. Replaces the per-character
/// `× 0.52` estimate, which also mis-measured multi-byte text (it counted bytes).
pub(crate) fn contact_link_rects(
    text: &str,
    x_start_mm: f32,
    family: FontFamily,
    font_size: f32,
    m: &dyn MeasureText,
) -> Vec<ContactLinkRect> {
    let mut out = Vec::new();
    let mut visible = String::new();
    for span in split_urls(text) {
        match span {
            Span::Text(t) => visible.push_str(&t),
            Span::Link { label, url } => {
                let x_left_mm = x_start_mm + m.advance_mm(&visible, family, false, font_size);
                let width_mm = m.advance_mm(&label, family, false, font_size);
                out.push(ContactLinkRect {
                    x_left_mm,
                    width_mm,
                    url,
                });
                visible.push_str(&label);
            }
        }
    }
    out
}

/// Greedily wrap a contact-line markdown string into multiple lines that each fit
/// `max_width_mm`, splitting only on its separator (`" | "` or `" · "`) so a part
/// (a label, an email, a `[Label](url)` link) is never broken apart. Measures the
/// *visible* width (links collapsed to their label). Returns the original string
/// as a single line when it has no separator or already fits.
pub(crate) fn wrap_contact_markdown(
    text: &str,
    max_width_mm: f32,
    family: FontFamily,
    font_size: f32,
) -> Vec<String> {
    let sep = if text.contains(" | ") {
        " | "
    } else if text.contains(" · ") {
        " · "
    } else {
        return vec![text.to_string()];
    };
    let sep_w = text_advance_mm(&display_text(sep), family, false, font_size);

    let mut lines: Vec<String> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut cur_w = 0.0_f32;
    for part in text.split(sep) {
        let pw = text_advance_mm(&display_text(part), family, false, font_size);
        if cur.is_empty() {
            cur.push(part);
            cur_w = pw;
        } else if cur_w + sep_w + pw <= max_width_mm {
            cur.push(part);
            cur_w += sep_w + pw;
        } else {
            lines.push(cur.join(sep));
            cur = vec![part];
            cur_w = pw;
        }
    }
    if !cur.is_empty() {
        lines.push(cur.join(sep));
    }
    if lines.is_empty() {
        lines.push(text.to_string());
    }
    lines
}
