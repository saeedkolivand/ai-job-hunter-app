use super::super::prompts::{company_roster_block, draft_user, strategy_user};
use super::super::stages::{reseed, seed_company_roster, MAX_COMPANY_PLANS};
use super::super::types::{
    CompanyPlan, EvidenceItem, EvidenceMap, EvidenceStatus, JobAnalysis, ResumeStrategy, SkillGroup,
};
use super::support::THREE_ROLE_RESUME;

/// **The core-rule assertion for this artifact.** `JobAnalysis` is presentation
/// metadata: a model-derived requirement list feeding the match PERCENTAGE the
/// user reads as objective would make the score a statement about what a model
/// guessed, not about what the posting says.
///
/// Enforced structurally rather than by review: the match kernel
/// (`documents::keywords`) and `score_one` take TEXT, and this type is not a
/// text. The test asserts the absence mechanically — no source file under the
/// match-scoring path may name it. Mutation check: add
/// `use crate::pipeline::resume::types::JobAnalysis;` to
/// `commands/match_resume.rs` and this fails.
#[test]
fn job_analysis_never_reaches_match_scoring() {
    let scoring_sources = [
        include_str!("../../../commands/match_resume.rs"),
        include_str!("../../../documents/keywords.rs"),
        include_str!("../../../documents/keywords/language.rs"),
        include_str!("../../../documents/keywords/lexicon.rs"),
        include_str!("../../../documents/keywords/posting.rs"),
        include_str!("../../../documents/keywords/stopwords_germanic.rs"),
        include_str!("../../../documents/keywords/stopwords_romance.rs"),
        include_str!("../../../commands/autopilot/rerank.rs"),
    ];
    for source in scoring_sources {
        assert!(
            !source.contains("JobAnalysis"),
            "match scoring must not read the model-derived JobAnalysis — the score has to \
             stay a statement about the posting's own text"
        );
    }
}

/// An evidence map where every named requirement is supported — so the
/// emphasis filter below is not what a test is accidentally measuring.
fn evidence_covering(requirements: &[&str]) -> EvidenceMap {
    EvidenceMap {
        items: requirements
            .iter()
            .map(|requirement| EvidenceItem {
                requirement: (*requirement).to_string(),
                status: EvidenceStatus::Covered,
                ..EvidenceItem::default()
            })
            .collect(),
    }
}

/// **The structural guarantee.** Whatever the model returns — a shorter list, a
/// renamed employer, re-dated entries — the plan comes back with exactly the
/// roster's roles, in the roster's order, with the roster's identity.
///
/// Mutation check: return `model.per_company` unchanged from `reseed` and every
/// assertion here fails.
#[test]
fn strategy_never_drops_renames_or_re_dates_a_role() {
    let roster = seed_company_roster(THREE_ROLE_RESUME, "We need a payments engineer.");
    assert_eq!(roster.len(), 3, "fixture must seed three roles");

    let model = ResumeStrategy {
        per_company: vec![CompanyPlan {
            // The employer named exactly (case-insensitively), which is the
            // ONLY way a plan is attached — see the positional-fallback test.
            company: "acme payments".to_string(),
            title: "Principal Engineer".to_string(),
            dates: "2015 - Present".to_string(),
            angle: "lead with the ledger".to_string(),
            emphasis: vec!["payments".to_string()],
            condensed: false,
        }],
        ..ResumeStrategy::default()
    };

    let (out, dropped) = reseed(&roster, &model, &evidence_covering(&["payments"]));
    assert_eq!(dropped, 0, "nothing was filtered out here");
    assert_eq!(out.len(), roster.len(), "no role may be dropped");
    for (planned, seed) in out.iter().zip(roster.iter()) {
        assert_eq!(planned.company, seed.company);
        assert_eq!(planned.title, seed.title);
        assert_eq!(planned.dates, seed.dates);
    }
    // …and the two fields the model IS allowed to author survive.
    assert_eq!(out[0].angle, "lead with the ledger");
    assert_eq!(out[0].emphasis, vec!["payments".to_string()]);
    assert!(
        out[1].angle.is_empty(),
        "an unplanned role gets no invented angle"
    );
}

