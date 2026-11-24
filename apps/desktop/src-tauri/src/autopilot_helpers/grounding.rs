//! Deterministic grounding for AI fit notes (#1446, slice 1).
//!
//! Three pure pieces, no model involved:
//! * [`classify_tier`] — is an ad line a must-have, a preference, or unspecified (EN/DE/IT cues);
//! * [`language_levels`] — the `(language, level)` pairs a text asserts (CEFR or fluency words);
//! * [`ground_note`] — drops note sentences that claim a candidate language level the résumé does
//!   not evidence, and appends the ad's unmet must-haves. Unknown stays unknown: a requirement
//!   the résumé is silent on is listed as "to verify", never as a gap.

mod language;
use language::FLUENT;
pub(crate) use language::{evidenced_levels, language_levels, Level};

/// How strongly an ad line demands something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tier {
    Mandatory,
    Preferred,
    Unspecified,
}

/// Cues that make a line a preference. Checked FIRST so "not required" / "a plus" never read as
/// mandatory; a mixed line is resolved toward `Preferred` (never an absolute exclusion).
const PREFERRED_CUES: &[&str] = &[
    "nice to have",
    "nice-to-have",
    "preferred",
    "a plus",
    "an advantage",
    "advantageous",
    "desirable",
    "bonus",
    "ideally",
    "not required",
    "not necessary",
    "wünschenswert",
    "von vorteil",
    "idealerweise",
    "nicht erforderlich",
    "nicht zwingend",
    "preferenzial",
    "gradit",
    "non richiest",
    "non necessari",
    "costituisce titolo",
    "è un plus",
];

const MANDATORY_CUES: &[&str] = &[
    "required",
    "requirement",
    "must have",
    "must-have",
    "must be",
    "must speak",
    "mandatory",
    "essential",
    "minimum",
    "zwingend",
    "erforderlich",
    "voraussetzung",
    "vorausgesetzt",
    "unbedingt",
    "notwendig",
    "indispensabil",
    "obbligatori",
    "richiest",
    "requisito",
    "imprescindibil",
    "almeno",
];

fn has_any(lower: &str, cues: &[&str]) -> bool {
    cues.iter().any(|c| lower.contains(c))
}

/// Minimum years an ad line demands ("7+ years", "almeno 3 anni", "5 Jahre"), if it states one.
fn years_required(lower: &str) -> Option<u32> {
    let ws: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '+')
        .filter(|w| !w.is_empty())
        .collect();
    ws.windows(2).find_map(|p| {
        let unit = p[1];
        let is_unit = ["year", "jahr", "anni", "anno", "ans"]
            .iter()
            .any(|u| unit.starts_with(u));
        let n: String = p[0].chars().take_while(char::is_ascii_digit).collect();
        (is_unit && !n.is_empty()).then(|| n.parse().ok()).flatten()
    })
}

/// Tier of one ad line: preference cues win, then must-have cues, a stated minimum-years figure,
/// or a fluent/C1+/native language demand; otherwise unspecified.
pub(crate) fn classify_tier(line: &str) -> Tier {
    let lower = line.to_lowercase();
    if has_any(&lower, PREFERRED_CUES) {
        return Tier::Preferred;
    }
    let demands_fluency = language_levels(line).iter().any(|(_, l)| *l >= FLUENT);
    if has_any(&lower, MANDATORY_CUES) || years_required(&lower).is_some() || demands_fluency {
        Tier::Mandatory
    } else {
        Tier::Unspecified
    }
}

/// One ad requirement line with its tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Requirement {
    pub(crate) text: String,
    pub(crate) tier: Tier,
}

/// Split an ad into requirement lines and tier each. A short heading that carries a preference cue
/// ("Nice to have:") demotes the lines under it until the next heading.
pub(crate) fn classify_requirements(description: &str) -> Vec<Requirement> {
    let mut in_preferred_section = false;
    let mut out = Vec::new();
    for raw in description.split(['\n', '•', ';']) {
        let line = raw.trim().trim_start_matches(['-', '*', '–']).trim();
        if line.is_empty() {
            continue;
        }
        if line.chars().count() <= 60 && line.ends_with(':') {
            in_preferred_section = has_any(&line.to_lowercase(), PREFERRED_CUES);
            continue;
        }
        let mut tier = classify_tier(line);
        if in_preferred_section {
            tier = Tier::Preferred;
        }
        if tier != Tier::Unspecified {
            out.push(Requirement {
                text: line.to_string(),
                tier,
            });
        }
    }
    out
}

fn level_name(level: Level) -> &'static str {
    match level {
        0 | 1 => "A1",
        2 => "A2",
        3 => "B1",
        4 => "B2",
        5 => "C1",
        6 => "C2",
        _ => "native",
    }
}

/// What the ad's must-haves ask of the candidate versus what the résumé evidences.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct MandatoryReport {
    /// Must-have language demands the résumé evidences at a LOWER level.
    pub(crate) gaps: Vec<String>,
    /// Must-haves the résumé cannot confirm either way (unknown, not a gap).
    pub(crate) to_verify: Vec<String>,
}

/// Char cap on a requirement line when shown back to the user.
const REQ_DISPLAY_CHARS: usize = 160;

