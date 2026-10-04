//! Fixtures shared by the adapter tests: visible-text flattening and the lookups
//! that pull a section, its headings or its entries out of a built model.

use super::*;
use crate::model::rich::RichText;

/// Flatten a RichText into its visible string for concise assertions.
pub(super) fn flat(rt: &RichText) -> String {
    rt.iter().map(|r| r.text.as_str()).collect()
}

pub(super) fn find_section(model: &DocumentModel, id: SectionId) -> Option<&Section> {
    model.sections.iter().find(|s| s.id == id)
}

pub(super) fn section_ids(model: &DocumentModel) -> Vec<&SectionId> {
    model.sections.iter().map(|s| &s.id).collect()
}

pub(super) fn section_headings(model: &DocumentModel) -> Vec<&str> {
    model.sections.iter().map(|s| s.heading.as_str()).collect()
}

fn entries_of<'a>(blocks: impl Iterator<Item = &'a Block>) -> Vec<&'a EntryBlock> {
    blocks
        .filter_map(|b| match b {
            Block::Entry(e) => Some(e),
            _ => None,
        })
        .collect()
}

/// The entries of one section, in order.
pub(super) fn section_entries(section: &Section) -> Vec<&EntryBlock> {
    entries_of(section.blocks.iter())
}

/// The entries of every section, in order.
pub(super) fn entries(model: &DocumentModel) -> Vec<&EntryBlock> {
    entries_of(model.sections.iter().flat_map(|s| &s.blocks))
}
