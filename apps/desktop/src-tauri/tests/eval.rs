//! Offline eval harness — the DETERMINISTIC validator layer, measured.
//!
//! Two independent things live here:
//!
//! * **The eval corpus shape** (`tests/corpus/`): every resume `.txt` has a
//!   `.tags` sidecar, every tag line parses, the expected fields are present,
//!   and emails are synthetic. Extraction-level precision/recall against the
//!   real extractor is still future work; this guards the corpus so it can be
//!   built on.
//! * **Validator precision/recall** on the LABELLED defect fixtures the
//!   `validate::content` unit tests already own
//!   (`src/validate/content/fixtures/`). Each fixture differs from its clean
//!   sibling by roughly one planted edit, so "did the suite report exactly the
//!   planted code, and what else did it fire?" is a measurable question.
//!
//! ## What this measures, and what it asserts
//!
//! The MEASUREMENT is the point: a per-code table of recall (was the planted
//! defect reported?) and of the **Warning-tier false-positive rate on truthful
//! documents** — the number that has to be known before
//! `factual.unsourced_term` (or any other Warning) can be escalated to
//! Critical, and before an LLM judge is given more surface. Printing is not
//! enough to keep a table honest, though, so four things are ASSERTED:
//!
//! 1. every planted code is reported, whatever its severity;
//! 2. every planted code's severity is the one this file's label CLAIMS;
//! 3. every truthful fixture (`Negative` and `Tolerated`) reports zero
//!    Criticals, and a planted fixture raises no Critical BEYOND the one
//!    planted in it;
//! 4. the number of Warning findings on truthful fixtures stays inside
//!    [`WARNING_FP_BUDGET`].
//!
//! **(1) and (2) deliberately overlap `validate::content::test`** — the same
//! codes on the same fixtures are asserted there (e.g.
//! `fabricated_metric_is_critical_and_names_the_number`,
//! `near_duplicate_bullets_warn_once_on_the_later_bullet`). The first cut of
//! this harness avoided the duplication by gating only Criticals and treating
//! planted-Warning recall as data, and that made three of seven labels
//! DECORATION: relabelling the duplicate-bullets fixture to an unrelated
//! registered code still passed, and demoting a code from Critical to Warning
//! silently voided its only assertion here. A label nothing checks is a lie
//! waiting to happen, and this file's labels are the input to an escalation
//! decision. So the overlap is accepted on purpose: the unit suite proves the
//! validator's behaviour, this one proves the LABELS still describe it.
//!
//! What stays unique to this harness: the corpus-coverage invariant
//! ([`every_generated_fixture_is_labelled`]), the cross-fixture metrics table,
//! and the false-positive budget.
//!
//! Warnings on truthful documents are NOT asserted per code — they are summed
//! against [`WARNING_FP_BUDGET`]. One budget moves in one place with a reason
//! attached, where two dozen per-code assertions would each get loosened
//! individually and quietly.
//!
//! **Read today's `0` for what it is.** Two of the five negatives
//! (`*_generated_clean.txt`) are already pinned to a COMPLETELY empty report by
//! `clean_resume_produces_no_issues_at_all` and
//! `clean_german_resume_produces_no_issues_at_all` (one test each — the German
//! fixture is not covered by the English one), so their contribution to the
//! false-positive count is zero by construction; the paraphrased pair and the
//! grounded letter are the only ones free to move. The rate becomes an
//! informative measurement as truthful fixtures are ADDED — that, not this
//! run's number, is what gates the escalation decision.
//!
//! Generation itself needs a live model, so depth A/B over REAL runs is not
//! here: it reads `pipeline_runs.metrics_json` via
//! `scripts/dump-run-metrics.mjs`.
//!
//! The table is a LOCAL artifact: CI captures stdout for a passing test, so it
//! is visible only under `cargo test --test eval -- --nocapture`. Nothing
//! downstream parses it — every claim it makes that must hold is one of the
//! four assertions above.
//!
//! This links the crate (`ajh_tauri`, the `[lib]` target) and calls the same
//! `validate_content` entry point the product calls — never a stitched-together
//! subset of the individual validators, which would measure a suite that does
//! not ship.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ajh_tauri::validate::content::{
    severity_for, validate_content, ContentInput, ContentReport, DocKind,
};
use ajh_tauri::validate::Severity;

