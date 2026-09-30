use super::*;

/// The fixed job ad every fixture runs against: English against English,
/// so the kernel's alignment decision is the same everywhere here.
const JOB_AD: &str = "We need Go and Kubernetes experience.";

/// Run the pure selector and return the map. Tests read `items` in the
/// order the requirements were passed.
fn build(source_resume: &str, requirements: &[&str]) -> EvidenceMap {
    build_evidence(
        source_resume,
        JOB_AD,
        "en",
        &requirements
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<_>>(),
    )
}

/// 1. Picks the bullet with the most requirement-token hits, and
///    `source_company` is that bullet's ROLE company.
///
/// The best bullet sits in the SECOND role. Mutation check: score a line
/// "any token present" (boolean instead of a count) and the first bullet
/// ties and wins the earlier-line tie-break — so this fails unless the
/// score is a real count.
#[test]
fn picks_the_bullet_with_the_most_hits_and_its_company() {
    let resume = "Jane Doe\n\nWORK EXPERIENCE\n\n\
        Frontend Engineer | Nova Labs | 2021 - Present\n\
        - Shipped a React dashboard for the sales team\n\n\
        Frontend Engineer | Vega Systems | 2019 - 2021\n\
        - Optimized React performance on the checkout flow";
    let map = build(resume, &["React Performance"]);
    let item = &map.items[0];
    assert_eq!(
        item.source_quote, "Optimized React performance on the checkout flow",
        "the line carrying BOTH requirement tokens must win"
    );
    assert_eq!(
        item.source_company, "Vega Systems",
        "the quote's company is the role the winning bullet sits under"
    );
}

/// 2. A tie for the best score goes to the EARLIER bullet.
///
/// Mutation check: `>=` in the best-update (later line wins) and this
/// fails — which is also why `max_by_key` is not used in `build_evidence`.
#[test]
fn tie_goes_to_the_earlier_bullet() {
    let resume = "Jane Doe\n\nWORK EXPERIENCE\n\n\
        Platform Engineer | Acme | 2021 - Present\n\
        - Tuned Postgres for the analytics API and wired Redis caching\n\n\
        Platform Engineer | Beta | 2019 - 2021\n\
        - Operated Postgres and Redis in a multi-region cluster";
    let map = build(resume, &["Postgres Redis"]);
    let item = &map.items[0];
    assert_eq!(
        item.source_quote, "Tuned Postgres for the analytics API and wired Redis caching",
        "the FIRST of two equally-supporting lines must win the tie"
    );
    assert_eq!(item.source_company, "Acme");
}

/// 3. A short-term requirement matches the line that says the WORD, never
///    a line that merely contains its letters inside a longer word.
///
/// Two sub-cases, because the example terms straddle both scoring paths:
///
/// * "Go" TOKENIZES (it is in `SHORT_TECH_TERMS`, so the kernel keep-list
///   admits it) and must match the bullet that says "Go" — not the
///   "Django" bullet, which the token intersection already rejects even
///   though "go" is a substring of "django".
/// * "Gui" (3 bytes, not allowlisted) truly tokenizes to NOTHING and takes
///   the word-bounded fallback. The earlier "Handled distinguish…" bullet
///   CONTAINS "gui" as a substring, so a substring fallback would score it
///   1 and the earlier-line tie-break would pick it — the word-bounded
///   `contains_word` is what rejects it.
///
/// Mutation check: `line.lower.contains(requirement_lower)` in the
/// fallback and the `Gui` assertions fail.
#[test]
fn short_requirement_matches_the_word_not_a_substring() {
    let resume = "Jane Doe\n\nWORK EXPERIENCE\n\n\
        Backend Engineer | Acme | 2021 - Present\n\
        - Wrote a Django service for the billing API\n\
        - Handled distinguish between prod and staging environments\n\
        - Built a Go microservice for the job runner\n\
        - Built a GUI dashboard for the ops team";
    let map = build(resume, &["Go", "Gui"]);

    let go = &map.items[0];
    assert_eq!(
        go.source_quote, "Built a Go microservice for the job runner",
        "the 'Go' line must beat the 'Django' line — 'go' is only a substring of 'django'"
    );
    assert_eq!(go.status, EvidenceStatus::Covered);

    let gui = &map.items[1];
    assert_eq!(
        gui.source_quote, "Built a GUI dashboard for the ops team",
        "under a substring fallback the earlier 'distinguish' line ties and wins — \
         the match must be word-bounded"
    );
    assert_eq!(gui.status, EvidenceStatus::Covered);
}

