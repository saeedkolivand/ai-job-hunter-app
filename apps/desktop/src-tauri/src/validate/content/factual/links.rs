//! `factual.altered_project_link` — the links a document claims against the
//! links its source carries, and the one canonical key they are compared on.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::SectionKind;
use crate::export::types::ParsedLine;
use crate::validate::content::consistency::project_entries;
use crate::validate::content::{
    issue, Analysis, ContentIssue, Section, FACTUAL_ALTERED_PROJECT_LINK,
};

/// Hosts whose bare (scheme-less) form is unambiguously a link rather than a
/// library name. Everything else needs a scheme, a `www.`, or a `/path`.
const CODE_HOSTS: &str = "github|gitlab|bitbucket|codeberg|sourcehut|sourceforge|npmjs|crates|pypi|huggingface|gitea|launchpad";

/// A URL in résumé prose.
///
/// Four arms, and the fourth one is the whole reason this regex is not simply
/// "host dot TLD": a scheme-less bare host reads a stack line's library names as
/// links. `Socket.IO` is `.io`, `Bun.sh` is `.sh`, `Nuxt.dev` is `.dev` — and a
/// stack line listing three of them produced three Critical
/// `factual.altered_project_link` findings on a truthful résumé. So a bare host
/// must either be a known code host or carry a `/path`.
///
/// **No fifth "bare domain, no path, lowercase" arm.** That was tried (to mirror
/// `model::rich`'s bare-domain-without-path renderer arm) and reverted: with no
/// email-aware arm and no lookaround in the `regex` crate, it matched the
/// DOMAIN HALF of an email address as a bare "URL" (`urls_in("jane@gmail.com")`
/// → `["gmail.com"]`), and on a title/description line — which carries no
/// `names_a_resource` filter — a lowercase library mention in ordinary prose
/// reopened the exact false-Critical shape this file exists to prevent. See
/// `model::rich`'s doc for why the renderer's mirror arm was reverted too.
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Assembled from parts, never a `\`-continued raw string: a raw string does
    // not process escapes, so a trailing backslash would leave a literal
    // `\<spaces>` inside the pattern and silently kill the arm before it.
    let pattern = [
        r"(?i)(?:https?://[^\s)\]<>]+",
        r"|www\.[^\s)\]<>]+",
        &format!(r"|(?:{CODE_HOSTS})\.(?:com|org|io|dev|net|rs)(?:/[^\s)\]<>]*)?"),
        r"|[a-z0-9-]+(?:\.[a-z0-9-]+)*\.(?:com|org|net|io|dev|app|de|co|ai|sh|me)/[^\s)\]<>]*)",
    ]
    .concat();
    Regex::new(&pattern).unwrap()
});

/// Every URL in `text`, VERBATIM, with trailing sentence punctuation trimmed.
/// Markdown link targets are captured by the same pass — the regex matches the
/// href inside `[anchor](href)` as well as a bare URL, and `)` is excluded from
/// every arm's character class so the href stops at the closing paren.
pub fn urls_in(text: &str) -> Vec<String> {
    URL_RE
        .find_iter(text)
        .map(|m| {
            m.as_str()
                .trim_end_matches(['.', ',', ';', ':'])
                .to_string()
        })
        .collect()
}

/// The comparison key for a link: scheme dropped, one leading `www.` dropped,
/// host lowercased, one trailing `/` removed. The PATH keeps its case —
/// `/JaneDoe/Ledger` and `/janedoe/ledger` are different resources on most
/// hosts, and treating them as one would hide a genuinely altered link.
///
/// Compared on the key, REPORTED verbatim. `https://github.com/janedoe/ledger`
/// and `github.com/janedoe/ledger` are the same link written two ways, and
/// telling a candidate their own URL was "missing or altered" because the model
/// dropped the scheme is the false Critical this key exists to prevent. `www.`
/// is the same edit one label along — the model drops it (or adds it) exactly
/// as readily — and it drew TWO Criticals per link, one for the source form
/// "missing or altered" and one for the generated form "invented".
///
/// Only the FIRST `www.` label goes: `www.www.example.com` is a different host
/// from `www.example.com`, and this key may only ever collapse spellings of the
/// same resource.
///
/// **Never index a `&str` by a fixed byte offset here.** URL text in a résumé is
/// arbitrary UTF-8 (`www.café-berlin.de`, `ab.com/éx`), and `&s[..8]` panics the
/// moment a multibyte char straddles byte 8. Release builds are `panic = "abort"`,
/// so that panic killed the whole generation before the document was saved. Every
/// boundary this function cuts at is therefore either byte-compared on
/// `as_bytes()` (below) or derived from a char-aware API (`trim*`, `split_once`).
pub fn canonical_link(raw: &str) -> String {
    let mut s = raw.trim().trim_end_matches(['.', ',', ';', ':']);
    for scheme in ["https://", "http://"] {
        // Byte-compare the prefix; only slice once it is known to BE the scheme.
        if s.len() >= scheme.len()
            && s.as_bytes()[..scheme.len()].eq_ignore_ascii_case(scheme.as_bytes())
        {
            s = &s[scheme.len()..]; // Safe: the matched prefix is pure ASCII.
            break;
        }
    }
    // Same rule one label along, and the same byte-compare-then-slice shape:
    // `wwwé.de` is a host that merely starts with those three letters, and a
    // blind `&s[..4]` there cuts inside the `é` and aborts the process.
    const WWW: &str = "www.";
    if s.len() >= WWW.len() && s.as_bytes()[..WWW.len()].eq_ignore_ascii_case(WWW.as_bytes()) {
        s = &s[WWW.len()..]; // Safe: the matched prefix is pure ASCII.
    }
    let s = s.trim_end_matches('/');
    match s.split_once('/') {
        Some((host, path)) => format!("{}/{path}", host.to_lowercase()),
        None => s.to_lowercase(),
    }
}

