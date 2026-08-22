//! Adapter: build a canonical [`DocumentModel`] from already-parsed resume text.
//!
//! Temporary strangler-fig bridge. It reuses the existing line-based
//! [`parser::parse_resume`](crate::export::parser::parse_resume) so the new model
//! (and the layout engine that will consume it in Phase 2b) can be populated
//! WITHOUT yet touching extraction. The flat `Vec<ParsedLine>` is regrouped into
//! the structured shape the model expects: a header plus titled sections whose
//! content is paragraphs, standalone bullets, and entries (a title line with an
//! optional subtitle / date and its own bullets).
//!
//! Faithfulness: this mirrors what `parse_resume` already recognizes — it does
//! not re-classify or enrich. Inline formatting is recovered with
//! [`tokenize_rich`], so links survive as first-class runs (and bold survives
//! wherever the parser preserved it). Content is never dropped: unrecognized or
//! out-of-place lines fall back to paragraphs. The one exception is a heading
//! with nothing beneath it — a heading the source or a generator wrote and
//! never filled in carries no content to lose, and [`push_nonempty_section`]
//! drops it rather than let it render as a visibly empty section. The
//! structured extractor that builds the model directly (skipping this text
//! round-trip) arrives in Phase 6.
//!
//! Resumes only. Cover letters have a fundamentally different shape (letterhead,
//! date, recipient, salutation, body, closing, signature) and stay on the legacy
//! `export::pdf` path until a later phase models them explicitly.

use crate::export::parser::{is_project_stack_shaped, is_project_title_shaped, parse_resume};
use crate::export::types::{DocumentType, LineKind, ParsedLine};

use super::document::{Block, DocumentModel, EntryBlock, HeaderBlock, Section, SectionId};
use super::rich::tokenize_rich;

/// A short role/headline line (1–6 words, ≤60 chars, no terminal sentence
/// punctuation) that sits directly after the name/contact and before any
/// section — the candidate's professional title. Rejects summary prose (long or
/// ends in punctuation), known section headings, and all-caps banners, so real
/// content is never mistaken for the title.
fn is_title_like(text: &str) -> bool {
    let words = text.split_whitespace().count();
    if !(1..=6).contains(&words) || text.chars().count() > 60 || text.ends_with(['.', '!', '?']) {
        return false;
    }
    // Never promote a line that classifies as a known (non-Custom) section
    // heading (e.g. "Skills", "Certifications") — that's section content, not a title.
    if !matches!(SectionId::from_header(text), SectionId::Custom(_)) {
        return false;
    }
    // Reject all-caps multi-word banners ("KEY HIGHLIGHTS"): a heading, not a title.
    let alpha: String = text.chars().filter(|c| c.is_alphabetic()).collect();
    if words >= 2 && !alpha.is_empty() && alpha == alpha.to_uppercase() {
        return false;
    }
    true
}

/// Push a finished block into the active section, or into the preamble when no
/// section has started yet (leading content under no heading).
fn push_block(block: Block, current: &mut Option<Section>, preamble: &mut Vec<Block>) {
    match current {
        Some(section) => section.blocks.push(block),
        None => preamble.push(block),
    }
}

/// Push a finished section into `sections`, unless it is a heading with no
/// content beneath it.
///
/// A heading followed by zero blocks carries nothing the candidate wrote —
/// there is no content here to lose, only a label with nothing under it, which
/// is what the reported "empty Projects/Publications section" bug looked like
/// once rendered. A legitimately terse section (even one short paragraph or
/// bullet) has at least one block and survives untouched; only the
/// zero-content case is dropped, so nothing a source document or a generator
/// actually wrote is ever discarded here.
fn push_nonempty_section(section: Section, sections: &mut Vec<Section>) {
    if !section.blocks.is_empty() && !is_placeholder_only(&section) {
        sections.push(section);
    }
}