/// **A plan is matched by NAME, never by position.**
///
/// The positional fallback (`model.per_company.get(index)`) was written for the
/// tolerant case — a model that reworded an employer — but it cannot tell that
/// case from the dangerous one: a model that drops, merges or REORDERS entries
/// gets its plan for role B attached to role A, and nothing downstream can see
/// that the angle describes a different job. Re-seeding exists precisely
/// because the model's list is not trusted to be parallel to the roster.
///
/// Mutation check: restore `.or_else(|| model.per_company.get(index))` and both
/// assertions here fail.
#[test]
fn strategy_never_attaches_a_plan_to_a_role_by_position() {
    let roster = seed_company_roster(THREE_ROLE_RESUME, "We need a payments engineer.");
    let model = ResumeStrategy {
        per_company: vec![
            // A renamed employer: matches nothing on the roster.
            CompanyPlan {
                company: "ACME PAYMENTS INTERNATIONAL".to_string(),
                angle: "lead with the ledger".to_string(),
                ..CompanyPlan::default()
            },
            // A plan for the THIRD role, sitting in the SECOND slot.
            CompanyPlan {
                company: "Gamma Industries".to_string(),
                angle: "show the testing depth".to_string(),
                ..CompanyPlan::default()
            },
        ],
        ..ResumeStrategy::default()
    };

    let (out, _dropped) = reseed(&roster, &model, &EvidenceMap::default());
    assert!(
        out[0].angle.is_empty(),
        "a renamed employer matches nothing and must get no angle, not the first plan"
    );
    assert!(
        out[1].angle.is_empty(),
        "role 2 must not inherit the plan that happens to sit at index 1"
    );
    assert_eq!(
        out[2].angle, "show the testing depth",
        "the plan that NAMED its employer lands on that employer"
    );
}

/// **A requirement the résumé cannot evidence is never emphasized.**
///
/// The prompt says so, and a prompt is not a guarantee. An emphasis is an
/// instruction to the DRAFT stage, so a `missing` requirement surviving here
/// tells the next stage to write a claim the source does not support —
/// arriving one stage before the validator can call it a Critical.
///
/// Mutation check: return `p.emphasis` unfiltered and both dropped terms
/// survive.
#[test]
fn strategy_emphasis_keeps_only_what_the_evidence_map_supports() {
    let roster = seed_company_roster(THREE_ROLE_RESUME, "We need a payments engineer.");
    let model = ResumeStrategy {
        per_company: vec![CompanyPlan {
            company: "Acme Payments".to_string(),
            angle: "lead with the ledger".to_string(),
            emphasis: vec![
                "Payments".to_string(),              // covered — kept (case-insensitively)
                "Ledgers".to_string(),               // partial — kept
                "Kubernetes".to_string(),            // MISSING — dropped
                "Executive sponsorship".to_string(), // not in the map at all — dropped
            ],
            ..CompanyPlan::default()
        }],
        ..ResumeStrategy::default()
    };
    let evidence = EvidenceMap {
        items: vec![
            EvidenceItem {
                requirement: "payments".to_string(),
                status: EvidenceStatus::Covered,
                ..EvidenceItem::default()
            },
            EvidenceItem {
                requirement: "Ledgers".to_string(),
                status: EvidenceStatus::Partial,
                ..EvidenceItem::default()
            },
            EvidenceItem {
                requirement: "Kubernetes".to_string(),
                status: EvidenceStatus::Missing,
                ..EvidenceItem::default()
            },
        ],
    };

    let (out, dropped) = reseed(&roster, &model, &evidence);
    assert_eq!(
        out[0].emphasis,
        vec!["Payments".to_string(), "Ledgers".to_string()],
        "only the requirements the résumé can vouch for survive"
    );
    // The drop is COUNTED. Without this the artifact of a run whose evidence
    // map spells a requirement differently from the strategy is
    // indistinguishable from one where the model emphasized nothing — the
    // filter matches requirement TEXT, so `"k8s"` against `"Kubernetes"` goes
    // the same silent way these two did.
    //
    // Mutation check: return a constant 0 from `reseed` and this fails.
    assert_eq!(
        dropped, 2,
        "both ungrounded terms are counted, not just removed"
    );
}