/// The href inside a link entry, unwrapping a markdown span (`[label](href)`)
/// captured verbatim by `pipeline::resume::source::seed_one_project`. A bare
/// URL passes through unchanged.
///
/// Lives beside [`canonical_link`], not in `pipeline::resume`, because BOTH
/// `pipeline::resume::source` (which captures the span) and
/// `pipeline::resume::projects` (which compares it) need it, and putting it in
/// either one would make the other import from it — a module cycle. Every
/// canonical comparison either module makes routes through this first, so a
/// labeled and a bare copy of the same URL are never treated as two different
/// links.
pub fn link_href(link: &str) -> &str {
    let trimmed = link.trim();
    let Some(rest) = trimmed.strip_prefix('[') else {
        return trimmed;
    };
    let Some(split) = rest.find("](") else {
        return trimmed;
    };
    let href = &rest[split + 2..];
    href.strip_suffix(')').unwrap_or(trimmed)
}

/// Whether a URL span names a RESOURCE rather than a package registry or a
/// library that happens to be host-shaped: it carries a scheme, a `www.` host,
/// or a path.
///
/// [`URL_RE`]'s third arm accepts a bare [`CODE_HOSTS`] name with no path
/// (`crates.io`, `npmjs.com`), which is correct on a title line — that is how a
/// project links to its package — and wrong on a stack line, where the same
/// token is the ecosystem the project is written for. A path or a scheme is
/// what tells the two apart; see [`project_links`].
///
/// `pub` and re-exported from [`super`] because the max-depth pipeline SEEDS a
/// project's links out of the same source section this check compares against:
/// a second answer to "is this span a link" there would mean the generator
/// omits a link the validator then reports as altered — a Critical produced by
/// two graders disagreeing rather than by anything wrong with the document.
pub fn names_a_resource(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("www.")
        || canonical_link(url).contains('/')
}

