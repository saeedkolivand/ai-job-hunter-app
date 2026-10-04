//! Rich-text → DOCX runs/hyperlinks, and the per-context run styling
//! ([`RunOpts`]) shared by the header contact line, body paragraphs, bold
//! entry titles, and italic subtitles.

use docx_rs::*;

use crate::export::docx_renderer::{docx_run_fonts, pt_to_half_points, DocxColors};
use crate::export::templates::Template;
use crate::export::types::FontFamily;
use crate::model::rich::RichText;
use crate::theme::LinkStyle;

/// Styling for a run of rich text. Built per context so the same `add_rich`
/// walker handles header contact, body, bold entry titles, and italic subtitles.
pub(super) struct RunOpts {
    size: usize,
    color: String,
    link_color: String,
    underline: bool,
    family: FontFamily,
    force_bold: bool,
    force_italic: bool,
}

impl RunOpts {
    fn link_color(colors: &DocxColors, link: LinkStyle) -> String {
        if link.use_accent {
            colors.emphasis.clone()
        } else {
            colors.body.clone()
        }
    }

    pub(super) fn contact(t: &Template, colors: &DocxColors) -> Self {
        let link = crate::theme::link_style(t.id);
        Self {
            size: pt_to_half_points(9.0),
            color: colors.date.clone(),
            link_color: Self::link_color(colors, link),
            underline: link.underline,
            family: t.fonts.body_family,
            force_bold: false,
            force_italic: false,
        }
    }

    pub(super) fn body(t: &Template, colors: &DocxColors, link: LinkStyle) -> Self {
        Self {
            size: pt_to_half_points(t.body_pt),
            color: colors.body.clone(),
            link_color: Self::link_color(colors, link),
            underline: link.underline,
            family: t.fonts.body_family,
            force_bold: false,
            force_italic: false,
        }
    }

    pub(super) fn entry_title(t: &Template, colors: &DocxColors) -> Self {
        let link = crate::theme::link_style(t.id);
        Self {
            size: pt_to_half_points(t.body_pt),
            color: colors.body.clone(),
            link_color: Self::link_color(colors, link),
            underline: link.underline,
            family: t.fonts.body_family,
            force_bold: true,
            force_italic: false,
        }
    }

    pub(super) fn subtitle(t: &Template, colors: &DocxColors) -> Self {
        let link = crate::theme::link_style(t.id);
        Self {
            size: pt_to_half_points(t.body_pt - 0.5),
            color: colors.date.clone(),
            link_color: Self::link_color(colors, link),
            underline: link.underline,
            family: t.fonts.body_family,
            force_bold: false,
            force_italic: t.job_title_italic,
        }
    }
}

pub(super) fn add_rich(mut para: Paragraph, rt: &RichText, opts: &RunOpts) -> Paragraph {
    for run in rt {
        let mut r = Run::new()
            .add_text(&run.text)
            .size(opts.size)
            .fonts(docx_run_fonts(opts.family));
        if run.bold || opts.force_bold {
            r = r.bold();
        }
        if run.italic || opts.force_italic {
            r = r.italic();
        }

        match &run.link {
            Some(url) => {
                r = r.color(opts.link_color.as_str());
                if opts.underline {
                    r = r.underline("single");
                }
                para = para
                    .add_hyperlink(Hyperlink::new(url.clone(), HyperlinkType::External).add_run(r));
            }
            None => {
                para = para.add_run(r.color(opts.color.as_str()));
            }
        }
    }
    para
}
