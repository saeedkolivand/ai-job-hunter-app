//! Alignment with the posting: coverage regressions, top requirements, and the
//! unusable-posting edge paths.

use super::{support::*, *};

/// Zero keyword overlap between résumé and posting is a real situation (a
/// career change), not a defect. Nothing may fire from it, and coverage must
/// report the honest 0 rather than going silent.
#[test]
fn zero_keyword_overlap_reports_zero_coverage_without_inventing_issues() {
    let baker = "EXPERIENCE\n\nBaker | Corner Bakery | 2019 - 2021\n- Shaped sourdough loaves\n";
    let report = report_for(
        baker,
        baker,
        "Hiring a welder for structural steel fabrication.",
        &[],
    );
    assert_eq!(report.metrics.keyword_coverage, Some(0.0));
    assert!(
        !codes(&report).contains(&ALIGNMENT_LOW_COVERAGE),
        "matching the source exactly can never be a coverage REGRESSION; got {:?}",
        codes(&report)
    );
    assert!(report.ok, "no criticals from a career change");
}

/// An empty or garbled posting must silence every posting comparison rather
/// than reporting 0% and a pile of derived warnings.
#[test]
fn empty_or_garbled_job_ad_silences_posting_comparisons() {
    for job_ad in ["", "   ", "!!! ??? ...", "\u{fffd}\u{fffd}\u{fffd}"] {
        let report = report_for(EN_CLEAN, EN_SOURCE, job_ad, &en_requirements());
        assert_eq!(
            report.metrics.keyword_coverage, None,
            "no extractable posting keywords must yield None, not 0% (job_ad={job_ad:?})"
        );
        assert_eq!(
            report.metrics.top_requirement_hits, None,
            "an uncomparable posting measures nothing — None, never a confident 0"
        );
        for code in [ALIGNMENT_LOW_COVERAGE, ALIGNMENT_MISSING_TOP_REQUIREMENT] {
            assert!(
                !codes(&report).contains(&code),
                "{code} must not fire against an unusable posting (job_ad={job_ad:?})"
            );
        }
    }
}

/// M6 — coverage moves in whole keyword steps, so any drop at all fired on every
/// legitimate edit. Only a drop of at least
/// [`alignment::MIN_COVERAGE_DROP_POINTS`] percentage points is reported.
#[test]
fn low_coverage_tolerates_a_drop_smaller_than_the_threshold() {
    // 25 posting keywords → each one is worth exactly 4 percentage points.
    let words: Vec<String> = (0..25).map(|i| format!("skillword{i}")).collect();
    let job = words.join(" ");
    let resume = |kept: usize| {
        format!(
            "EXPERIENCE\n\nAcme | 2021 - Present\n- Delivered {}\n",
            words[..kept].join(" ")
        )
    };
    let source = resume(25);

    // One keyword lost = 4 points, under the threshold.
    silent(
        &report_for(&resume(24), &source, &job, &[]),
        ALIGNMENT_LOW_COVERAGE,
    );
    // Two = 8 points, over it.
    fired(
        &report_for(&resume(23), &source, &job, &[]),
        ALIGNMENT_LOW_COVERAGE,
    );
}

/// Coverage is a REGRESSION check: dropping the posting's vocabulary during
/// tailoring fires; keeping it does not, however low the absolute number is.
#[test]
fn low_coverage_fires_on_a_regression_not_on_a_low_absolute_score() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                  - Shipped Docker containers onto a Kubernetes cluster\n\
                  - Wrote the Terraform modules for the AWS estate\n";
    let job = "Docker Kubernetes Terraform AWS platform engineer";
    // Tailoring dropped both technical bullets.
    let stripped = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Organised the team offsite for forty people\n";
    let hits = {
        let report = report_for(stripped, source, job, &[]);
        fired(&report, ALIGNMENT_LOW_COVERAGE).len()
    };
    assert_eq!(hits, 1);
    // The untouched résumé covers the same posting equally — no regression.
    silent(
        &report_for(source, source, job, &[]),
        ALIGNMENT_LOW_COVERAGE,
    );
}

/// A top requirement the candidate cannot meet is not a document defect. Only
/// one the SOURCE evidenced and the output dropped is reported.
#[test]
fn missing_top_requirement_fires_only_when_the_source_had_the_evidence() {
    let source =
        "EXPERIENCE\n\nAcme | 2021 - Present\n- Wrote Terraform modules for the AWS estate\n";
    let job = "Terraform AWS Kubernetes platform engineer";
    let dropped = "EXPERIENCE\n\nAcme | 2021 - Present\n- Kept the AWS estate running\n";
    let evidenced = vec!["Terraform modules".to_string()];
    let hits = fired(
        &report_for(dropped, source, job, &evidenced),
        ALIGNMENT_MISSING_TOP_REQUIREMENT,
    )
    .len();
    assert_eq!(hits, 1);

    // Never evidenced anywhere → the candidate's gap, not the document's.
    let never_had = vec!["Kubernetes operators".to_string()];
    silent(
        &report_for(dropped, source, job, &never_had),
        ALIGNMENT_MISSING_TOP_REQUIREMENT,
    );
    // Still present in the output → counted as a hit, not an issue.
    let report = report_for(source, source, job, &evidenced);
    silent(&report, ALIGNMENT_MISSING_TOP_REQUIREMENT);
    assert_eq!(report.metrics.top_requirement_hits, Some(1));
}