/// The links a projects section actually claims, verbatim.
///
/// The second line of an entry is the STACK line in the owner-locked projects
/// format ("Rust · SQLite · Clap"), and a technology list is never a link. What
/// it may still carry is a bare package-registry host, so the exclusion is
/// applied per URL — only a span that [`names_a_resource`] survives there.
///
/// It used to be applied per LINE, keyed on the literal `"://"`: a stack line
/// without that substring was dropped whole. A links line written without a
/// scheme ("github.com/janedoe/ledger", the form half of all résumés use) was
/// therefore cut out of the SOURCE link set, and a generated document that
/// spelled the same link out with `https://` was reported as linking somewhere
/// "not in your source résumé" — a Critical telling candidates they had invented
/// their own repository URL. Substring-testing for a scheme cannot tell a
/// missing scheme from a missing link; the resource test can.
///
/// Entry grouping is `consistency::project_entries`, so the two checks cannot
/// disagree about where an entry begins.
///
/// Returned PER ENTRY, keyed by [`project_entry_name`], because a link only
/// means "missing or altered" relative to an entry the document kept — see
/// [`project_link_issues`].
fn project_entry_links(section: &Section) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for entry in project_entries(section) {
        let mut urls = Vec::new();
        for (index, line) in entry.iter().enumerate() {
            let found = urls_in(&line.text);
            if index == 1 {
                urls.extend(found.into_iter().filter(|u| names_a_resource(u)));
            } else {
                urls.extend(found);
            }
        }
        out.push((project_entry_name(&entry), urls));
    }
    out
}

/// The identifying NAME of one projects entry: its title line up to the first
/// separator, reduced to lowercase alphanumeric words ("**Ledger CLI** · repo"
/// → `"ledger cli"`).
///
/// Deliberately an EXACT key rather than a fuzzy one. A stricter match makes
/// more entries look dropped, and a dropped entry is silent — so the error this
/// choice makes is a missed check, which is the direction this whole file errs
/// in. (A loose match would do the opposite: pair two unrelated projects and
/// report one's links as the other's.) An entry whose title line yields no
/// words is unmatchable and therefore silent too.
///
/// *Accepted cost, stated:* a generated document that RENAMES a project
/// ("Ledger CLI" → "Ledger CLI Tool") reads as one entry dropped and one added,
/// so the rename's links are no longer diffed against the original's — only
/// against the whole source set, which still catches an invented host.
fn project_entry_name(entry: &[&ParsedLine]) -> String {
    let Some(first) = entry.first() else {
        return String::new();
    };
    let head = first.text.split(['·', '|', '•']).next().unwrap_or_default();
    head.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<String>>()
        .join(" ")
}

