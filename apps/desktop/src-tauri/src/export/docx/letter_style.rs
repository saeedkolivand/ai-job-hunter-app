//! Per-layout DOCX styling decisions for the non-Classic cover-letter
//! renderer ([`super::letter_layout`]), plus the shared "header band" tint
//! both that renderer and [`super::super::model_docx`]'s Awesome résumé
//! header call.
//!
//! Why [`LetterDocxStyle`] exists: the renderer used to carry a single
//! `is_refined` boolean and let everything else fall through to Banded's
//! treatment. Adding `Navy` therefore silently gave it Banded's shaded band,
//! footer rule and bolding while stripping Refined's title and subject
//! caption — a letter that rendered as Navy in PDF and Banded in DOCX. Three
//! review rounds each found a different surviving branch, including two
//! contact paths that disagreed with each other depending on whether a
//! `ContactProfile` happened to be attached.
//!
//! Every field is filled in per layout in [`LetterDocxStyle::for_layout`], so
//! a new layout cannot inherit another's look by omission — it has to say
//! what it does. Fields are `pub(super)` (not private) because the orchestrator
//! in [`super::letter_layout`] reads them directly.

use docx_rs::AlignmentType;

use crate::export::types::LetterLayout;

use super::super::docx_renderer::rgb_to_hex;

pub(super) struct LetterDocxStyle {
    /// Name in caps (Banded, Navy) vs. as written (Refined).
    pub(super) uppercase_name: bool,
    /// Points added to the template's name size.
    pub(super) name_pt_bonus: f32,
    /// Shaded block behind the name (Banded's band, Sidebar's rail).
    /// Decorative — suppressed under ATS mode.
    pub(super) header_band: bool,
    /// Shaded run carrying the letterhead initials at the head of the name
    /// paragraph (Monogram only). Decorative — suppressed under ATS mode, which
    /// is also what `letter_monogram.typ` does with the device it approximates.
    pub(super) monogram_device: bool,
    /// Name/title centred (Navy only).
    pub(super) centred_letterhead: bool,
    /// Contact-line alignment. BOTH contact paths — profile-backed and the
    /// no-profile fallback — read this ONE field, so they cannot drift apart
    /// again; they did once, and the same letter then rendered differently
    /// depending on whether a `ContactProfile` happened to be attached. It is a
    /// stored value rather than a function of `centred_letterhead` because
    /// Sidebar and Monogram left-align the contact while leaving the name
    /// left-aligned too — a derived "centred or right" could not express that.
    pub(super) contact_align: AlignmentType,
    /// Role line under the name (Refined, Navy).
    pub(super) shows_title: bool,
    /// Role line rendered uppercase + letter-spaced in the accent colour
    /// (Refined), vs. plain case in the muted date colour (Navy). Presence and
    /// STYLE are separate decisions: the first pass made `shows_title` layout
    /// aware but left the styling hardcoded to Refined's, so Navy's DOCX title
    /// was accent/uppercase/tracked while its `.typ` renders muted plain text.
    pub(super) title_emphasised: bool,
    /// Subject caption colour: accent for Refined (`letter_refined.typ` uses
    /// `c-accent`), the name colour for Navy (`letter_navy.typ` uses `c-name`).
    pub(super) caption_uses_name_colour: bool,
    /// Small-caps subject caption (Refined, Navy).
    pub(super) shows_subject_caption: bool,
    /// Bold date + recipient blocks (Banded only — see each `.typ`'s emit blocks).
    pub(super) bolds_addressing: bool,
    /// Rule under the header block (Refined, Navy).
    pub(super) header_rule: bool,
    /// Short rule at the foot of the letter (Banded only).
    pub(super) footer_rule: bool,
    /// Empty paragraphs before the signature (Refined only).
    pub(super) signature_gap: bool,
    /// Space after the contact paragraph, in twentieths of a point.
    pub(super) contact_space_after: u32,
    /// Space after the sign-off, in twentieths of a point.
    pub(super) signoff_space_after: u32,
}

impl LetterDocxStyle {
    pub(super) fn for_layout(layout: LetterLayout) -> Self {
        match layout {
            // Classic never reaches this renderer (see the debug_assert in
            // `letter_layout::generate_cover_letter_docx_layout`); it is listed
            // so the match stays exhaustive and a future layout fails to
            // compile until someone states its style.
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

/// Shading fill for every DOCX "header band" approximation: the template
/// accent lightened 85 % toward white.
///
/// DOCX has no page-background primitive, so a PDF band becomes paragraph
/// shading — and paragraph shading is only legible behind the normal dark ink
/// if the fill is a pale tint. Both users ([`super::letter_layout`]'s Banded
/// cover letter and [`super::super::model_docx`]'s Awesome résumé header) call
/// this, so the two can never drift to different tints of the same accent.
pub(in crate::export) fn band_tint_hex(accent: (u8, u8, u8)) -> String {
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