pub(crate) fn mandatory_report(resume: &str, description: &str) -> MandatoryReport {
    let evidence = evidenced_levels(resume);
    let mut report = MandatoryReport::default();
    for req in classify_requirements(description)
        .into_iter()
        .filter(|r| r.tier == Tier::Mandatory)
    {
        let lower = req.text.to_lowercase();
        if years_required(&lower).is_some() {
            // Analysis ran on the full line; only the displayed copy is truncated.
            let shown: String = req.text.chars().take(REQ_DISPLAY_CHARS).collect();
            if !report.to_verify.contains(&shown) {
                report.to_verify.push(shown);
            }
        }
        // An explicit must-have cue makes every stated level binding; the fluency threshold only
        // decides which levels count when mandatory status was merely inferred.
        let explicit = has_any(&lower, MANDATORY_CUES);
        for (lang, need) in language_levels(&req.text)
            .into_iter()
            .filter(|(_, l)| explicit || *l >= FLUENT)
        {
            match evidence.get(lang) {
                Some(have) if *have < need => {
                    let gap = format!(
                        "{lang} {} required, résumé shows {}",
                        level_name(need),
                        level_name(*have)
                    );
                    if !report.gaps.contains(&gap) {
                        report.gaps.push(gap);
                    }
                }
                Some(_) => {}
                None => {
                    let item = format!("{lang} {}", level_name(need));
                    if !report.to_verify.contains(&item) {
                        report.to_verify.push(item);
                    }
                }
            }
        }
    }
    report
}

/// Echoing the ad's demand ("requires fluent Italian") is not a claim about the candidate.
const ECHO_CUES: &[&str] = &[
    "require",
    "need",
    "must",
    "mandatory",
    "looking for",
    "seeks",
    "erforder",
    "richied",
    "richiest",
    "verlangt",
    "voraus",
];

fn sentences(note: &str) -> Vec<&str> {
    note.split_inclusive(['.', '!', '?', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Words that make a sentence a statement about the candidate: a restatement of the ad that also
/// speaks about them ("You meet the requirement for fluent Italian") is still a claim.
const CANDIDATE_WORDS: &[&str] = &[
    "you",
    "your",
    "candidate",
    "resume",
    "résumé",
    "cv",
    "meet",
    "meets",
    "du",
    "dein",
    "deine",
    "sie",
    "ihr",
    "tu",
    "lei",
];

fn restates_the_ad(lower: &str) -> bool {
    has_any(lower, ECHO_CUES)
        && !lower
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| CANDIDATE_WORDS.contains(&w))
}

/// The note's sentences minus every one that claims a fluent/native language the résumé does not
/// evidence at that level.
fn grounded_prose(note: &str, resume: &str) -> String {
    let evidence = evidenced_levels(resume);
    sentences(note)
        .into_iter()
        .filter(|s| {
            restates_the_ad(&s.to_lowercase())
                || !language_levels(s)
                    .iter()
                    .any(|(lang, l)| *l >= FLUENT && evidence.get(lang).copied().unwrap_or(0) < *l)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The ad's unmet / to-verify must-haves as appended note text ("" when there are none).
fn report_suffix(resume: &str, description: &str) -> String {
    let report = mandatory_report(resume, description);
    [
        ("Unmet must-have", &report.gaps),
        ("To verify (must-have)", &report.to_verify),
    ]
    .iter()
    .filter(|(_, items)| !items.is_empty())
    .map(|(label, items)| format!("{label}: {}.", items.join("; ")))
    .collect::<Vec<_>>()
    .join(" ")
}

fn join_note(prose: &str, suffix: &str) -> String {
    match (prose.is_empty(), suffix.is_empty()) {
        (_, true) => prose.to_string(),
        (true, false) => suffix.to_string(),
        _ => format!("{prose} {suffix}"),
    }
}

/// Drop every sentence that claims a fluent/native language the résumé does not evidence at that
/// level, then append the ad's unmet must-haves. Never adds a positive claim.
#[cfg(test)]
pub(crate) fn ground_note(note: &str, resume: &str, description: &str) -> String {
    join_note(
        &grounded_prose(note, resume),
        &report_suffix(resume, description),
    )
}

/// Char cap on a stored note — defense-in-depth on output size: the system prompt asks for 2–4
/// sentences and the provider layer exposes no max-tokens knob.
const NOTE_CAP: usize = 800;

/// A raw completion → the stored note: grounded, with prose + appended must-have report together
/// within [`NOTE_CAP`] chars. The report (the warnings) is reserved first; the prose gives way.
pub(crate) fn finish_note(text: &str, resume: &str, description: Option<&str>) -> String {
    let suffix: String = report_suffix(resume, description.unwrap_or(""))
        .chars()
        .take(NOTE_CAP)
        .collect();
    let reserved = if suffix.is_empty() {
        0
    } else {
        suffix.chars().count() + 1
    };
    let capped: String = text.trim().chars().take(NOTE_CAP).collect();
    let prose: String = grounded_prose(&capped, resume)
        .chars()
        .take(NOTE_CAP - reserved.min(NOTE_CAP))
        .collect();
    join_note(prose.trim_end(), &suffix)
}

#[cfg(test)]
mod tests;
