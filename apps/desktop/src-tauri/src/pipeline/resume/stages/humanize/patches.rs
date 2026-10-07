//! Line patches: which lines a `voice.*` finding points at, what the model is
//! shown, and how its `{id, replacement}` answers are applied.
//!
//! The model never re-emits the document (that was ~1.1-1.5k output tokens to
//! change a handful of lines). It sees the flagged lines plus their
//! neighbours, numbered, and returns one replacement per flagged line. Every
//! guarantee that used to come from "the model copied the rest verbatim" now
//! comes from [`apply_patches`], in Rust: only flagged ids are touched, and a
//! replacement that is empty, multi-line, carries a fence tag, or changes a
//! number is dropped (the original line stays).

use serde::Deserialize;

use crate::validate::content::{urls_in, ContentReport};

use super::super::repair::issue_line;
use super::predicates::eligible_voice_issues;

/// How many `voice.*` findings a document may carry and still "read human".
/// Zero: the generator's own bans list is the standard, `voice.*` codes are
/// Warnings with no tolerance band, and the revert guard
/// (`predicates::humanize_is_worse`) already treats ANY extra flag as a loss.
/// This only names the number the stage's skip gate has always used.
pub(crate) const HUMAN_VOICE_FLAGS: usize = 0;

/// Neighbouring lines shown on each side of a flagged one, so the model can
/// read the sentence it is fixing in context.
const CONTEXT_LINES: usize = 1;

/// Bullet characters a line may start with; kept on the line when patched.
const BULLETS: &str = "-*\u{2022}\u{2013}\u{b7}\u{25aa}";

/// One flagged line: a 1-based id into `document.split('\n')` and every
/// finding that points at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlaggedLine {
    pub id: usize,
    pub findings: Vec<String>,
}

/// A document's flags split by whether they can be patched at all.
#[derive(Debug, Default)]
pub(crate) struct Flags {
    pub lines: Vec<FlaggedLine>,
    /// Findings with no locatable line (rhythm, em-dash density, ...): context
    /// for the model, never a reason to call it.
    pub document_wide: Vec<String>,
}

/// Locate each eligible `voice.*` issue on the document's lines. An issue
/// whose evidence appears on no line is document-wide. A line carrying a URL
/// is never flagged, whatever the case of the match.
pub(crate) fn flagged_lines(report: &ContentReport, document: &str) -> Flags {
    let lines: Vec<String> = document.split('\n').map(str::to_lowercase).collect();
    let linked: Vec<bool> = document
        .split('\n')
        .map(|line| !urls_in(line).is_empty())
        .collect();
    let mut flags = Flags::default();
    for issue in eligible_voice_issues(report, document) {
        let needle = issue
            .evidence
            .as_deref()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty());
        let hits: Vec<usize> = needle.map_or_else(Vec::new, |needle| {
            (0..lines.len())
                .filter(|&i| !linked[i] && lines[i].contains(&needle))
                .collect()
        });
        if hits.is_empty() {
            flags.document_wide.push(issue_line(issue));
        }
        for i in hits {
            let id = i + 1;
            match flags.lines.iter_mut().find(|l| l.id == id) {
                Some(line) => line.findings.push(issue_line(issue)),
                None => flags.lines.push(FlaggedLine {
                    id,
                    findings: vec![issue_line(issue)],
                }),
            }
        }
    }
    flags.lines.sort_by_key(|l| l.id);
    flags
}