// ── Part 1: the extraction corpus's shape ───────────────────────────────────

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Parse a `.tags` sidecar: `key: value` lines; `#` comments and blanks ignored.
/// Repeatable keys (`section`, `link`) accumulate in order.
fn parse_tags(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            panic!("tag line {} is not `key: value`: {raw:?}", i + 1);
        };
        map.entry(k.trim().to_string())
            .or_default()
            .push(v.trim().to_string());
    }
    map
}

#[test]
fn corpus_fixtures_are_well_formed() {
    let dir = corpus_dir();
    let mut resumes = 0;

    for entry in fs::read_dir(&dir).expect("read tests/corpus directory") {
        let path = entry.expect("read dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        resumes += 1;

        let body = fs::read_to_string(&path).expect("read resume fixture");
        assert!(!body.trim().is_empty(), "{path:?} is empty");

        let tags_path = path.with_extension("tags");
        assert!(tags_path.exists(), "missing .tags sidecar for {path:?}");
        let tags = parse_tags(&fs::read_to_string(&tags_path).expect("read .tags sidecar"));

        // Every fixture declares at least a name, an email, and one section.
        for key in ["name", "email", "section"] {
            assert!(tags.contains_key(key), "{tags_path:?} tags missing `{key}`");
        }

        // No-PII guard: every declared email must be on a reserved example domain,
        // and must also literally appear in the resume body.
        for email in tags.get("email").into_iter().flatten() {
            assert!(
                email.contains("@example."),
                "fixture email must be synthetic: {email}"
            );
            assert!(
                body.contains(email.as_str()),
                "tagged email {email} not found in {path:?}"
            );
        }
    }

    assert!(
        resumes >= 2,
        "expected at least two corpus fixtures, found {resumes}"
    );
}

// ── Part 2: validator precision / recall on labelled defect fixtures ────────

const FIXTURES_SUBDIR: &str = "src/validate/content/fixtures";

const EN_SOURCE: &str = include_str!("../src/validate/content/fixtures/en_source_resume.txt");
const EN_JOB_AD: &str = include_str!("../src/validate/content/fixtures/en_job_ad.txt");
const DE_SOURCE: &str = include_str!("../src/validate/content/fixtures/de_source_resume.txt");
const DE_JOB_AD: &str = include_str!("../src/validate/content/fixtures/de_job_ad.txt");

/// What a fixture is FOR. The three cases are graded differently, so they are
/// three variants rather than one "expected codes" list with implicit rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Label {
    /// A defect was planted; every code here must be reported, AT the severity
    /// paired with it.
    ///
    /// The severity is carried rather than looked up because it is the thing
    /// being claimed: this harness exists to inform "should this Warning become
    /// a Critical?", so a code silently changing tier must fail here and be
    /// re-stated deliberately, not be absorbed by a `severity_for` call that
    /// agrees with whatever the table currently says.
    Planted(&'static [(&'static str, Severity)]),
    /// A truthful document. Zero Criticals is the invariant; every Warning is a
    /// false positive and counts against [`WARNING_FP_BUDGET`].
    Negative,
    /// A legal degradation — the source simply carried less. Also a truthful
    /// document, so it carries the same zero-Critical invariant; what it does
    /// NOT carry is a planted code or a clean-report claim, and its Warnings
    /// are reported without counting against the budget (the unit suite owns
    /// "this specific code must stay silent" for these).
    Tolerated,
}

/// One labelled fixture.
///
/// Built only through [`case!`], which derives `text` FROM `file` — the two
/// cannot name different fixtures, so the table can never grade one document
/// twice while reporting it as two.
struct Case {
    file: &'static str,
    text: &'static str,
    lang: &'static str,
    kind: DocKind,
    label: Label,
}

/// Declare a [`Case`], reading the fixture named by `$file` and nothing else.
///
/// `file` is not decoration: it is what [`every_generated_fixture_is_labelled`]
/// diffs against the directory and what a failure message names. Spelling the
/// path a second time inside `include_str!` let the two disagree — a row could
/// say `tier2` and grade `tier3`, passing every assertion while leaving one
/// fixture ungraded and another graded twice. One spelling, one fixture.
macro_rules! case {
    ($file:literal, $lang:literal, $kind:expr, $label:expr) => {
        Case {
            file: $file,
            text: include_str!(concat!("../src/validate/content/fixtures/", $file)),
            lang: $lang,
            kind: $kind,
            label: $label,
        }
    };
}

