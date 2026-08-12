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
//! ## Why this is metrics, not another pass/fail suite
//!
//! `validate::content::test` already asserts, per defect, that the right code
//! fires with the right evidence. Repeating that here would buy nothing. What
//! nothing measured before is the **Warning-tier false-positive rate on
//! truthful documents** — the number that has to be known before
//! `factual.unsourced_term` (or any other Warning) can be escalated to
//! Critical, and before an LLM judge is given more surface. So this harness
//! prints a per-code table (`cargo test --test eval -- --nocapture`) and
//! asserts only the two invariants that must never regress:
//!
//! 1. every planted **Critical** defect is reported, under its expected code;
//! 2. the clean / paraphrased / grounded negatives report **zero Criticals**.
//!
//! Warnings on negatives are DATA, not failures — a harness that failed on them
//! would be a duplicate of the unit suite and would have to be loosened the
//! first time a threshold moved, which is exactly when the number matters.
//!
//! Planted-Warning recall is likewise reported rather than asserted: the unit
//! suite pins those (`near_duplicate_bullets_warn_once_on_the_later_bullet`,
//! `project_outside_the_three_tiers_warns`), and duplicating an assertion in two
//! files means fixing it in two files.
//!
//! **Read today's `0%` for what it is.** Two of the five negatives
//! (`*_generated_clean.txt`) are already pinned to a COMPLETELY empty report by
//! `clean_resume_produces_no_issues_at_all`, so their contribution to the
//! false-positive rate is zero by construction; the paraphrased pair and the
//! grounded letter are the only ones free to move. The rate becomes an
//! informative measurement as truthful fixtures are ADDED — that, not this
//! run's number, is what gates the escalation decision.
//!
//! Generation itself needs a live model, so depth A/B over REAL runs is not
//! here: it reads `pipeline_runs.metrics_json` via
//! `scripts/dump-run-metrics.mjs`.
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
    /// A defect was planted; these codes must be reported. Criticals among them
    /// are asserted, Warnings among them are measured (see the module docs).
    Planted(&'static [&'static str]),
    /// A truthful document. Zero Criticals is the invariant; every Warning is a
    /// false positive and is counted as one.
    Negative,
    /// A legal degradation — the source simply carried less. No planted defect
    /// and no clean-report claim, so it contributes data only. (The unit suite
    /// owns the "this specific code must stay silent" assertion for these.)
    Tolerated,
}

/// One labelled fixture. `file` is the on-disk name so the coverage guard below
/// can prove the whole corpus is graded, and so a failure names the file.
struct Case {
    file: &'static str,
    text: &'static str,
    lang: &'static str,
    kind: DocKind,
    label: Label,
}

/// Every labelled fixture, with the code its planted edit is expected to
/// produce.
///
/// The labels are the ones `validate::content::test` already asserts — this
/// table is a second READER of those fixtures, never a second opinion about
/// what is planted in them.
const CASES: &[Case] = &[
    // Truthful documents — the precision denominator.
    Case {
        file: "en_generated_clean.txt",
        text: include_str!("../src/validate/content/fixtures/en_generated_clean.txt"),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Negative,
    },
    Case {
        file: "de_generated_clean.txt",
        text: include_str!("../src/validate/content/fixtures/de_generated_clean.txt"),
        lang: "de",
        kind: DocKind::Resume,
        label: Label::Negative,
    },
    Case {
        file: "en_generated_paraphrased.txt",
        text: include_str!("../src/validate/content/fixtures/en_generated_paraphrased.txt"),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Negative,
    },
    Case {
        file: "de_generated_paraphrased.txt",
        text: include_str!("../src/validate/content/fixtures/de_generated_paraphrased.txt"),
        lang: "de",
        kind: DocKind::Resume,
        label: Label::Negative,
    },
    Case {
        file: "en_letter_grounded.txt",
        text: include_str!("../src/validate/content/fixtures/en_letter_grounded.txt"),
        lang: "en",
        kind: DocKind::CoverLetter,
        label: Label::Negative,
    },
    // Planted defects — the recall numerator.
    Case {
        file: "en_generated_fabricated_metric.txt",
        text: include_str!("../src/validate/content/fixtures/en_generated_fabricated_metric.txt"),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Planted(&["factual.unsourced_metric"]),
    },
    Case {
        file: "en_generated_dropped_role.txt",
        text: include_str!("../src/validate/content/fixtures/en_generated_dropped_role.txt"),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Planted(&["factual.dropped_role"]),
    },
    Case {
        file: "en_generated_altered_project_link.txt",
        text: include_str!(
            "../src/validate/content/fixtures/en_generated_altered_project_link.txt"
        ),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Planted(&["factual.altered_project_link"]),
    },
    Case {
        file: "en_generated_wrong_language.txt",
        text: include_str!("../src/validate/content/fixtures/en_generated_wrong_language.txt"),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Planted(&["content.language_mismatch"]),
    },
    Case {
        file: "en_generated_duplicate_bullets.txt",
        text: include_str!("../src/validate/content/fixtures/en_generated_duplicate_bullets.txt"),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Planted(&["duplicate.bullet"]),
    },
    Case {
        file: "en_generated_projects_broken.txt",
        text: include_str!("../src/validate/content/fixtures/en_generated_projects_broken.txt"),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Planted(&["consistency.project_structure"]),
    },
    Case {
        file: "en_letter_ai_tells.txt",
        text: include_str!("../src/validate/content/fixtures/en_letter_ai_tells.txt"),
        lang: "en",
        kind: DocKind::CoverLetter,
        label: Label::Planted(&["voice.ai_tell_lexical", "voice.template_opener"]),
    },
    // Accepted degradations.
    Case {
        file: "en_generated_projects_tier2.txt",
        text: include_str!("../src/validate/content/fixtures/en_generated_projects_tier2.txt"),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Tolerated,
    },
    Case {
        file: "en_generated_projects_tier3.txt",
        text: include_str!("../src/validate/content/fixtures/en_generated_projects_tier3.txt"),
        lang: "en",
        kind: DocKind::Resume,
        label: Label::Tolerated,
    },
];