/// Past the per-company cap, the remaining roles are CONDENSED into one entry —
/// never dropped. Mutation check: replace the condensed branch with a plain
/// `take(MAX_COMPANY_PLANS)` and the "every employer accounted for" assertion
/// fails.
#[test]
fn strategy_condenses_rather_than_drops_past_the_company_cap() {
    let mut resume = String::from("Jane Doe\n\nWORK EXPERIENCE\n");
    for index in 0..MAX_COMPANY_PLANS + 3 {
        resume.push_str(&format!(
            "\nEngineer | Company{index} | 20{:02} - 20{:02}\n- Did work\n",
            10 + index,
            11 + index
        ));
    }
    let roster = seed_company_roster(&resume, "engineer");
    assert_eq!(
        roster.len(),
        MAX_COMPANY_PLANS + 1,
        "cap plus one condensed group"
    );
    let condensed = roster.last().expect("condensed group");
    assert!(condensed.condensed);
    for index in MAX_COMPANY_PLANS..MAX_COMPANY_PLANS + 3 {
        assert!(
            condensed.title.contains(&format!("Company{index}")),
            "every employer past the cap must still be named; got {:?}",
            condensed.title
        );
    }

    // The group's DATES must span the whole group, oldest start → newest end.
    // The fixture's roles run 2010-2011 … 2020-2021 in document order, so the
    // three past the cap are 2018-2019, 2019-2020, 2020-2021 and the condensed
    // entry stands for 2018 → 2021. Mutation check: go back to
    // `rest.last().dates` and this reads "2020 - 2021", understating the
    // history by two roles at the one place the draft prompt renders it.
    assert_eq!(
        condensed.dates, "2018 \u{2013} 2021",
        "the condensed group must span oldest start to newest end; got {:?}",
        condensed.dates
    );
}

/// The seeded roster reaches the model as DATA, with its identity fields
/// intact — that is what makes "the roster is fixed" a statement the model can
/// act on rather than a rule only Rust knows.
#[test]
fn the_company_roster_block_carries_the_seeded_identity() {
    let roster = seed_company_roster(THREE_ROLE_RESUME, "payments engineer");
    let block = company_roster_block(&roster);
    assert!(block.starts_with("<company_roster>"));
    assert!(block.contains("Acme Payments"));
    assert!(block.contains("condensed=false"));
    // The strategy turn composes it alongside three other blocks.
    let user = strategy_user(
        THREE_ROLE_RESUME,
        &JobAnalysis::default(),
        &EvidenceMap::default(),
    );
    assert!(user.contains("<evidence_map>"));
}