/// R5-F4 — `topRequirementHits` rendered a confident "0" for a posting nobody
/// could compare against and for a run that was given no requirements at all.
/// Same class as the letter-metrics fix: an UNMEASURED value must not be
/// presented as a measurement.
#[test]
fn top_requirement_hits_is_unmeasured_rather_than_zero() {
    let wire = |report: ContentReport| {
        serde_json::to_value(&report).expect("a report must serialize")["metrics"]
            ["topRequirementHits"]
            .clone()
    };

    // No requirements were supplied — nothing was measured.
    assert_eq!(en_resume(EN_CLEAN, &[]).metrics.top_requirement_hits, None);
    assert_eq!(
        report_for(EN_CLEAN, EN_SOURCE, "   ", &en_requirements())
            .metrics
            .top_requirement_hits,
        None
    );
    assert_eq!(
        en_letter(EN_LETTER_GROUNDED).metrics.top_requirement_hits,
        None
    );
    assert!(
        wire(en_resume(EN_CLEAN, &[])).is_null(),
        "no requirements means no measurement"
    );
    // …and the OTHER alignment finding is untouched by the metric's absence:
    // `low_coverage` compares two coverages and never reads the requirements
    // list, so gating it on that list would silence a real regression.
    assert!(
        en_resume(EN_CLEAN, &[]).metrics.keyword_coverage.is_some(),
        "coverage is still measured without a requirements list"
    );
    // A posting with no extractable keywords is not comparable.
    assert!(
        wire(report_for(EN_CLEAN, EN_SOURCE, "   ", &en_requirements())).is_null(),
        "an uncomparable posting means no measurement"
    );
    // A cover letter never runs the alignment pass at all.
    assert!(
        wire(en_letter(EN_LETTER_GROUNDED)).is_null(),
        "a letter never measures top-requirement hits"
    );
    // Genuinely measured → a real count on the wire.
    let measured = wire(en_resume(EN_CLEAN, &en_requirements()));
    assert!(
        measured.as_u64().is_some_and(|n| n > 0),
        "the clean fixture evidences at least one top requirement; got {measured}"
    );
}

/// R9-F2 — `topRequirementHits` is a bare count with no denominator, and a
/// requirement with no extractable keywords is dropped from the loop silently,
/// so "2" could mean 2 of 2, 2 of 10, or 2 out of a list where nothing else was
/// measurable. The panel cannot tell those apart, and neither could this test
/// suite.
#[test]
fn top_requirement_hits_carry_the_denominator_they_were_measured_against() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                  - Wrote Terraform modules for the AWS estate\n\
                  - Ran the PostgreSQL fleet through two major upgrades\n";
    let job = "Terraform AWS PostgreSQL Kubernetes platform engineer";
    let metrics = |reqs: &[String]| {
        serde_json::to_value(report_for(source, source, job, reqs).metrics)
            .expect("metrics must serialize")
    };
    let req = |s: &str| s.to_string();

    let both = metrics(&[req("Terraform modules"), req("PostgreSQL fleet")]);
    assert_eq!(both["topRequirementHits"], 2);
    assert_eq!(
        both["topRequirementsMeasured"], 2,
        "2 of 2 must be distinguishable from 2 of 10"
    );

    // A requirement with no extractable keywords ("5+ yrs" — every token is
    // under the kernel's length filter) is UNMEASURABLE, and an unmeasurable
    // requirement counts for neither side of the ratio.
    let mixed = metrics(&[
        req("Terraform modules"),
        req("PostgreSQL fleet"),
        req("5+ yrs"),
    ]);
    assert_eq!(mixed["topRequirementHits"], 2);
    assert_eq!(
        mixed["topRequirementsMeasured"], 2,
        "an unanswerable requirement is not a denominator"
    );

    // A requirement nothing evidences DOES count in the denominator — that is
    // the whole difference between "2 of 2" and "2 of 3".
    let with_gap = metrics(&[req("Terraform modules"), req("Kubernetes operators")]);
    assert_eq!(with_gap["topRequirementHits"], 1);
    assert_eq!(with_gap["topRequirementsMeasured"], 2);

    // The invariant the wire mirror depends on: the denominator is absent
    // EXACTLY when the hit count is, so the renderer needs one null check for
    // the pair. Both routes to "unmeasured" are covered — an empty requirements
    // list, and a cover letter (which never runs the alignment pass at all).
    for unmeasured in [
        metrics(&[]),
        serde_json::to_value(en_letter(EN_LETTER_GROUNDED).metrics).expect("metrics serialize"),
    ] {
        assert!(unmeasured["topRequirementHits"].is_null());
        assert!(unmeasured["topRequirementsMeasured"].is_null());
    }
}
