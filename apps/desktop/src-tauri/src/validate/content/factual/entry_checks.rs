//! The per-entry checks that are not about a single term: a location the source
//! gives the role no such place for, a bullet sharing nothing with its entry,
//! and an outcome clause or scope claim the entry does not state.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::{
    canonical_place, country_of, split_entry, GEOGRAPHY_TOKENS, NON_PLACE_TOKENS,
};
use crate::validate::content::{
    issue, Analysis, ContentIssue, FACTUAL_INVENTED_LOCATION, FACTUAL_UNSUPPORTED_BULLET,
    FACTUAL_UNSUPPORTED_LOCATION, FACTUAL_UNSUPPORTED_OUTCOME,
};

use super::blocks::Block;
use super::terms::stems;

/// A bullet needs at least this many content tokens to be judged at all.
pub const MIN_BULLET_TOKENS: usize = 4;
/// Content tokens a bullet must share with its source entry to count as
/// grounded in it.
pub const MIN_BULLET_OVERLAP: usize = 2;
/// An outcome clause needs at least this many content tokens to be judged.
pub const MIN_OUTCOME_TOKENS: usize = 2;

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
}

/// Places on a role's header that are neither its employer nor its title.
fn header_places(block: &Block) -> Vec<String> {
    let (company, title, _) = split_entry(block.header);
    let identity: HashSet<String> = words(&company).chain(words(&title)).collect();
    words(&block.header.text)
        .filter(|w| !identity.contains(w))
        .map(|w| canonical_place(&w))
        // Canonical first: "Vienna"/"Zurich"/"EU" are equivalents, not listed tokens.
        .filter(|p| {
            GEOGRAPHY_TOKENS.contains(&p.as_str()) && !NON_PLACE_TOKENS.contains(&p.as_str())
        })
        .collect()
}

/// Canonical places in `text`, with the country of every city among them.
pub(super) fn places_in(text: &str) -> HashSet<String> {
    let mut out: HashSet<String> = words(text).map(|w| canonical_place(&w)).collect();
    let countries: Vec<String> = out
        .iter()
        .filter_map(|p| country_of(p))
        .map(str::to_string)
        .collect();
    out.extend(countries);
    out
}

/// A place on a role's header that the source entry does not give it.
///
/// Known: the matched entry's own places, with the country of any city among
/// them. A place found only in the contact block (`contact`) is advisory,
/// `factual.unsupported_location`; a place found nowhere, or only under another
/// role or in Education ("TU Berlin" is no evidence the role was in Berlin), is
/// `factual.invented_location`.
pub(super) fn location_issues(
    block: &Block,
    source: &[&Block],
    contact: &HashSet<String>,
    section: Option<&str>,
) -> Vec<ContentIssue> {
    let entry_text = source.iter().map(|b| b.text()).collect::<Vec<_>>().join(
        "
",
    );
    let known = places_in(&entry_text);
    let mut seen = HashSet::new();
    header_places(block)
        .into_iter()
        .filter(|p| !known.contains(p) && seen.insert(p.clone()))
        .map(|place| {
            // As the header spells it: the evidence must be a verbatim span.
            let shown = block
                .header
                .text
                .split(|c: char| !c.is_alphanumeric())
                .find(|w| canonical_place(&w.to_lowercase()) == place)
                .unwrap_or(&place)
                .to_string();
            let (code, message) = if contact.contains(&place) {
                (
                    FACTUAL_UNSUPPORTED_LOCATION,
                    format!(
                        concat!(
                            "\"{}\" is given elsewhere in your résumé, not for \"{}\". ",
                            "Keep it on this entry only if it is true."
                        ),
                        shown, block.identity
                    ),
                )
            } else {
                (
                    FACTUAL_INVENTED_LOCATION,
                    format!(
                        concat!(
                            "Your source résumé gives no location \"{}\" for \"{}\". ",
                            "Remove it from this entry unless it is true."
                        ),
                        shown, block.identity
                    ),
                )
            };
            issue(code, section, message, Some(shown))
        })
        .collect()
}

/// Connectives that open an outcome clause tacked on to a bullet (English and
/// German). The clause is what follows.
static OUTCOME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i),\s*(?:reducing|improving|increasing|boosting|enabling|ensuring|driving|cutting|saving|streamlining|accelerating|resulting in|leading to|which (?:reduced|improved|increased|saved|enabled|boosted|cut)|wodurch|was zu|sodass|so that)\s+(.+)$",
    )
    .unwrap()
});

/// A claim of organisation-wide reach.
static SCOPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:company|organi[sz]ation|enterprise|firm|unternehmens)[- ]?(?:wide|weit)\b",
    )
    .unwrap()
});

pub(super) fn bullet_issues(
    ctx: &Analysis,
    line_raw: &str,
    line_text: &str,
    entry_text: &str,
    entry_stems: &HashSet<String>,
    section: Option<&str>,
) -> Vec<ContentIssue> {
    let mut out = Vec::new();
    let own = stems(ctx, line_text);
    let shared = own.intersection(entry_stems).count();
    if own.len() >= MIN_BULLET_TOKENS && shared < MIN_BULLET_OVERLAP {
        out.push(issue(
            FACTUAL_UNSUPPORTED_BULLET,
            section,
            "This bullet shares almost nothing with the entry it sits under in your source \
             résumé. Check that it describes work you did there."
                .to_string(),
            Some(line_raw.to_string()),
        ));
    }
    let clause = OUTCOME_RE
        .captures(line_text)
        .and_then(|c| c.get(1))
        .map(|m| stems(ctx, m.as_str()));
    let invented_clause =
        clause.is_some_and(|c| c.len() >= MIN_OUTCOME_TOKENS && c.is_disjoint(entry_stems));
    let invented_scope = SCOPE_RE.is_match(line_text) && !SCOPE_RE.is_match(entry_text);
    if invented_clause || invented_scope {
        out.push(issue(
            FACTUAL_UNSUPPORTED_OUTCOME,
            section,
            "This bullet ends in an outcome your source résumé does not state for this entry. \
             Keep it only if it is true."
                .to_string(),
            Some(line_raw.to_string()),
        ));
    }
    out
}