/// The numbered excerpt the model sees: `{id}> line` for a flagged line,
/// `{id}| line` for context, `...` where lines were left out.
pub(crate) fn excerpt(document: &str, flagged: &[FlaggedLine]) -> String {
    let doc: Vec<&str> = document.split('\n').collect();
    let mut shown: Vec<usize> = flagged
        .iter()
        .flat_map(|l| {
            let lo = l.id.saturating_sub(CONTEXT_LINES).max(1);
            lo..=(l.id + CONTEXT_LINES).min(doc.len())
        })
        .collect();
    shown.sort_unstable();
    shown.dedup();
    let mut out = String::new();
    let mut prev = 0;
    for id in shown {
        if prev != 0 && id != prev + 1 {
            out.push_str("...\n");
        }
        let mark = if flagged.iter().any(|l| l.id == id) {
            '>'
        } else {
            '|'
        };
        out.push_str(&format!(
            "{id}{mark} {}\n",
            doc[id - 1].trim_end_matches('\r')
        ));
        prev = id;
    }
    out
}

/// The `<humanize_findings>` entries: per-line findings keyed by line id, then
/// the document-wide ones.
pub(crate) fn findings_for_prompt(
    flagged: &[FlaggedLine],
    document_wide: &[String],
) -> Vec<String> {
    let per_line = flagged.iter().flat_map(|l| {
        l.findings
            .iter()
            .map(move |f| format!("line {}: {f}", l.id))
    });
    let wide = document_wide.iter().map(|f| format!("document-wide: {f}"));
    per_line.chain(wide).collect()
}

/// One model-proposed replacement.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Patch {
    pub id: usize,
    pub replacement: String,
}

/// The structured answer: `{ "patches": [...] }`. Missing key = no patches.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct PatchList {
    #[serde(default)]
    pub patches: Vec<Patch>,
}

/// Split a line into its indent + bullet marker and the text after it.
fn split_prefix(line: &str) -> (&str, &str) {
    let rest = line.trim_start();
    let mut len = line.len() - rest.len();
    if let Some(bullet) = rest.chars().next().filter(|c| BULLETS.contains(*c)) {
        let after = &rest[bullet.len_utf8()..];
        len += bullet.len_utf8() + (after.len() - after.trim_start().len());
    }
    line.split_at(len)
}

/// Digit runs of `text`, sorted — equal runs means no number was added,
/// dropped or changed.
fn digit_runs(text: &str) -> Vec<&str> {
    let mut runs: Vec<&str> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|run| !run.is_empty())
        .collect();
    runs.sort_unstable();
    runs
}

/// The accepted replacement body for `original_body`, or `None` to keep it.
fn accepted_body(original_prefix: &str, original_body: &str, replacement: &str) -> Option<String> {
    let mut body = replacement.trim();
    // A model may echo the bullet it was shown; the line keeps its own.
    if let Some(bullet) = original_prefix
        .trim_start()
        .chars()
        .next()
        .filter(|c| body.starts_with(*c))
    {
        body = body[bullet.len_utf8()..].trim_start();
    }
    let usable = !body.is_empty()
        && !body.contains(['\n', '\r'])
        && !crate::prompt_fence::contains_fence_tag(body)
        && digit_runs(body) == digit_runs(original_body);
    usable.then(|| body.to_string())
}

/// Apply `patches` to `document`. Only ids in `flagged` are touched; unknown
/// ids are ignored; the first patch for an id wins; a replacement that fails
/// [`accepted_body`] leaves its line as it was.
pub(crate) fn apply_patches(document: &str, flagged: &[FlaggedLine], patches: &[Patch]) -> String {
    let mut lines: Vec<String> = document.split('\n').map(str::to_string).collect();
    let mut done: Vec<usize> = Vec::new();
    for patch in patches {
        if !flagged.iter().any(|l| l.id == patch.id) || done.contains(&patch.id) {
            continue;
        }
        let Some(original) = lines.get(patch.id - 1) else {
            continue;
        };
        let (line, cr) = match original.strip_suffix('\r') {
            Some(line) => (line, "\r"),
            None => (original.as_str(), ""),
        };
        let (prefix, body) = split_prefix(line);
        if let Some(new_body) = accepted_body(prefix, body, &patch.replacement) {
            lines[patch.id - 1] = format!("{prefix}{new_body}{cr}");
            done.push(patch.id);
        }
    }
    lines.join("\n")
}