/// Every labelled fixture, with the code its planted edit is expected to
/// produce.
///
/// The labels are the ones `validate::content::test` already asserts — this
/// table is a second READER of those fixtures, never a second opinion about
/// what is planted in them.
const CASES: &[Case] = &[
    // Truthful documents — the false-positive denominator.
    case!(
        "en_generated_clean.txt",
        "en",
        DocKind::Resume,
        Label::Negative
    ),
    case!(
        "de_generated_clean.txt",
        "de",
        DocKind::Resume,
        Label::Negative
    ),
    case!(
        "en_generated_paraphrased.txt",
        "en",
        DocKind::Resume,
        Label::Negative
    ),
    case!(
        "de_generated_paraphrased.txt",
        "de",
        DocKind::Resume,
        Label::Negative
    ),
    case!(
        "en_letter_grounded.txt",
        "en",
        DocKind::CoverLetter,
        Label::Negative
    ),
    // Planted defects — the recall numerator.
    case!(
        "en_generated_fabricated_metric.txt",
        "en",
        DocKind::Resume,
        Label::Planted(&[("factual.unsourced_metric", Severity::Critical)])
    ),
    case!(
        "en_generated_dropped_role.txt",
        "en",
        DocKind::Resume,
        Label::Planted(&[("factual.dropped_role", Severity::Critical)])
    ),
    case!(
        "en_generated_altered_project_link.txt",
        "en",
        DocKind::Resume,
        Label::Planted(&[("factual.altered_project_link", Severity::Critical)])
    ),
    case!(
        "en_generated_wrong_language.txt",
        "en",
        DocKind::Resume,
        Label::Planted(&[("content.language_mismatch", Severity::Critical)])
    ),
    case!(
        "en_generated_duplicate_bullets.txt",
        "en",
        DocKind::Resume,
        Label::Planted(&[("duplicate.bullet", Severity::Warning)])
    ),
    case!(
        "en_generated_projects_broken.txt",
        "en",
        DocKind::Resume,
        Label::Planted(&[("consistency.project_structure", Severity::Warning)])
    ),
    case!(
        "en_letter_ai_tells.txt",
        "en",
        DocKind::CoverLetter,
        Label::Planted(&[
            ("voice.ai_tell_lexical", Severity::Warning),
            ("voice.template_opener", Severity::Warning),
        ])
    ),
    // Accepted degradations — truthful, but with less in the source to work
    // from. No planted code; still no Critical.
    case!(
        "en_generated_projects_tier2.txt",
        "en",
        DocKind::Resume,
        Label::Tolerated
    ),
    case!(
        "en_generated_projects_tier3.txt",
        "en",
        DocKind::Resume,
        Label::Tolerated
    ),
];

/// How many Warning FINDINGS the truthful fixtures may produce in total.
///
/// Findings, not codes: one unit here is one line a user is shown about a
/// document that is fine. The per-code table counts each code once per fixture
/// — right for "which check over-fires", wrong for a budget, since one code can
/// fire a dozen times in one document (the AI-tells letter carries 16 warnings
/// from 2 codes).
///
/// Zero today, and that is a measurement rather than an aspiration: the whole
/// truthful set comes back completely clean. It is a BUDGET rather than a
/// per-code assertion so that loosening it is one visible edit with a reason,
/// and it is asserted so the headline number in the printed table is a guard
/// instead of a comment.
const WARNING_FP_BUDGET: usize = 0;

/// The posting requirements each language's job ad is analysed into — the same
/// lists `validate::content::test` passes, so alignment findings here mean what
/// they mean there.
///
/// KNOWN DUPLICATION: these strings are hand-copied from that file's
/// `en_requirements()` and its German inline list, because they are private to
/// it. If the two drift, the alignment family's numbers here stop being
/// comparable with the unit suite's — which is why the lists are small, in one
/// function, and named.
fn requirements(lang: &str) -> Vec<String> {
    let raw: &[&str] = if lang == "de" {
        &["Docker und Kubernetes im Produktivbetrieb"]
    } else {
        &[
            "Strong Rust and Python",
            "Production Docker and Kubernetes",
            "PostgreSQL and Redis at scale",
            "Terraform and AWS",
        ]
    };
    raw.iter().map(|s| (*s).to_string()).collect()
}