/// A section whose ENTIRE content is one parenthetical aside, e.g.
///
/// ```text
/// AUSZEICHNUNGEN
/// (Keine Auszeichnungen im vorliegenden Lebenslauf)
/// ```
///
/// A generator told to omit a section it has no content for sometimes writes
/// the heading anyway with a note explaining the absence. `push_nonempty_section`
/// cannot catch that — the section is not empty — so it reached the page as a
/// real heading advertising that the candidate has no awards, which is strictly
/// worse than not printing the section at all.
///
/// Tested on the SHAPE, not on wording: the note is written in the résumé's own
/// language, so a keyword list ("Keine", "None", "N/A") would be a permanent
/// translation debt. A lone paragraph wrapped end to end in parentheses is a
/// meta-comment about the document in any language; real résumé content is not
/// written that way.
///
/// The whole paragraph must be ONE parenthetical — first and last character is
/// not enough. `(B.Sc.) Computer Science (2020)` opens and closes with a paren
/// but is ordinary Education content, and dropping its section would delete a
/// real qualification. The opening paren must therefore still be unclosed until
/// the final character.
fn is_placeholder_only(section: &Section) -> bool {
    let [Block::Paragraph(runs)] = section.blocks.as_slice() else {
        return false;
    };
    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
    wraps_one_parenthetical(text.trim())
}

/// Is `text` a SINGLE parenthetical — one `(…)` spanning the whole string?
fn wraps_one_parenthetical(text: &str) -> bool {
    if text.chars().count() <= 2 || !text.starts_with('(') || !text.ends_with(')') {
        return false;
    }
    let last = text.chars().count() - 1;
    let mut depth = 0usize;
    for (index, ch) in text.chars().enumerate() {
        match ch {
            '(' => depth += 1,
            ')' => match depth.checked_sub(1) {
                // A `)` with nothing open: unbalanced, so not one parenthetical.
                None => return false,
                // The outer pair closed early — what follows is more content.
                Some(0) if index != last => return false,
                Some(rest) => depth = rest,
            },
            _ => {}
        }
    }
    depth == 0
}

/// The next line with content WITHIN the current section, skipping blanks.
///
/// Blanks separate projects, so a project's last description line must still
/// see the next project's title. A [`LineKind::SectionHeader`] ends the search
/// instead: looking past it let a separator-bearing heading (`SKILLS · TOOLS`)
/// turn the final line of Projects into a title, and the sibling groupings in
/// `pipeline::resume::source` and `validate::content` are section-scoped by
/// construction — this is what keeps all three answering alike.
fn next_content_line(lines: &[ParsedLine], idx: usize) -> Option<&ParsedLine> {
    lines[idx.saturating_add(1)..]
        .iter()
        .take_while(|l| !matches!(l.kind, LineKind::SectionHeader))
        .find(|l| !matches!(l.kind, LineKind::Blank) && !l.text.trim().is_empty())
}

/// Does this line open a project entry in the RENDER model?
///
/// A leading bold run is the signal the generated signature carries. Failing
/// that, the shape decides — see [`is_project_title_shaped`], which is shared
/// with the pipeline's grouping so the two can never disagree about where an
/// entry begins.
///
/// A bullet is deliberately NOT an opener here: the compact tier
/// (`• Name · Website · Github`) is a standalone one-liner, and the `Bullet` arm
/// already appends bullets to whichever entry is open.
fn opens_project_entry(
    line: &ParsedLine,
    next: Option<&ParsedLine>,
    at_paragraph_start: bool,
) -> bool {
    if line
        .segments
        .first()
        .is_some_and(|seg| seg.bold && !seg.text.trim().is_empty())
    {
        return true;
    }
    is_project_title_shaped(
        &line.text,
        next.map(|n| n.text.as_str()),
        at_paragraph_start,
    )
}