/// **A max-roster strategy reaches the draft turn WHOLE.**
///
/// `fenced` truncates at its cap with NO marker, and the strategy artifact is
/// the ONE place the seeded roster reaches the document: a cut mid-`perCompany`
/// silently undoes "never drop a role" — and the `factual.dropped_role`
/// Critical it produces downstream is unrepairable, because an absence has no
/// section to regenerate. The old 4 000-char cap was below a full roster's
/// pretty-printed size, so this was reachable with eight ordinary jobs.
///
/// Mutation check: restore `ARTIFACT_CAP = 4_000` and both the last company and
/// the JSON parse fail. (Restoring `to_string_pretty` does NOT fail this at the
/// current cap — 7 553 chars still fits. Compactness is documented as MARGIN,
/// not as the guard, and the size assertion below is what notices it.)
#[test]
fn the_strategy_artifact_survives_a_max_roster_uncapped() {
    let angle = "Lead with the ledger migration: this role is the one that proves end-to-end \
                 ownership of a payments platform, from schema design through the on-call \
                 rotation, at the scale this posting names.";
    let per_company: Vec<CompanyPlan> = (0..=MAX_COMPANY_PLANS)
        .map(|index| CompanyPlan {
            company: format!("Company Number {index} Payments Systems International GmbH"),
            title: "Senior Staff Software Engineer, Platform".to_string(),
            dates: "January 2019 \u{2013} March 2021".to_string(),
            angle: angle.to_string(),
            emphasis: vec![
                "distributed systems".to_string(),
                "payments domain".to_string(),
                "Kubernetes".to_string(),
                "incident response".to_string(),
                "team leadership".to_string(),
            ],
            condensed: index == MAX_COMPANY_PLANS,
        })
        .collect();
    let strategy = ResumeStrategy {
        headline_angle: angle.to_string(),
        summary_focus: (0..6).map(|i| format!("focus area number {i}")).collect(),
        per_company,
        skills_groups: (0..6)
            .map(|group| SkillGroup {
                label: format!("Skill group number {group}"),
                skills: (0..8).map(|s| format!("Technology {group}-{s}")).collect(),
            })
            .collect(),
    };

    let out = draft_user(
        "Jane Doe\nEXPERIENCE\n- Built things",
        "We need it all.",
        &strategy,
        &[],
    );
    let last_company = format!("Company Number {MAX_COMPANY_PLANS} Payments Systems");
    assert!(
        out.contains(&last_company),
        "the LAST roster entry must survive the cap — a silent cut here is a dropped employer"
    );

    // Not merely "the name is in there": the whole block must still be valid
    // JSON, which is what a mid-object truncation destroys.
    let body = out
        .split_once("<resume_strategy>\n")
        .and_then(|(_, rest)| rest.split_once("\n</resume_strategy>"))
        .map(|(body, _)| body)
        .expect("the strategy block is fenced");
    let round_tripped: ResumeStrategy =
        serde_json::from_str(body).expect("the fenced artifact must still be parseable JSON");
    assert_eq!(
        round_tripped.per_company.len(),
        MAX_COMPANY_PLANS + 1,
        "every roster entry, including the condensed group"
    );
    assert!(
        round_tripped
            .per_company
            .last()
            .is_some_and(|p| p.condensed),
        "the condensed group must still be last"
    );
    // The MEASURED size the cap's documented margin is derived from. A tripwire,
    // not a style rule: an artifact that grows past this has eaten the margin
    // and the cap has to be re-argued (or the artifact trimmed) rather than
    // silently approaching a truncation nobody marks.
    let measured = body.chars().count();
    assert!(
        measured <= 7_000,
        "the max-roster strategy measured {measured} chars — the cap's margin was derived \
         from 5 845; re-derive ARTIFACT_CAP before letting this grow"
    );
}

/// The OTHER artifact that rides `ARTIFACT_CAP`, and the one that actually
/// approaches it: a full 40-requirement evidence map, each item carrying a
/// verbatim résumé line. Measured for the same reason — the cap is sized for
/// this one, so a change here is what eats the margin first.
///
/// Mutation check: restore `ARTIFACT_CAP = 12_000` and the round-trip fails.
#[test]
fn the_evidence_artifact_survives_a_full_requirement_set() {
    let evidence = EvidenceMap {
        items: (0..40)
            .map(|index| EvidenceItem {
                requirement: format!("Requirement number {index} with a long noun phrase"),
                source_quote: format!(
                    "- Delivered the thing number {index} across a long verbatim résumé line \
                     that a model copied character for character from the source document"
                ),
                source_company: "Company Number 1 Payments Systems International GmbH".to_string(),
                status: EvidenceStatus::Covered,
                strength: 3,
            })
            .collect(),
    };

    let out = strategy_user(
        "Jane Doe\nEXPERIENCE\n- Built things",
        &JobAnalysis::default(),
        &evidence,
    );
    let body = out
        .split_once("<evidence_map>\n")
        .and_then(|(_, rest)| rest.split_once("\n</evidence_map>"))
        .map(|(body, _)| body)
        .expect("the evidence block is fenced");
    let round_tripped: EvidenceMap =
        serde_json::from_str(body).expect("the fenced artifact must still be parseable JSON");
    assert_eq!(round_tripped.items.len(), 40, "no requirement may be cut");
    let measured = body.chars().count();
    assert!(
        measured <= 14_500,
        "the full evidence map measured {measured} chars — the cap's margin was derived from \
         13 191; re-derive ARTIFACT_CAP before letting this grow"
    );
}