/// `factual.altered_project_link` — the projects section's links must be the
/// candidate's own. Dropped, changed and invented all fire (a change surfaces as
/// one drop plus one invention).
///
/// A project link is how a reviewer verifies the work exists: a "helpfully"
/// corrected host or a stripped path leads them somewhere else. What is compared
/// is therefore the [`canonical_link`] key — everything that identifies the
/// resource — while the evidence quotes the span exactly as written, so the user
/// can see which of the two forms their document carries.
///
/// **Trimming is not altering, at either grain.** Cutting the section entirely
/// is a tailoring decision, and the commonest one there is — a résumé trimmed to
/// one page drops projects before it drops a role. Reading that as "every one of
/// your links was altered" produced a Critical per source link on a document
/// whose only change was an editorial cut, which is the loudest possible way to
/// be wrong about the safest possible edit.
///
/// The SAME argument applies one level down, and the section-level carve-out
/// alone did not cover it: trimming three projects to two is the identical edit
/// at the identical grain, and it fired a Critical per link on the entry that
/// went. So the source side reports only links belonging to an entry the
/// generated document KEPT ([`project_entry_name`]); a whole entry that is
/// simply gone is silent.
///
/// The GENERATED side is unscoped on purpose and needs no carve-out: trimming
/// removes links from that side, it never adds one, so a URL the source never
/// carried anywhere is an invention however it arrived.
///
/// Deliberately NOT replaced by a Warning-severity "dropped" note, at either
/// grain. A cut is not a defect: nothing was altered, invented or lost from the
/// candidate's own claims, so there is no evidence to show them and no action to
/// advise. If the cut actually cost the document something, that is a measured
/// finding `alignment.low_coverage` already makes — with the numbers to back it
/// — rather than a second unmeasured one here. (A new code would also need a
/// registered severity and an i18n key in both locales for a message that would
/// read "you did a normal thing".)
pub(super) fn project_link_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    // ALL source Projects sections, unioned — not just the first. `SECTION_NAMES`
    // recognises both "projects" and "side projects", and both classify
    // `Projects`, so a second source Projects section is ordinary, not rare.
    // Reading only the first left the second one's links out of the sourced
    // set, so a document that never changed a thing accused itself of
    // inventing its own link — and unclearably: `criticals_by_section` routes
    // the finding to the FIRST Projects section, which never contained it.
    let mut source_sections = ctx
        .source_sections_of_kind(SectionKind::Projects)
        .peekable();
    if source_sections.peek().is_none() {
        return Vec::new(); // Nothing to compare against.
    }
    let source_entries: Vec<(String, Vec<String>)> =
        source_sections.flat_map(project_entry_links).collect();
    // ALL generated Projects sections, not just the first — see
    // `Analysis::generated_sections_of_kind`'s doc. An invented link that
    // lands in a SECOND Projects section (a duplicate the model produced, or
    // one already present in an imported résumé) must not go unchecked.
    let mut generated_sections = ctx
        .generated_sections_of_kind(SectionKind::Projects)
        .peekable();
    if generated_sections.peek().is_none() {
        return Vec::new(); // The section was cut, not rewritten — see above.
    }
    let generated_entries: Vec<(String, Vec<String>)> =
        generated_sections.flat_map(project_entry_links).collect();
    let flatten = |entries: &[(String, Vec<String>)]| -> Vec<String> {
        entries.iter().flat_map(|(_, u)| u.clone()).collect()
    };
    let source_urls = flatten(&source_entries);
    let generated_urls = flatten(&generated_entries);
    if source_urls.is_empty() && generated_urls.is_empty() {
        return Vec::new();
    }
    let key_set =
        |urls: &[String]| -> HashSet<String> { urls.iter().map(|u| canonical_link(u)).collect() };
    let source_keys = key_set(&source_urls);
    let generated_keys = key_set(&generated_urls);
    // An unnamed entry matches nothing, deliberately — see `project_entry_name`.
    let surviving: HashSet<&str> = generated_entries
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !name.is_empty())
        .collect();

    let mut issues = Vec::new();
    for url in source_entries
        .iter()
        .filter(|(name, _)| surviving.contains(name.as_str()))
        .flat_map(|(_, urls)| urls)
    {
        if !generated_keys.contains(&canonical_link(url)) {
            issues.push(issue(
                FACTUAL_ALTERED_PROJECT_LINK,
                Some("Projects"),
                format!(
                    "The project link {url} from your source résumé is missing or altered in \
                     the generated document. Project links must match your own exactly."
                ),
                Some(url.clone()),
            ));
        }
    }
    // `generated_urls` is the UNION of every generated Projects section
    // (`Analysis::generated_sections_of_kind`'s doc) — the same invented URL
    // can appear in two of them (a duplicate the model produced, or one
    // already present in an imported résumé), and without a seen-set each
    // occurrence pushed its own identical Critical. Deduped on the SAME
    // `canonical_link` key the comparison itself uses, so two spellings of one
    // invented resource still collapse to one finding — `HashSet::insert`
    // returning `false` on the second occurrence is exactly "already
    // reported".
    let mut reported_invented: HashSet<String> = HashSet::new();
    for url in &generated_urls {
        let key = canonical_link(url);
        if !source_keys.contains(&key) && reported_invented.insert(key) {
            issues.push(issue(
                FACTUAL_ALTERED_PROJECT_LINK,
                Some("Projects"),
                format!(
                    "The generated projects section links to {url}, which is not in your source \
                     résumé. Remove it or replace it with your own link."
                ),
                Some(url.clone()),
            ));
        }
    }
    issues
}
