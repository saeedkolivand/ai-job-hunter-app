//! Role and project entries of a document, and the source entry a generated
//! entry stands for. The unit the grounding checks compare a claim against.

use std::collections::HashSet;

use crate::documents::evidence::{split_entry, SectionKind};
use crate::export::types::{LineKind, ParsedLine};
use crate::validate::content::consistency::project_entries;
use crate::validate::content::{Analysis, Section};

use super::employment::company_survives;
use super::terms::keys;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Role,
    Project,
}

pub(super) struct Block<'a> {
    pub kind: Kind,
    pub header: &'a ParsedLine,
    /// Everything under the header: bullets and plain lines.
    pub body: Vec<&'a ParsedLine>,
    /// Employer (role) or project name, as written.
    pub identity: String,
}

impl Block<'_> {
    /// The header and every body line, as one text.
    pub fn text(&self) -> String {
        std::iter::once(self.header)
            .chain(self.body.iter().copied())
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn project_name(header: &ParsedLine) -> String {
    header
        .segments
        .iter()
        .find(|s| s.bold)
        .map(|s| s.text.clone())
        .unwrap_or_else(|| {
            header
                .text
                .split(['·', '|', '•'])
                .next()
                .unwrap_or("")
                .to_string()
        })
        .trim()
        .to_string()
}

/// The role or project entries of one section. Lines ahead of the first entry
/// belong to no entry and are not returned.
pub(super) fn section_blocks(section: &Section) -> Vec<Block<'_>> {
    match section.kind {
        SectionKind::Experience => {
            let mut blocks: Vec<Block> = Vec::new();
            for line in &section.lines {
                match line.kind {
                    LineKind::JobEntry => blocks.push(Block {
                        kind: Kind::Role,
                        header: line,
                        body: Vec::new(),
                        identity: split_entry(line).0,
                    }),
                    LineKind::Blank | LineKind::SectionHeader => {}
                    _ => {
                        if let Some(last) = blocks.last_mut() {
                            last.body.push(line);
                        }
                    }
                }
            }
            blocks
        }
        SectionKind::Projects => project_entries(section)
            .into_iter()
            .map(|mut entry| {
                let header = entry.remove(0);
                Block {
                    kind: Kind::Project,
                    header,
                    body: entry,
                    identity: project_name(header),
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn all_blocks(sections: &[Section]) -> Vec<Block<'_>> {
    sections.iter().flat_map(section_blocks).collect()
}

fn name_tokens(name: &str) -> HashSet<String> {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 3)
        .map(str::to_lowercase)
        .collect()
}

/// The source entries a generated entry stands for. Several can match (the same
/// employer twice, two projects sharing a word): the checks then read their
/// union, which can only make them quieter. Empty means the entry cannot be
/// identified, and nothing is checked against it.
pub(super) fn counterparts<'b, 'a>(
    generated: &Block,
    source: &'b [Block<'a>],
) -> Vec<&'b Block<'a>> {
    let wanted = name_tokens(&generated.identity);
    source
        .iter()
        .filter(|s| s.kind == generated.kind)
        .filter(|s| match generated.kind {
            Kind::Role => company_survives(&s.identity.to_lowercase(), &generated.identity),
            Kind::Project => !wanted.is_disjoint(&name_tokens(&s.identity)),
        })
        .collect()
}

/// Kernel keys of the whole text of `blocks`.
pub(super) fn keys_of(ctx: &Analysis, blocks: &[&Block]) -> HashSet<String> {
    keys(
        ctx,
        &blocks
            .iter()
            .map(|b| b.text())
            .collect::<Vec<_>>()
            .join("\n"),
    )
}
