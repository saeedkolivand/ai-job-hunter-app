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

use crate::validate::content::{line_carries_phrase, normalize_language, urls_in, ContentReport};

use crate::pipeline::resume::prompts::HUMANIZE_DOCUMENT_CAP;

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
pub(crate) fn flagged_lines(report: &ContentReport, document: &str, lang: &str) -> Flags {
    let lang = normalize_language(lang);
    let lines: Vec<&str> = document.split('\n').collect();
    let linked: Vec<bool> = lines.iter().map(|l| !urls_in(l).is_empty()).collect();
    let mut flags = Flags::default();
    for issue in eligible_voice_issues(report, document) {
        let needle = issue
            .evidence
            .as_deref()
            .map(str::trim)
            .filter(|e| !e.is_empty());
        let hits: Vec<usize> = needle.map_or_else(Vec::new, |needle| {
            (0..lines.len())
                .filter(|&i| !linked[i] && line_carries_phrase(lines[i], needle, &lang))
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

/// How a document is humanized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Line-locatable flags: the model returns line patches.
    Patch,
    /// Only document-wide flags (no line to patch): the model re-emits the
    /// whole document, as before line patches existed.
    Rewrite,
}

impl Mode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Mode::Patch => "patch",
            Mode::Rewrite => "rewrite",
        }
    }
}

/// Route a document's flags: any line-locatable flag means patches
/// (document-wide ones then stay context); document-wide flags alone mean a
/// whole-document rewrite; nothing eligible means no call.
pub(crate) fn humanize_mode(flags: &Flags) -> Option<Mode> {
    if !flags.lines.is_empty() {
        Some(Mode::Patch)
    } else if !flags.document_wide.is_empty() {
        Some(Mode::Rewrite)
    } else {
        None
    }
}

/// Chars of excerpt the model may be shown. Below the prompt fence's own cap
/// (`HUMANIZE_DOCUMENT_CAP`) with headroom, because `fenced` truncates with NO
/// marker: an over-long excerpt would cut a flagged line mid-sentence, and
/// `apply_patches` would then replace the FULL line with the model's patch of
/// the cut text.
const EXCERPT_BUDGET: usize = HUMANIZE_DOCUMENT_CAP - 1_000;

/// Render the lines `shown` (sorted ids): `{id}> line` for a flagged line,
/// `{id}| line` for context, `...` where lines were left out.
fn render(doc: &[&str], flagged: &[&FlaggedLine], shown: &[usize]) -> String {
    let mut out = String::new();
    let mut prev = 0;
    for &id in shown {
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

/// The numbered excerpt the model sees, built to [`EXCERPT_BUDGET`], plus the
/// flagged lines actually IN it. A flagged line (with its neighbours) that
/// would not fit whole is left out and never offered, so no line is shown
/// truncated; the caller must patch only the returned lines.
pub(crate) fn excerpt(document: &str, flagged: &[FlaggedLine]) -> (String, Vec<FlaggedLine>) {
    excerpt_within(document, flagged, EXCERPT_BUDGET)
}

pub(crate) fn excerpt_within(
    document: &str,
    flagged: &[FlaggedLine],
    budget: usize,
) -> (String, Vec<FlaggedLine>) {
    let doc: Vec<&str> = document.split('\n').collect();
    let mut kept: Vec<&FlaggedLine> = Vec::new();
    let mut shown: Vec<usize> = Vec::new();
    let mut out = String::new();
    for line in flagged {
        let lo = line.id.saturating_sub(CONTEXT_LINES).max(1);
        let mut trial = shown.clone();
        trial.extend(lo..=(line.id + CONTEXT_LINES).min(doc.len()));
        trial.sort_unstable();
        trial.dedup();
        let mut trial_kept = kept.clone();
        trial_kept.push(line);
        let rendered = render(&doc, &trial_kept, &trial);
        if rendered.chars().count() <= budget {
            (kept, shown, out) = (trial_kept, trial, rendered);
        }
    }
    (out, kept.into_iter().cloned().collect())
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

/// Split a line into its indent + marker (a single bullet or a `#` heading
/// run, followed by whitespace) and the text after it. A `**bold` opener is
/// NOT a bullet.
fn split_prefix(line: &str) -> (&str, &str) {
    let rest = line.trim_start();
    let mut len = line.len() - rest.len();
    let marker = rest
        .chars()
        .next()
        .filter(|c| BULLETS.contains(*c) || *c == '#');
    if let Some(marker) = marker {
        let run = rest.len() - rest.trim_start_matches(marker).len();
        let after = &rest[run..];
        let spaced = after.is_empty() || after.starts_with(char::is_whitespace);
        if spaced && (marker == '#' || run == marker.len_utf8()) {
            len += run + (after.len() - after.trim_start().len());
        }
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
    // A model may echo the bullet/heading marker it was shown, or drop it; the
    // line keeps its own marker either way.
    if let Some(marker) = original_prefix
        .trim_start()
        .chars()
        .next()
        .filter(|c| body.starts_with(*c))
    {
        // Exactly ONE bullet (or one `#` run) followed by whitespace: a
        // replacement starting `**Bold**` is not an echoed `*` bullet.
        let run = body.len() - body.trim_start_matches(marker).len();
        let after = &body[run..];
        if (marker == '#' || run == marker.len_utf8()) && after.starts_with(char::is_whitespace) {
            body = after.trim_start();
        }
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
        // Marked done BEFORE validation: a rejected first patch is not
        // followed by a second one for the same id.
        done.push(patch.id);
        let (prefix, body) = split_prefix(line);
        if let Some(new_body) = accepted_body(prefix, body, &patch.replacement) {
            lines[patch.id - 1] = format!("{prefix}{new_body}{cr}");
        }
    }
    lines.join("\n")
}