/// 4. A requirement with no supporting line gets an empty quote, an empty
///    company, strength 0 and status `Missing`.
///
/// Mutation check: seed `best` with the first candidate (or any default
/// line) and this fails.
#[test]
fn unmatched_requirement_has_no_quote_and_no_company() {
    let resume = "Jane Doe\n\nWORK EXPERIENCE\n\n\
        Engineer | Acme | 2021 - Present\n\
        - Wrote the billing API";
    let map = build(resume, &["Go"]);
    let item = &map.items[0];
    assert!(item.source_quote.is_empty(), "no line evidences 'Go'");
    assert!(item.source_company.is_empty());
    assert_eq!(item.strength, 0);
    assert_eq!(item.status, EvidenceStatus::Missing);
}

/// 5. Strength: all tokens + an ASCII digit → 3; all tokens, no digit →
///    2; only some tokens → 1.
///
/// Mutation check: drop the digit gate (all → 3) and the first assertion
/// fails; drop the "all tokens" gate (partial → 2/3) and the last does.
#[test]
fn strength_reflects_coverage_and_a_measured_result() {
    let resume = "Jane Doe\n\nWORK EXPERIENCE\n\n\
        Engineer | Acme | 2021 - Present\n\
        - Optimized React performance on the checkout flow in 2024\n\
        - Built a Go microservice for the job runner\n\
        - Shipped Terraform in production";
    let map = build(resume, &["React Performance", "Go", "Terraform Kubernetes"]);
    let strengths: Vec<u8> = map.items.iter().map(|item| item.strength).collect();
    assert_eq!(
        strengths,
        vec![3, 2, 1],
        "all tokens + digit = 3, all tokens = 2, partial = 1"
    );
}

/// 6. A PROJECT bullet can be the quote, with an EMPTY company — projects
///    have no employer, so an unattributed quote must not invent one.
///
/// Mutation check: build candidates from roles only (skipping projects)
/// and the quote comes back empty.
#[test]
fn a_project_bullet_can_be_the_quote_with_an_empty_company() {
    let resume = "Jane Doe\n\nPROJECTS\n\n- Built a payment webhook relay in Go";
    let map = build(resume, &["Webhook Relay"]);
    let item = &map.items[0];
    assert_eq!(item.source_quote, "Built a payment webhook relay in Go");
    assert!(
        item.source_company.is_empty(),
        "a project quote must carry no company"
    );
}

/// 7. Every non-empty `source_quote` is the `text` of a bullet
///    `extract_evidence` produced — the honesty spine the old model path
///    had to ENFORCE afterwards is the shape of the data now: nothing is
///    synthesized, an unmatched requirement just stays empty.
///
/// Mutation check: build the quote from the requirement text (or any
/// non-bullet string) and this fails.
#[test]
fn every_quote_is_a_bullet_text_from_extract_evidence() {
    let resume = "Jane Doe\n\nWORK EXPERIENCE\n\n\
        Engineer | Acme | 2021 - Present\n\
        - Optimized React performance on the checkout flow in 2024\n\n\
        PROJECTS\n\n\
        - Built a payment webhook relay in Go";
    let requirements = [
        "React Performance",
        "Webhook Relay",
        "Not Mentioned Anywhere",
    ];
    let map = build(resume, &requirements);

    let extracted = extract_evidence(resume, JOB_AD);
    let bullet_texts: HashSet<&str> = extracted
        .roles
        .iter()
        .flat_map(|role| role.bullets.iter().map(|bullet| bullet.text.as_str()))
        .chain(extracted.projects.iter().map(|bullet| bullet.text.as_str()))
        .collect();

    for item in &map.items {
        if !item.source_quote.is_empty() {
            assert!(
                bullet_texts.contains(item.source_quote.as_str()),
                "quote {:?} is not the text of any extracted bullet — it must have been \
                 synthesized",
                item.source_quote
            );
        }
    }
    assert_eq!(
        map.items[2].source_quote, "",
        "the unmatched requirement stays empty"
    );
}

/// The stage makes NO provider call — what the boundary deadline check and
/// the paying/free vocabulary rely on.
#[test]
fn match_evidence_makes_no_provider_call() {
    assert!(!MatchEvidence.costs_a_provider_call());
}
