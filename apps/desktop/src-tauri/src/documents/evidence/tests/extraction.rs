//! `extract_evidence`: the source résumé structured into roles, projects and education, and what each
//! kind of section does and does not contribute.

use super::*;

// ── extract_evidence ────────────────────────────────────────────────────

#[test]
fn evidence_groups_bullets_under_their_role() {
    let set = extract_evidence(STRUCTURED, "Docker Kubernetes Rust backend engineer");
    assert_eq!(set.roles.len(), 2, "two entries; got {:?}", set.roles);
    assert_eq!(set.roles[0].company, "Acme Corp");
    assert_eq!(set.roles[0].title, "Senior Engineer");
    assert_eq!(set.roles[0].dates, "2021 - Present");
    assert_eq!(set.roles[0].bullets.len(), 2);
    assert_eq!(set.roles[1].company, "Globex");
    assert_eq!(set.roles[1].bullets.len(), 1);
    // Ids are role-scoped and stable.
    assert_eq!(set.roles[1].bullets[0].id, "r1b0");
}

#[test]
fn evidence_separates_projects_and_education_from_experience() {
    let set = extract_evidence(STRUCTURED, "Docker Kubernetes Rust backend engineer");
    assert_eq!(set.projects.len(), 1, "got {:?}", set.projects);
    assert!(set.projects[0].text.contains("Ledger CLI"));
    assert_eq!(set.projects[0].id, "p0");
    assert!(
        set.education.iter().any(|e| e.contains("TU Berlin")),
        "education content must land in `education`; got {:?}",
        set.education
    );
    assert!(
        !set.roles
            .iter()
            .any(|r| r.bullets.iter().any(|b| b.text.contains("Ledger"))),
        "a projects bullet must never attach to an experience role"
    );
}

/// A posting with no extractable keywords still yields the résumé's
/// STRUCTURE (roles, projects, education) — only the scoring goes quiet.
/// `rank_bullets` returns nothing there; `extract_evidence` must not, or a
/// generation prompt would lose the candidate's evidence entirely.
#[test]
fn keywordless_posting_still_yields_structure() {
    let set = extract_evidence(STRUCTURED, "!!! ??? ...");
    assert_eq!(
        set.roles.len(),
        2,
        "structure survives a keywordless posting"
    );
    assert!(set.skills_present.is_empty());
    assert!(set.skills_absent.is_empty());
    assert!(
        set.roles[0].bullets.iter().all(|b| b.score == 0.0),
        "with no posting vocabulary every bullet scores zero"
    );
}

/// A degree line with a date span on it is the NORMAL shape, and
/// `export::parser` classifies it `Contact` because `PHONE_RE` matches
/// "2014 - 2018". Before that arm existed, the only education entries that
/// reached the evidence set were the ones with no dates — so a prompt built
/// from this set was told the candidate had an undated degree, or none.
#[test]
fn education_entries_with_date_spans_are_kept() {
    let resume = "EXPERIENCE\n\n\
                  Senior Engineer | Acme | 2021 - 2024\n\
                  - Shipped the ledger service\n\n\
                  EDUCATION\n\n\
                  BSc Computer Science, TU Berlin, 2014 - 2018\n\
                  MSc Distributed Systems, TU Berlin, 2018 - 2020\n";
    let set = extract_evidence(resume, "backend engineer computer science");
    assert_eq!(
        set.education.len(),
        2,
        "both degrees carry a date span and both must survive; got {:?}",
        set.education
    );
    assert!(set.education.iter().any(|e| e.contains("BSc")));
    assert!(set.education.iter().any(|e| e.contains("MSc")));
    assert!(
        !set.roles
            .iter()
            .any(|r| r.bullets.iter().any(|b| b.text.contains("BSc"))),
        "an education line must never attach to an experience role"
    );
}