/// The posting requirements each language's job ad is analysed into — the same
/// lists `validate::content::test` passes, so alignment findings here mean what
/// they mean there.
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
    let reqs = requirements(case.lang);
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
}

/// Per-code tallies across the whole corpus.
#[derive(Default)]
struct CodeStats {
    /// Fixtures where this code was planted.
    planted: usize,
    /// …of which the suite actually reported it.
    recalled: usize,
    /// Truthful (`Label::Negative`) fixtures that fired it — false positives.
    fp_negatives: usize,
    /// `Tolerated` fixtures that fired it — data, not a verdict.
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
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().to_string_lossy().into_owned();
            let gradeable = name.ends_with(".txt")
                && !name.ends_with("_source_resume.txt")
                && !name.ends_with("_job_ad.txt");
            gradeable.then_some(name)
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
            }
        })
        .collect();

    let negatives = outcomes
        .iter()
        .filter(|o| o.label == Label::Negative)
        .count();

    let mut stats: BTreeMap<&str, CodeStats> = BTreeMap::new();
    for outcome in &outcomes {
        let planted: &[&str] = match outcome.label {
            Label::Planted(codes) => codes,
            _ => &[],
        };
        for code in planted {
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
                Label::Planted(_) if !planted.contains(code) => entry.fired_offtarget += 1,
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
                    .map(|c| {
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
                    .filter(|c| !codes.contains(c))
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
        "\n{:<36} {:<9} {:>7} {:>8} {:>9} {:>8} {:>9}",
        "code", "severity", "planted", "recall", "fp/truthful", "fp-rate", "off-target"
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
            "{code:<36} {:<9} {:>7} {recall:>8} {:>9} {fp_rate:>8} {:>9}",
            severity_label(code),
            s.planted,
            format!("{}/{negatives}", s.fp_negatives),
            s.fired_offtarget + s.fired_tolerated,
        );
    }

    let warning_fps: usize = stats
        .iter()
        .filter(|(code, _)| severity_for(code) == Severity::Warning)
        .map(|(_, s)| s.fp_negatives)
        .sum();
    println!(
        "\nWarning-tier false positives on truthful documents: {warning_fps} across {negatives} \
         fixtures — the number that gates any W→C escalation.\n"
    );

    // ── Assert (only the invariants) ────────────────────────────────────────
    for outcome in &outcomes {
        if let Label::Planted(codes) = outcome.label {
            for code in codes {
                if severity_for(code) != Severity::Critical {
                    continue; // Warning recall is measured above, not gated here.
                }
                assert!(
                    outcome.fired.contains(code),
                    "{}: planted Critical {code} was NOT reported; the report carried {:?}",
                    outcome.file,
                    outcome.fired
                );
            }
        }
        if outcome.label == Label::Negative {
            assert_eq!(
                outcome.criticals, 0,
                "{}: a truthful document must never raise a Critical; it fired {:?}",
                outcome.file, outcome.fired
            );
        }
    }
}

/// Every gradeable fixture on disk carries a label, so a fixture added for a new
/// defect cannot sit in the corpus unmeasured (and a deleted one cannot leave a
/// row in `CASES` that grades nothing).
#[test]
fn every_generated_fixture_is_labelled() {
    let on_disk = gradeable_fixture_files();
    let labelled: BTreeSet<String> = CASES.iter().map(|c| c.file.to_string()).collect();

    let unlabelled: Vec<&String> = on_disk.difference(&labelled).collect();
    assert!(
        unlabelled.is_empty(),
        "these fixtures are graded by nothing — add them to `CASES`: {unlabelled:?}"
    );
    let missing: Vec<&String> = labelled.difference(&on_disk).collect();
    assert!(
        missing.is_empty(),
        "`CASES` names fixtures that no longer exist: {missing:?}"
    );
}

fn join_or_dash(codes: &[&str]) -> String {
    if codes.is_empty() {
        "—".to_string()
    } else {
        codes.join(", ")
    }
}