fn run_case(case: &Case) -> ContentReport {
    // A letter never runs the alignment pass, and the unit suite's `en_letter`
    // helper passes NO requirements — feeding four here would be a different
    // input than the one this harness claims parity with.
    let reqs = if case.kind == DocKind::CoverLetter {
        Vec::new()
    } else {
        requirements(case.lang)
    };
    let (source, job_ad) = if case.lang == "de" {
        (DE_SOURCE, DE_JOB_AD)
    } else {
        (EN_SOURCE, EN_JOB_AD)
    };
    validate_content(&ContentInput {
        generated: case.text,
        source_resume: source,
        job_ad,
        top_requirements: &reqs,
        target_language: case.lang,
        doc_kind: case.kind,
    })
}

/// One fixture's outcome, kept so the whole table can be PRINTED before any
/// assertion fires — a harness that panics before it reports is a harness you
/// have to re-run to read.
struct Outcome {
    file: &'static str,
    label: Label,
    criticals: usize,
    warnings: usize,
    /// Every code the report carried, deduplicated, in stable order.
    fired: BTreeSet<&'static str>,
    /// Just the Critical-severity codes. Kept separately because "did this
    /// document raise a Critical nobody planted" cannot be answered from
    /// [`Self::fired`] (which has no severities) or from [`Self::criticals`]
    /// (which has no codes).
    critical_codes: BTreeSet<&'static str>,
}

/// Per-code tallies across the whole corpus.
#[derive(Default)]
struct CodeStats {
    /// Fixtures where this code was planted.
    planted: usize,
    /// …of which the suite actually reported it.
    recalled: usize,
    /// Truthful (`Label::Negative`) fixtures that fired it — false positives,
    /// and the only tally that counts against [`WARNING_FP_BUDGET`].
    fp_negatives: usize,
    /// `Tolerated` fixtures that fired it. Reported in its OWN column: a
    /// degradation fixture firing a structure warning is a different event from
    /// a defect fixture firing a code nobody planted, and summing the two hid
    /// which had happened.
    fired_tolerated: usize,
    /// Planted fixtures that fired it while it was NOT the planted code.
    fired_offtarget: usize,
}

fn severity_label(code: &str) -> &'static str {
    match severity_for(code) {
        Severity::Critical => "critical",
        Severity::Warning => "warning",
    }
}

/// The `.txt` fixtures that are GENERATED output (the gradeable ones) — every
/// file that is neither a source résumé nor a job ad.
fn gradeable_fixture_files() -> BTreeSet<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_SUBDIR);
    fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {dir:?}: {e}"))
        .map(|entry| {
            // NOT `entry.ok()?`: an unreadable entry would silently SHRINK the
            // corpus this guard compares against, which is the one failure it
            // cannot be allowed to have.
            entry
                .unwrap_or_else(|e| panic!("read a fixture dir entry in {dir:?}: {e}"))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| {
            name.ends_with(".txt")
                && !name.ends_with("_source_resume.txt")
                && !name.ends_with("_job_ad.txt")
        })
        .collect()
}