/// Regroup one line of a Projects section into an [`EntryBlock`], returning
/// `true` when the line was consumed.
///
/// Projects are the one section whose entries carry no date, and that is why
/// they arrive here as loose lines. The parser reads a project's title line
/// (`**Ledger CLI** · https://github.com/…`) as `Contact` — it holds a URL — and
/// its `·`-separated tech-stack line as `Contact` or `Text` depending only on how
/// many technologies are listed, so the generic arms below flatten a whole
/// project into three indistinguishable paragraphs. Every template already
/// styles an entry's title and subtitle; nothing styles three paragraphs. This
/// rebuilds the entry the text was always describing:
///
/// ```text
/// **Ledger CLI** · https://github.com/janedoe/ledger   <- title  (bold-led)
/// Rust · SQLite · Clap                                 <- subtitle (stack line)
/// A double-entry bookkeeping tool for the terminal.    <- bullet  (description)
/// ```
///
/// Only the line DIRECTLY under a title can claim the subtitle slot
/// (`subtitle.is_none() && bullets.is_empty()`), so a later `·`-bearing sentence
/// stays body content. A line that opens no entry and has no entry to join is
/// left to the caller untouched, so a prose-only Projects section renders exactly
/// as it does today. Scoped to [`SectionId::Projects`]: the same shapes under
/// Experience or a custom heading are not touched.
fn absorb_project_line(
    line: &ParsedLine,
    next: Option<&ParsedLine>,
    at_paragraph_start: bool,
    entry: &mut Option<EntryBlock>,
    current: &mut Option<Section>,
    preamble: &mut Vec<Block>,
) -> bool {
    if !matches!(current, Some(section) if section.id == SectionId::Projects) {
        return false;
    }

    if opens_project_entry(line, next, at_paragraph_start) {
        flush_entry(entry, current, preamble);
        *entry = Some(EntryBlock {
            // `line.raw` so the bold run and the markdown links survive.
            title: tokenize_rich(&line.raw),
            subtitle: None,
            date: None,
            bullets: Vec::new(),
        });
        return true;
    }

    let Some(open) = entry.as_mut() else {
        return false;
    };
    if open.subtitle.is_none() && open.bullets.is_empty() && is_project_stack_shaped(&line.text) {
        open.subtitle = Some(tokenize_rich(&line.raw));
    } else {
        open.bullets.push(tokenize_rich(&line.raw));
    }
    true
}

/// Close the in-progress entry (if any), emitting it as a [`Block::Entry`].
fn flush_entry(
    entry: &mut Option<EntryBlock>,
    current: &mut Option<Section>,
    preamble: &mut Vec<Block>,
) {
    if let Some(e) = entry.take() {
        push_block(Block::Entry(e), current, preamble);
    }
}