/// A projects entry whose links live on a non-bulleted title line is the
/// owner-locked format's NORMAL shape, and `export::parser` classifies it
/// `Contact` (a `github.com` URL, or two `·` separators, is all it takes).
/// The Education arm above already had `LineKind::Contact` added for exactly
/// this reason; the Projects arm did not, so the line naming the project and
/// its repository was silently dropped from the evidence set — a generation
/// prompt was told the candidate's project had no link and no stack.
#[test]
fn project_lines_carrying_links_are_kept_as_evidence() {
    let resume = "EXPERIENCE\n\n\
                  Senior Engineer | Acme | 2021 - 2024\n\
                  - Shipped the ledger service\n\n\
                  PROJECTS\n\n\
                  **Ledger CLI** · https://ledger.example.dev · github.com/janedoe/ledger\n\
                  Rust · SQLite · Clap\n\
                  A double-entry bookkeeping tool for freelancers.\n";
    let set = extract_evidence(resume, "rust backend engineer bookkeeping ledger");
    assert!(
        set.projects
            .iter()
            .any(|p| p.text.contains("github.com/janedoe/ledger")),
        "the project's title + link line must reach the evidence set; got {:?}",
        set.projects
    );
    assert!(
        set.projects.iter().any(|p| p.text.contains("SQLite")),
        "the `·`-separated stack line is also Contact-shaped and must survive; got {:?}",
        set.projects
    );
    // Ids stay dense and document-ordered across the newly-kept lines.
    let ids: Vec<&str> = set.projects.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["p0", "p1", "p2"], "got {ids:?}");
    assert!(
        !set.roles
            .iter()
            .any(|r| r.bullets.iter().any(|b| b.text.contains("Ledger CLI"))),
        "a projects line must never attach to an experience role"
    );
}

/// The same defect end to end: the contact block under a "PERSONAL
/// INFORMATION" heading was handed to the prompt as the candidate's
/// education.
#[test]
fn a_personal_information_block_is_never_extracted_as_education() {
    let resume = "Jane Doe\n\n\
                  EXPERIENCE\n\n\
                  Senior Engineer | Acme | 2021 - 2024\n\
                  - Shipped the ledger service\n\n\
                  PERSONAL INFORMATION\n\n\
                  jane.doe@example.com | +49 30 1234567\n";
    let set = extract_evidence(resume, "backend engineer ledger");
    assert!(
        set.education.is_empty(),
        "a contact block is not a degree; got {:?}",
        set.education
    );
}

/// The same defect end to end: the section's entries and bullets must reach the
/// evidence set.
#[test]
fn a_werdegang_section_yields_roles_and_bullets() {
    let resume = "\
Jana Mustermann

BERUFLICHER WERDEGANG

Senior Backend Engineer | Acme Payments | 2021 - Heute
- Docker-Container auf einem Kubernetes-Cluster betrieben
";
    let set = extract_evidence(resume, "Docker Kubernetes backend");
    assert_eq!(set.roles.len(), 1, "got {:?}", set.roles);
    assert_eq!(set.roles[0].company, "Acme Payments");
    assert!(
        set.roles[0]
            .bullets
            .iter()
            .any(|b| b.text.contains("Docker")),
        "the section's bullets must survive; got {:?}",
        set.roles[0].bullets
    );
}

/// R6-F6, second half — a heading no list recognises still classifies as
/// `Other`, and its bullets were discarded outright. When the document has NO
/// recognised experience section, that silently throws away the candidate's
/// only evidence.
#[test]
fn an_unclassified_section_with_bullets_still_contributes_evidence() {
    let resume = "\
Jane Doe

MEINE HIGHLIGHTS

- Shipped Docker containers onto a Kubernetes cluster
- Cut checkout latency with a Redis cache in front of the ledger service
";
    let set = extract_evidence(resume, "Docker Kubernetes Redis backend engineer");
    let texts: Vec<&str> = set
        .roles
        .iter()
        .flat_map(|r| r.bullets.iter())
        .map(|b| b.text.as_str())
        .collect();
    assert!(
        texts.iter().any(|t| t.contains("Docker")),
        "an unclassified section's bullets are still the candidate's own evidence; got {texts:?}"
    );
    // Never a guessed employer — the bucket stays unattributed.
    assert!(
        set.roles.iter().all(|r| r.company.is_empty()),
        "no company may be invented for an unclassified section; got {:?}",
        set.roles
    );
}