#[test]
fn validator_metrics_over_labelled_fixtures() {
    // ── Measure ─────────────────────────────────────────────────────────────
    let outcomes: Vec<Outcome> = CASES
        .iter()
        .map(|case| {
            let report = run_case(case);
            Outcome {
                file: case.file,
                label: case.label,
                criticals: report
                    .issues
                    .iter()
                    .filter(|i| i.severity == Severity::Critical)
                    .count(),
                warnings: report
                    .issues
                    .iter()
                    .filter(|i| i.severity == Severity::Warning)
                    .count(),
                fired: report.issues.iter().map(|i| i.code).collect(),
                critical_codes: report
                    .issues
                    .iter()
                    .filter(|i| i.severity == Severity::Critical)
                    .map(|i| i.code)
                    .collect(),
            }
        })
        .collect();

    let negatives = outcomes
        .iter()
        .filter(|o| o.label == Label::Negative)
        .count();

    let mut stats: BTreeMap<&str, CodeStats> = BTreeMap::new();
    for outcome in &outcomes {
        let planted: &[(&str, Severity)] = match outcome.label {
            Label::Planted(codes) => codes,
            _ => &[],
        };
        for (code, _) in planted {
            let entry = stats.entry(code).or_default();
            entry.planted += 1;
            if outcome.fired.contains(code) {
                entry.recalled += 1;
            }
        }
        for code in &outcome.fired {
            let entry = stats.entry(code).or_default();
            match outcome.label {
                Label::Negative => entry.fp_negatives += 1,
                Label::Tolerated => entry.fired_tolerated += 1,
                Label::Planted(_) if !planted.iter().any(|(c, _)| c == code) => {
                    entry.fired_offtarget += 1;
                }
                Label::Planted(_) => {}
            }
        }
    }

    // ── Report (visible with `-- --nocapture`) ──────────────────────────────
    println!("\n=== eval: deterministic content validators on labelled fixtures ===");
    println!(
        "{} fixtures — {} planted, {negatives} truthful, {} tolerated-degradation\n",
        outcomes.len(),
        outcomes
            .iter()
            .filter(|o| matches!(o.label, Label::Planted(_)))
            .count(),
        outcomes
            .iter()
            .filter(|o| o.label == Label::Tolerated)
            .count(),
    );

    println!(
        "{:<44} {:<10} {:>4} {:>4}  planted → found | other codes fired",
        "fixture", "label", "crit", "warn"
    );
    for outcome in &outcomes {
        let (label, detail) = match outcome.label {
            Label::Planted(codes) => {
                let found: Vec<String> = codes
                    .iter()
                    .map(|(c, _)| {
                        format!(
                            "{c}→{}",
                            if outcome.fired.contains(c) {
                                "FOUND"
                            } else {
                                "MISSED"
                            }
                        )
                    })
                    .collect();
                let others: Vec<&str> = outcome
                    .fired
                    .iter()
                    .filter(|c| !codes.iter().any(|(planted, _)| planted == *c))
                    .copied()
                    .collect();
                (
                    "planted",
                    format!("{} | {}", found.join(" "), join_or_dash(&others)),
                )
            }
            Label::Negative => (
                "truthful",
                format!(
                    "— | {}",
                    join_or_dash(&outcome.fired.iter().copied().collect::<Vec<_>>())
                ),
            ),
            Label::Tolerated => (
                "tolerated",
                format!(
                    "— | {}",
                    join_or_dash(&outcome.fired.iter().copied().collect::<Vec<_>>())
                ),
            ),
        };
        println!(
            "{:<44} {label:<10} {:>4} {:>4}  {detail}",
            outcome.file, outcome.criticals, outcome.warnings
        );
    }

    println!(
        "\n{:<36} {:<9} {:>7} {:>8} {:>9} {:>8} {:>10} {:>9}",
        "code",
        "severity",
        "planted",
        "recall",
        "fp/truthful",
        "fp-rate",
        "off-target",
        "tolerated"
    );
    for (code, s) in &stats {
        let recall = if s.planted == 0 {
            "—".to_string()
        } else {
            format!("{}/{}", s.recalled, s.planted)
        };
        let fp_rate = if negatives == 0 {
            "—".to_string()
        } else {
            format!("{:.0}%", 100.0 * s.fp_negatives as f64 / negatives as f64)
        };
        println!(
            "{code:<36} {:<9} {:>7} {recall:>8} {:>9} {fp_rate:>8} {:>10} {:>9}",
            severity_label(code),
            s.planted,
            format!("{}/{negatives}", s.fp_negatives),
            s.fired_offtarget,
            s.fired_tolerated,
        );
    }

    // FINDINGS, not codes. The per-code table above counts each code once per
    // fixture, which is the right shape for "which check over-fires" and the
    // WRONG shape for a budget: the letter fixture alone carries 16 warnings from
    // 2 codes, so a code-based sum understates by 8× exactly where it matters.
    // What a user is shown is one line per finding, so that is what is budgeted.
    let warning_fps: usize = outcomes
        .iter()
        .filter(|o| o.label == Label::Negative)
        .map(|o| o.warnings)
        .sum();
    let pinned_empty = outcomes
        .iter()
        .filter(|o| o.label == Label::Negative && o.file.contains("_generated_clean"))
        .count();
    println!(
        "\nWarning findings on truthful documents: {warning_fps} (budget {WARNING_FP_BUDGET}) \
         across {negatives} fixtures — the number that gates any W→C escalation.\nCaveat: \
         {pinned_empty} of those {negatives} are already pinned to a completely empty report by \
         `clean_resume_produces_no_issues_at_all` / \
         `clean_german_resume_produces_no_issues_at_all`, so only {} can move.\n",
        negatives - pinned_empty
    );

    // ── Assert ──────────────────────────────────────────────────────────────
    for outcome in &outcomes {
        if let Label::Planted(codes) = outcome.label {
            for (code, expected) in codes {
                // The severity this file CLAIMS, checked against the registry.
                // A demotion (or promotion) is exactly the event this harness
                // informs, so it must be re-stated here rather than absorbed.
                assert_eq!(
                    severity_for(code),
                    *expected,
                    "{}: {code} is registered as {:?} but this harness labels it {expected:?} — \
                     re-state the label if the tier change was deliberate",
                    outcome.file,
                    severity_for(code),
                );
                // Recall, at EVERY severity: a planted label that nothing checks
                // is decoration, and three of these were exactly that while only
                // Criticals were gated.
                assert!(
                    outcome.fired.contains(code),
                    "{}: planted {expected:?} {code} was NOT reported; the report carried {:?}",
                    outcome.file,
                    outcome.fired
                );
            }
            // A defect fixture is one planted edit away from its clean sibling,
            // so any OTHER Critical on it is a false accusation against a
            // document that is truthful in every respect but the planted one —
            // and nothing else gates that: the zero-Critical rule covers only
            // the truthful labels, and the off-target column is print-only.
            let unplanted: Vec<&str> = outcome
                .critical_codes
                .iter()
                .filter(|c| !codes.iter().any(|(planted, _)| planted == *c))
                .copied()
                .collect();
            assert!(
                unplanted.is_empty(),
                "{}: raised Critical(s) nobody planted: {unplanted:?}. Everything in this fixture \
                 except the planted edit is true, so an extra Critical is a false accusation.",
                outcome.file
            );
        }
        // Both truthful kinds. A `Tolerated` fixture is a legal degradation, not
        // a defective document: a Critical on one is a regression, and nothing
        // else gates it.
        if matches!(outcome.label, Label::Negative | Label::Tolerated) {
            assert_eq!(
                outcome.criticals, 0,
                "{}: a truthful document must never raise a Critical; it fired {:?}",
                outcome.file, outcome.fired
            );
        }
    }

    // The headline number, as a guard rather than a remark.
    //
    // `<=` against a budget that happens to be 0 today is
    // `absurd_extreme_comparisons`, and the lint is right about the arithmetic
    // and wrong about the intent: the comparison is the CONTRACT ("at most the
    // budget"), and rewriting it as `== 0` would have to be rewritten back the
    // first time the budget moves off zero — exactly the edit most likely to be
    // made carelessly.
    #[allow(clippy::absurd_extreme_comparisons)]
    let within_budget = warning_fps <= WARNING_FP_BUDGET;
    assert!(
        within_budget,
        "Warning findings on truthful documents rose to {warning_fps}, over the budget of \
         {WARNING_FP_BUDGET}. Either the validator over-fires, or the budget moves WITH a \
         reason — see the table above for which codes fired."
    );
}

/// Every gradeable fixture on disk carries a label, so a fixture added for a new
/// defect cannot sit in the corpus unmeasured.
///
/// Only this direction needs a runtime check. The reverse — a `CASES` row naming
/// a fixture that no longer exists — cannot compile: [`case!`] builds its
/// `include_str!` path out of the same `file` literal, so a deleted fixture is a
/// BUILD error, and a runtime assertion for it would be unreachable code
/// pretending to be a guard.
#[test]
fn every_generated_fixture_is_labelled() {
    let on_disk = gradeable_fixture_files();
    let labelled: BTreeSet<String> = CASES.iter().map(|c| c.file.to_string()).collect();

    let unlabelled: Vec<&String> = on_disk.difference(&labelled).collect();
    assert!(
        unlabelled.is_empty(),
        "these fixtures are graded by nothing — add them to `CASES`: {unlabelled:?}"
    );
}

fn join_or_dash(codes: &[&str]) -> String {
    if codes.is_empty() {
        "—".to_string()
    } else {
        codes.join(", ")
    }
}