/// Build a resume [`DocumentModel`] from raw resume text.
///
/// The first non-section line that looks like a name becomes the header name;
/// contact lines before the first section become the header contact runs.
/// Everything after is grouped under its section heading. Content appearing
/// before any heading is kept in a leading [`SectionId::Summary`] section with an
/// empty heading (so the layout engine renders the body with no visible title).
pub fn model_from_resume_text(text: &str) -> DocumentModel {
    let parsed = parse_resume(text);

    let mut name: Option<String> = None;
    let mut title: Option<String> = None;
    let mut contact_parts: Vec<String> = Vec::new();
    let mut seen_section = false;

    let mut sections: Vec<Section> = Vec::new();
    let mut preamble: Vec<Block> = Vec::new();
    let mut current: Option<Section> = None;
    let mut entry: Option<EntryBlock> = None;

    // True when the line about to be handled opens a paragraph: the first
    // content line after a blank or a section heading. Projects are separated by
    // blank lines and a description line never is, which is what keeps an
    // unpunctuated line INSIDE an entry from being read as the next title.
    let mut at_paragraph_start = true;
    for (idx, line) in parsed.lines.iter().enumerate() {
        let at_paragraph_start = std::mem::replace(
            &mut at_paragraph_start,
            matches!(line.kind, LineKind::Blank | LineKind::SectionHeader),
        );
        match line.kind {
            LineKind::Blank => {}

            LineKind::Name => {
                if !seen_section && name.is_none() {
                    name = Some(line.text.clone());
                } else {
                    // A stray name-like line inside the body: keep it, don't drop it.
                    flush_entry(&mut entry, &mut current, &mut preamble);
                    push_block(
                        Block::Paragraph(tokenize_rich(&line.raw)),
                        &mut current,
                        &mut preamble,
                    );
                }
            }

            LineKind::Contact => {
                if !seen_section {
                    // Header contact: keep the raw (un-stripped) line so markdown
                    // links `[label](url)` tokenize into clickable runs.
                    contact_parts.push(line.raw.clone());
                } else if !absorb_project_line(
                    line,
                    next_content_line(&parsed.lines, idx),
                    at_paragraph_start,
                    &mut entry,
                    &mut current,
                    &mut preamble,
                ) {
                    flush_entry(&mut entry, &mut current, &mut preamble);
                    push_block(
                        Block::Paragraph(tokenize_rich(&line.raw)),
                        &mut current,
                        &mut preamble,
                    );
                }
            }

            LineKind::SectionHeader => {
                flush_entry(&mut entry, &mut current, &mut preamble);
                if let Some(section) = current.take() {
                    push_nonempty_section(section, &mut sections);
                }
                seen_section = true;
                current = Some(Section {
                    id: SectionId::from_header(&line.text),
                    heading: line.text.clone(),
                    blocks: Vec::new(),
                });
            }

            LineKind::JobEntry => {
                flush_entry(&mut entry, &mut current, &mut preamble);
                entry = Some(EntryBlock {
                    // `line.raw` keeps the title's own markdown (bold/links);
                    // the date, when the parser split one out, is
                    // `right_text` and is never part of it.
                    title: tokenize_rich(&line.raw),
                    subtitle: None,
                    date: line.right_text.clone(),
                    bullets: Vec::new(),
                });
            }

            LineKind::JobTitle => {
                // Attach as the subtitle of the open entry if it doesn't have one.
                let attached = match entry.as_mut() {
                    Some(e) if e.subtitle.is_none() => {
                        e.subtitle = Some(tokenize_rich(&line.raw));
                        true
                    }
                    _ => false,
                };
                if !attached {
                    flush_entry(&mut entry, &mut current, &mut preamble);
                    push_block(
                        Block::Paragraph(tokenize_rich(&line.raw)),
                        &mut current,
                        &mut preamble,
                    );
                }
            }

            LineKind::Bullet => match entry.as_mut() {
                Some(e) => e.bullets.push(tokenize_rich(&line.raw)),
                None => push_block(
                    Block::Bullet(tokenize_rich(&line.raw)),
                    &mut current,
                    &mut preamble,
                ),
            },

            LineKind::Text => {
                // Promote a short role line that sits directly after the
                // name/contact (before any section, as the very first body line)
                // into the header title slot — that's where every template
                // renders it (italic, under the name), so it no longer floats as
                // a redundant stray paragraph above the summary.
                if !seen_section
                    && title.is_none()
                    && name.is_some()
                    && entry.is_none()
                    && preamble.is_empty()
                    && is_title_like(&line.text)
                {
                    title = Some(line.text.clone());
                } else if !absorb_project_line(
                    line,
                    next_content_line(&parsed.lines, idx),
                    at_paragraph_start,
                    &mut entry,
                    &mut current,
                    &mut preamble,
                ) {
                    flush_entry(&mut entry, &mut current, &mut preamble);
                    push_block(
                        Block::Paragraph(tokenize_rich(&line.raw)),
                        &mut current,
                        &mut preamble,
                    );
                }
            }
        }
    }

    // Final flush of any open entry / section.
    flush_entry(&mut entry, &mut current, &mut preamble);
    if let Some(section) = current.take() {
        push_nonempty_section(section, &mut sections);
    }

    // Leading content under no heading becomes an untitled Summary section so it
    // renders first without an invented heading.
    if !preamble.is_empty() {
        sections.insert(
            0,
            Section {
                id: SectionId::Summary,
                heading: String::new(),
                blocks: preamble,
            },
        );
    }

    // Each `contact_parts` entry is one already-classified `Contact` LINE from
    // the source text — which, when the source itself splits the contact
    // block across two or more physical lines, commonly already ends (or
    // starts) with its own trailing/leading separator (a pipe or middot the
    // source used to continue visually onto the next line). Joining those
    // raw lines with another " · " on top used to double up: "Berlin |" + " · "
    // + "· email@x.com ·" + " · " + "|" → visibly doubled/misaligned
    // separators in the rendered header. Stripping ONE stray separator (plus
    // surrounding whitespace) off each line's ends before joining keeps every
    // line's own INTERNAL separator style untouched while guaranteeing
    // exactly one clean " · " between lines.
    let contact = if contact_parts.is_empty() {
        Vec::new()
    } else {
        let cleaned: Vec<String> = contact_parts
            .iter()
            .map(|part| {
                part.trim()
                    .trim_start_matches(['|', '\u{b7}', '\u{2022}'])
                    .trim_end_matches(['|', '\u{b7}', '\u{2022}'])
                    .trim()
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();
        tokenize_rich(&cleaned.join(" \u{b7} "))
    };

    let mut model = DocumentModel::new(DocumentType::Resume);
    model.header = HeaderBlock {
        name: name.unwrap_or_default(),
        title,
        contact,
    };
    model.sections = sections;
    model
}

#[cfg(test)]
mod tests;