/// A CLASSIFIED section's bullets keep going where they belong — the last-resort
/// fallback must not become a second home for skills or summary lines.
#[test]
fn the_unclassified_fallback_yields_to_a_real_experience_section() {
    let resume = "\
Jane Doe

EXPERIENCE

Senior Engineer | Acme Payments | 2021 - Present
- Shipped Docker containers onto a Kubernetes cluster

INTERESTS

- Runs the local bouldering meetup every second Thursday
";
    let set = extract_evidence(resume, "Docker Kubernetes backend engineer");
    assert_eq!(set.roles.len(), 1, "got {:?}", set.roles);
    assert!(
        !set.roles[0]
            .bullets
            .iter()
            .any(|b| b.text.contains("bouldering")),
        "a hobby must not become work evidence when a real experience section \
         exists; got {:?}",
        set.roles[0].bullets
    );
}

/// The same defect end to end: summary prose filed as a work bullet under a
/// role the résumé never had.
#[test]
fn career_summary_prose_is_not_filed_as_work_evidence() {
    let resume = "\
Jane Doe

CAREER SUMMARY

Backend engineer with eight years on payment and container platforms.

EXPERIENCE

Senior Engineer | Acme Payments | 2021 - Present
- Shipped Docker containers onto a Kubernetes cluster
";
    let set = extract_evidence(resume, "Docker Kubernetes backend engineer");
    assert_eq!(
        set.roles.len(),
        1,
        "the summary must not open a role of its own; got {:?}",
        set.roles
    );
    assert_eq!(set.roles[0].company, "Acme Payments");
    let texts: Vec<&str> = set.roles[0]
        .bullets
        .iter()
        .map(|b| b.text.as_str())
        .collect();
    assert_eq!(
        texts,
        vec!["Shipped Docker containers onto a Kubernetes cluster"],
        "a summary sentence is a claim, not an achievement to draw on"
    );
}

/// The same defect end to end: the skills matrix became work evidence.
#[test]
fn skill_lines_under_a_combined_heading_are_not_work_bullets() {
    let resume = "\
Jane Doe

SKILLS AND EXPERIENCE

- Rust, Python and Go, eight years
- Docker and Kubernetes in production

EXPERIENCE

Senior Engineer | Acme Payments | 2021 - Present
- Shipped the ledger service onto a Kubernetes cluster
";
    let set = extract_evidence(resume, "Docker Kubernetes Rust backend engineer");
    assert_eq!(
        set.roles.len(),
        1,
        "a skills matrix must not open a role of its own; got {:?}",
        set.roles
    );
    assert_eq!(set.roles[0].company, "Acme Payments");
    let texts: Vec<&str> = set.roles[0]
        .bullets
        .iter()
        .map(|b| b.text.as_str())
        .collect();
    assert_eq!(
        texts,
        vec!["Shipped the ledger service onto a Kubernetes cluster"],
        "a skill line is a claim, not an achievement to draw on"
    );
}

/// The counter-case, and the reason the fix is scoped to the AMBIGUOUS stems:
/// resolving every combined heading to Skills would not merely "unpolice" a
/// work history, it would DELETE it. Nothing in [`extract_evidence`] reads a
/// Skills section — no role arm, no bullet arm, and the last-resort rescue only
/// covers `Other` — so a résumé whose one work-history heading happens to carry
/// a skills word would hand the generation prompt an empty evidence set.
#[test]
fn a_work_history_heading_keeps_its_roles_beside_a_skills_word() {
    let resume = "\
Jana Mustermann

BERUFSERFAHRUNG UND KENNTNISSE

Senior Backend Engineer | Acme Payments | 2021 - Heute
- Docker-Container auf einem Kubernetes-Cluster betrieben
";
    let set = extract_evidence(resume, "Docker Kubernetes backend");
    assert_eq!(
        set.roles.len(),
        1,
        "the work history must survive a skills word on its heading; got {:?}",
        set.roles
    );
    assert_eq!(set.roles[0].company, "Acme Payments");
    assert!(
        set.roles[0]
            .bullets
            .iter()
            .any(|b| b.text.contains("Docker")),
        "the section's bullets must survive; got {:?}",
        set.roles[0].bullets
    );
}
