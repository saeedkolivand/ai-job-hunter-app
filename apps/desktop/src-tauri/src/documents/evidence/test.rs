// The entry/label/date family moved to `super::entry`; its `pub(super)` helpers
// are not re-exported by `super`, so this glob keeps the direct-unit tests
// (`split_two_space_label`, `is_date_only`, …) resolving unchanged.
use super::entry::*;
use super::*;

const RESUME: &str = "\
EXPERIENCE

Senior Engineer, Acme
- Built and shipped Docker containers onto a Kubernetes cluster
- Organised the team offsite and the summer party for forty people
- Ran the weekly standup
";

/// The whole point: a bullet the posting never mentions must rank BELOW one
/// full of the posting's vocabulary, so the weakest-first list is cuttable
/// from the top.
#[test]
fn irrelevant_bullets_rank_below_keyword_bearing_ones() {
    let job = "We need a backend engineer with strong Docker and Kubernetes experience \
               to own our container platform.";
    let ranked = rank_bullets(RESUME, job);

    assert_eq!(ranked.len(), 3, "all three bullets are candidates");
    assert!(
        ranked[0].text.contains("offsite"),
        "the offsite bullet carries none of the posting's vocabulary and must rank first \
         for cutting; got {:?}",
        ranked[0].text
    );
    let docker = ranked
        .iter()
        .position(|c| c.text.contains("Docker"))
        .expect("the Docker bullet must be present");
    assert_eq!(
        docker, 2,
        "the Docker bullet is the strongest — cut it last"
    );
    assert!(ranked[docker].score > 0.0);
    // Hits are surfaced unstemmed — "kubernetes", never the stem "kubernet".
    assert!(
        ranked[docker].hits.iter().any(|h| h == "kubernetes"),
        "hits must be readable display forms, not Snowball stems; got {:?}",
        ranked[docker].hits
    );
}

/// Ids are assigned in DOCUMENT order, before the weakest-first sort, so
/// `b0` still names the first bullet after ranking reorders the list.
#[test]
fn ids_follow_document_order_not_rank_order() {
    let job = "Docker and Kubernetes platform engineering.";
    let ranked = rank_bullets(RESUME, job);
    let docker = ranked
        .iter()
        .find(|c| c.text.contains("Docker"))
        .expect("the Docker bullet must be present");
    assert_eq!(
        docker.id, "b0",
        "the Docker bullet is the document's FIRST bullet, so its id is b0 \
         even though it ranks last; got {:?}",
        docker.id
    );
    let ids: HashSet<&str> = ranked.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), 3, "ids must be unique; got {ids:?}");
}

/// Ties break on length: two equally worthless bullets, the longer one frees
/// more space, so it is offered first.
#[test]
fn equally_weak_bullets_are_ordered_longest_first() {
    let job = "Docker and Kubernetes platform engineering.";
    let ranked = rank_bullets(RESUME, job);
    let zeroes: Vec<&EvidenceBullet> = ranked.iter().filter(|c| c.score == 0.0).collect();
    assert!(
        zeroes.len() >= 2,
        "expected at least two zero-scoring lines"
    );
    assert!(
        zeroes[0].text.len() >= zeroes[1].text.len(),
        "equal-score lines must be ordered longest-first; got {:?} before {:?}",
        zeroes[0].text,
        zeroes[1].text
    );
}

/// `SHORT_TECH_TERMS` bypass stemming (aws → aw would be corruption). The
/// panel must surface them intact, same as the match score does.
#[test]
fn short_tech_terms_survive_intact() {
    let resume = "EXPERIENCE\n\n- Migrated the fleet to AWS and wrote the Go services\n";
    let job = "Hiring an engineer fluent in AWS and Go.";
    let ranked = rank_bullets(resume, job);
    let hits = &ranked[0].hits;
    assert!(hits.iter().any(|h| h == "aws"), "got {hits:?}");
    assert!(hits.iter().any(|h| h == "go"), "got {hits:?}");
}

/// The invariant the whole panel rests on: it must never disagree with the
/// match score for the same pair. A cross-language pair is where that breaks
/// — `score_one` leaves BOTH sides unstemmed there, so ranking must too.
/// Before this, ranking always stemmed with the JD stemmer and a German
/// posting against an English résumé scored a shared tech token on one side
/// only. Both now route through `languages_align`.
#[test]
fn cross_language_pair_ranks_symmetrically_like_score_one() {
    // German JD, English résumé — divergent, so neither side may be stemmed.
    let job = "Wir suchen einen erfahrenen Entwickler mit Kubernetes und Docker \
               für unsere Container-Plattform in München.";
    let resume = "EXPERIENCE\n\n\
                  - Shipped kubernetes clusters and docker containers to production\n\
                  - Organised the team offsite and the summer party for forty people\n";

    assert!(
        !languages_align(job, detect_locale_tag(resume)),
        "fixture must actually be a divergent pair, or this test proves nothing"
    );

    let ranked = rank_bullets(resume, job);
    let tech = ranked
        .iter()
        .find(|c| c.text.contains("kubernetes"))
        .expect("the tech bullet must be a candidate");
    assert!(
        tech.score > 0.0,
        "shared tech tokens must still match across languages when both sides \
         stay unstemmed; got {:?}",
        tech.hits
    );
    assert!(
        ranked[0].text.contains("offsite"),
        "the JD-irrelevant bullet must still rank first for cutting; got {:?}",
        ranked[0].text
    );
}

/// A posting with nothing extractable yields no ranking rather than a
/// ranking in which everything ties at zero — mirrors `keyword_coverage`
/// returning `None` instead of 0%.
#[test]
fn keywordless_posting_yields_no_suggestions() {
    assert!(rank_bullets(RESUME, "!!! ??? ...").is_empty());
}

/// Only bullets are cuttable. Section headers and the name/contact block are
/// structural — never offer them.
#[test]
fn only_bullets_are_candidates() {
    let job = "Docker and Kubernetes platform engineering.";
    let ranked = rank_bullets(RESUME, job);
    assert!(
        !ranked.iter().any(|c| c.text.contains("EXPERIENCE")),
        "section headers must not be offered for cutting"
    );
    assert!(
        !ranked.iter().any(|c| c.text.contains("Senior Engineer")),
        "job entries must not be offered for cutting"
    );
}

// ── extract_evidence ────────────────────────────────────────────────────

const STRUCTURED: &str = "\
Jane Doe
jane@example.com | +49 30 1234567

EXPERIENCE

Senior Engineer | Acme Corp | 2021 - Present
- Shipped Docker containers onto a Kubernetes cluster
- Ran the weekly standup

Backend Developer | Globex | 2018 - 2021
- Built the billing API in Rust

PROJECTS

- Ledger CLI - a Rust tool for double-entry bookkeeping

EDUCATION

BSc Computer Science, TU Berlin
";

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

/// `skills_present`/`skills_absent` partition the posting's keywords — every
/// keyword lands on exactly one side, which is what makes the gap list
/// honest rather than decorative.
#[test]
fn skills_present_and_absent_partition_the_posting_vocabulary() {
    let set = extract_evidence(STRUCTURED, "Docker Kubernetes Terraform Rust engineer");
    assert!(
        set.skills_present.iter().any(|s| s == "docker"),
        "docker is evidenced by a bullet; got {:?}",
        set.skills_present
    );
    assert!(
        set.skills_absent.iter().any(|s| s == "terraform"),
        "terraform appears nowhere in the résumé; got {:?}",
        set.skills_absent
    );
    let overlap: Vec<&String> = set
        .skills_present
        .iter()
        .filter(|s| set.skills_absent.contains(s))
        .collect();
    assert!(
        overlap.is_empty(),
        "sides must be disjoint; got {overlap:?}"
    );
    // Every term in this posting is stated exactly once, so relevance ties
    // everywhere and the alphabetical TIEBREAK decides the whole list. The
    // ordering contract itself is
    // `skills_lists_lead_with_the_postings_own_priorities` — alphabetical is no
    // longer the rule, only what a tie falls back to.
    assert!(
        set.skills_present.windows(2).all(|w| w[0] <= w[1]),
        "equally-relevant terms are ordered alphabetically; got {:?}",
        set.skills_present
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

/// The kernel's `STOPWORDS` list is English-only, so a German posting filled
/// the honest gap list with "unsere", "hinter" and "sehr". Those are not
/// skills, and a gap list full of them reads as broken. Filtered LOCALLY —
/// `STOPWORDS` feeds the scoring kernel and is formula-version-pinned.
#[test]
fn german_function_words_never_reach_the_skills_split() {
    let job = "Wir suchen eine erfahrene Backend-Entwicklerin für unsere \
               Container-Plattform und die Dienste hinter dem Bezahlvorgang. Du \
               betreibst Kubernetes unter Last und schreibst Rust. Sehr gute \
               Kenntnisse in Docker sind eine Anforderung.";
    let resume = "BERUFSERFAHRUNG\n\n\
                  Senior Backend Engineer | Acme | 2021 - Heute\n\
                  - Docker-Container auf einem Kubernetes-Cluster betrieben\n";
    let set = extract_evidence(resume, job);
    let listed: Vec<&String> = set
        .skills_present
        .iter()
        .chain(set.skills_absent.iter())
        .collect();
    for function_word in [
        "unsere", "hinter", "unter", "sehr", "gute", "eine", "suchen", "für",
    ] {
        assert!(
            !listed.iter().any(|s| s.as_str() == function_word),
            "{function_word:?} is a function word, not a skill; got {listed:?}"
        );
    }
    // The real vocabulary is untouched.
    assert!(
        set.skills_present.iter().any(|s| s == "kubernetes"),
        "got {:?}",
        set.skills_present
    );
    assert!(
        set.skills_absent.iter().any(|s| s == "rust"),
        "got {:?}",
        set.skills_absent
    );
}

/// The filter must stay OUT of the scored bullet path: `hits` (and therefore
/// `score`) drives the trim panel's wire payload and its ranking, and the
/// scoring kernel owns that vocabulary.
#[test]
fn the_function_word_filter_does_not_touch_bullet_scores() {
    let job = "Wir suchen eine erfahrene Entwicklerin für unsere Container-Plattform \
               mit Kubernetes und Docker.";
    let resume = "BERUFSERFAHRUNG\n\n\
                  Senior Engineer | Acme | 2021 - Heute\n\
                  - Unsere Container-Plattform mit Kubernetes betrieben\n";
    let ranked = rank_bullets(resume, job);
    assert!(
        ranked[0].hits.iter().any(|h| h == "unsere"),
        "bullet hits come from the scoring kernel and must be left alone; got {:?}",
        ranked[0].hits
    );
}

#[test]
fn open_ended_spans_are_recognised_in_every_spelling() {
    for open in [
        "2021 - Present",
        "2021 – Heute",
        "seit 2021",
        "since Jan 2021",
        "from 2019",
        "2021 –",
        "2021 -",
    ] {
        assert!(is_open_ended(open), "{open:?} has no end date");
    }
    for closed in [
        "2018 - 2021",
        "Jan 2018 to Mar 2021",
        "presented the roadmap",
        "knowledge sharing",
        "currently",
        "actually shipped it",
        "since the rewrite",
    ] {
        assert!(!is_open_ended(closed), "{closed:?} is not an open span");
    }
}

#[test]
fn contains_word_respects_boundaries() {
    assert!(contains_word("this is vital work", "vital"));
    assert!(!contains_word("we revitalized the pipeline", "vital"));
    assert!(contains_word("not just faster, but cheaper", "not just"));
    assert!(!contains_word("", "vital"));
    assert!(!contains_word("anything", ""));
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

/// `classify_section` matches substrings, and "formation" hides inside
/// "information" (as do `formación`/`formação`/`formazione` inside
/// `información`/`informação`/`informazione`). So the "PERSONAL INFORMATION"
/// heading that heads half the CVs in Europe classified as EDUCATION — and,
/// with the Contact arm above, filed the candidate's phone number and email
/// as a degree.
#[test]
fn personal_information_headings_are_not_education() {
    for heading in [
        "PERSONAL INFORMATION",
        "Personal Information",
        "Información personal",
        "Informação pessoal",
        "Informazioni personali",
    ] {
        assert_ne!(
            classify_section(heading),
            SectionKind::Education,
            "{heading:?} is a contact block, not education"
        );
    }
    // The real headings these stems exist for still classify.
    for heading in [
        "FORMATION",
        "Formation académique",
        "Formations",
        "Formación académica",
        "Formação acadêmica",
        "Formazione",
        "Ausbildung",
        "Weiterbildung",
        "EDUCATION",
    ] {
        assert_eq!(
            classify_section(heading),
            SectionKind::Education,
            "{heading:?} names an education section"
        );
    }
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

#[test]
fn italian_skills_heading_is_classified() {
    assert_eq!(classify_section("COMPETENZE"), SectionKind::Skills);
    assert_eq!(classify_section("Competenze tecniche"), SectionKind::Skills);
}

#[test]
fn years_in_ignores_non_year_digit_runs() {
    assert_eq!(years_in("2021 - Present"), vec![2021]);
    assert_eq!(years_in("Jan 2018 to Mar 2021"), vec![2018, 2021]);
    assert!(
        years_in("processed 4500 orders").is_empty(),
        "a quantity outside 1900–2099 is not a year"
    );
    assert!(
        years_in("+49 30 1234567").is_empty(),
        "a phone number's digit runs are not years"
    );
}

#[test]
fn split_entry_handles_the_two_space_form() {
    let parsed = parse_resume("EXPERIENCE\n\nAcme Corporation    2021 - Present\n");
    let entry = parsed
        .lines
        .iter()
        .find(|l| matches!(l.kind, LineKind::JobEntry))
        .expect("the two-space form must parse as a JobEntry");
    let (company, title, dates) = split_entry(entry);
    assert_eq!(company, "Acme Corporation");
    assert_eq!(title, "");
    assert_eq!(dates, "2021 - Present");
}

/// The two-space form is the extracted-PDF shape, where the comma tail is a
/// LOCATION far more often than a company. Reusing the parenthesized form's
/// title-first "split at the last comma" rule named the CITY as the
/// employer — and `validate::content` then went looking for that city in
/// the generated document and raised a `factual.dropped_role` Critical when
/// a tailored résumé (reasonably) left the location out.
#[test]
fn two_space_form_names_the_company_not_the_city() {
    let split = |line: &str| {
        let text = format!("EXPERIENCE\n\n{line}\n");
        let parsed = parse_resume(&text);
        let entry = parsed
            .lines
            .iter()
            .find(|l| matches!(l.kind, LineKind::JobEntry))
            .unwrap_or_else(|| panic!("{line:?} must parse as a JobEntry"))
            .clone();
        split_entry(&entry)
    };

    // A known city as the tail.
    let (company, title, dates) = split("Acme GmbH, Berlin    2021 - Present");
    assert_eq!(company, "Acme GmbH");
    assert_eq!(title, "");
    assert_eq!(dates, "2021 - Present");

    // An UNLISTED city: the legal form in the head is the evidence.
    let (company, title, _) = split("Nordwind Systeme GmbH, Ingolstadt    2018 - 2021");
    assert_eq!(company, "Nordwind Systeme GmbH");
    assert_eq!(title, "");

    // City plus country.
    let (company, _, _) = split("Globex Logistics, Munich, Germany    2018 - 2021");
    assert_eq!(company, "Globex Logistics");

    // Nothing says otherwise → the title-first reading is untouched.
    let (company, title, _) = split("Senior Engineer, Acme Corp    2021 - Present");
    assert_eq!(company, "Acme Corp");
    assert_eq!(title, "Senior Engineer");
}

/// An experience section whose entry line does not parse as a `JobEntry`
/// (no pipe form, no two-space date column, no parenthesized span) used to
/// lose its ENTIRE experience section: `checked_sub` on an empty `roles`
/// silently discarded every bullet, and the prompt was then told the
/// candidate had no experience at all.
#[test]
fn experience_bullets_survive_an_unparsed_entry_line() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Acme Payments — Senior Backend Engineer
2021 to Present
- Shipped Docker containers onto a Kubernetes cluster
- Cut checkout latency with a Redis cache in front of the ledger service
";
    let set = extract_evidence(resume, "Docker Kubernetes backend engineer");
    let texts: Vec<&str> = set
        .roles
        .iter()
        .flat_map(|r| r.bullets.iter())
        .map(|b| b.text.as_str())
        .collect();
    assert!(
        texts.iter().any(|t| t.contains("Docker")),
        "an orphan bullet under Experience must still be evidence; got {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("Redis")),
        "every orphan bullet lands in the same bucket; got {texts:?}"
    );
    // The bucket is UNATTRIBUTED, never a company the résumé never named.
    assert!(
        set.roles
            .iter()
            .all(|r| r.company.is_empty() || !r.bullets.is_empty()),
        "no empty role may be invented; got {:?}",
        set.roles
    );
}

// ── PR #963 round-5 findings ────────────────────────────────────────────────

/// R5-F6 — the round-4 orphan-bullet fix rescued `LineKind::Text` under
/// Experience but not `LineKind::Contact`, and the exact entry shape it was
/// written for is contact-shaped: `export::parser` reads the date span in
/// "Acme Payments, Berlin, 2018 - 2021" as a phone number. So the employer line
/// was still dropped while its bullets survived in the unattributed bucket —
/// the prompt saw the work and never learned who it was for.
#[test]
fn a_contact_shaped_entry_line_keeps_its_employer_under_experience() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Acme Payments, Berlin, 2018 - 2021
- Shipped Docker containers onto a Kubernetes cluster
- Cut checkout latency with a Redis cache in front of the ledger service
";
    let set = extract_evidence(resume, "Docker Kubernetes backend engineer");
    let texts: Vec<&str> = set
        .roles
        .iter()
        .flat_map(|r| r.bullets.iter())
        .map(|b| b.text.as_str())
        .chain(set.roles.iter().map(|r| r.company.as_str()))
        .collect();
    assert!(
        texts.iter().any(|t| t.contains("Acme Payments")),
        "the employer named on a contact-shaped entry line must survive; got {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("Docker")),
        "its bullets survive too; got {texts:?}"
    );
}

/// R5-F7 — `has_legal_form` tested EVERY token, and `LEGAL_FORMS` carries the
/// ordinary English title words `group`, `company` and `holding`. So the head of
/// "Group Product Manager, Acme Payments" looked like a company name, the
/// company-first rule fired, and the employer was recorded as "Group Product
/// Manager" with the real one discarded. A legal form is a SUFFIX.
#[test]
fn a_title_starting_with_an_ambiguous_legal_word_still_resolves_title_first() {
    let split = |line: &str| {
        let text = format!("EXPERIENCE\n\n{line}\n");
        let parsed = parse_resume(&text);
        let entry = parsed
            .lines
            .iter()
            .find(|l| matches!(l.kind, LineKind::JobEntry))
            .unwrap_or_else(|| panic!("{line:?} must parse as a JobEntry"))
            .clone();
        split_entry(&entry)
    };

    let (company, title, _) = split("Group Product Manager, Acme Payments    2021 - Present");
    assert_eq!(company, "Acme Payments");
    assert_eq!(title, "Group Product Manager");

    // The same for the other two ambiguous English words in the list.
    let (company, title, _) = split("Company Secretary, Globex Logistics    2018 - 2021");
    assert_eq!(company, "Globex Logistics");
    assert_eq!(title, "Company Secretary");

    // …and a genuine trailing legal form still names the company first.
    let (company, title, _) = split("Nordwind Systeme GmbH, Ingolstadt    2018 - 2021");
    assert_eq!(company, "Nordwind Systeme GmbH");
    assert_eq!(title, "");
}

// ── PR #963 round-6 findings ────────────────────────────────────────────────

/// Parse one line as a `JobEntry` and split it — the shared harness the
/// split tests use.
fn split_line(line: &str) -> (String, String, String) {
    let text = format!("EXPERIENCE\n\n{line}\n");
    let parsed = parse_resume(&text);
    let entry = parsed
        .lines
        .iter()
        .find(|l| matches!(l.kind, LineKind::JobEntry))
        .unwrap_or_else(|| panic!("{line:?} must parse as a JobEntry"))
        .clone();
    split_entry(&entry)
}

/// R6-F5 — the two-space arm was hardened against reading a LOCATION column as
/// the employer, but the pipe/middot arm never was: it takes the first two
/// non-date segments as `(title, company)` verbatim, so
/// "Acme Corp | Berlin | 2021 – Present" recorded the CITY as the company and
/// the employer as the job title.
#[test]
fn the_pipe_form_names_the_company_not_the_city() {
    let (company, title, dates) = split_line("Acme Corp | Berlin | 2021 - Present");
    assert_eq!(company, "Acme Corp");
    assert_eq!(title, "");
    assert_eq!(dates, "2021 - Present");

    // A city column between title and dates must not displace either.
    let (company, title, _) = split_line("Senior Engineer | Acme Corp | Munich | 2018 - 2021");
    assert_eq!(company, "Acme Corp");
    assert_eq!(title, "Senior Engineer");

    // The ordinary three-segment form is untouched.
    let (company, title, _) = split_line("Senior Engineer | Acme Corp | 2021 - Present");
    assert_eq!(company, "Acme Corp");
    assert_eq!(title, "Senior Engineer");
}

/// R6-F6 — "Beruflicher Werdegang" is a standard German experience heading and
/// classified as `Other`, so every bullet under it was discarded and the prompt
/// was told the candidate had no experience.
#[test]
fn german_career_headings_classify_as_experience() {
    for heading in [
        "Beruflicher Werdegang",
        "BERUFLICHER WERDEGANG",
        "Werdegang",
        "Erfahrung",
        "Berufliche Erfahrung",
        "Erfahrungen",
        // …and the spellings that already worked must keep working.
        "BERUFSERFAHRUNG",
        "Arbeitserfahrung",
    ] {
        assert_eq!(
            classify_section(heading),
            SectionKind::Experience,
            "{heading:?} names an experience section"
        );
    }
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

/// R6-F7 — the round-5 Contact-arm rescue appends to `roles.last()`, so a
/// mid-section entry line the parser did not recognise (and every bullet under
/// it) landed under the PREVIOUS employer: one role absorbed another's header
/// line and all of its work.
#[test]
fn an_unparsed_entry_line_opens_its_own_role_instead_of_joining_the_last() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Senior Engineer | Globex Logistics | 2015 - 2018
- Built the billing API in Python and PostgreSQL

Acme Payments, Berlin, 2018 - 2021
- Shipped Docker containers onto a Kubernetes cluster
- Cut checkout latency with a Redis cache in front of the ledger service
";
    let set = extract_evidence(resume, "Docker Kubernetes Redis Python backend engineer");
    assert_eq!(set.roles.len(), 2, "two employers; got {:?}", set.roles);

    let globex = &set.roles[0];
    assert_eq!(globex.company, "Globex Logistics");
    assert_eq!(
        globex.bullets.len(),
        1,
        "Globex's role must not absorb Acme's header line or its work; got {:?}",
        globex.bullets
    );
    assert!(
        !globex
            .bullets
            .iter()
            .any(|b| b.text.contains("Docker") || b.text.contains("Acme")),
        "Acme's work must not be credited to Globex; got {:?}",
        globex.bullets
    );

    let acme = &set.roles[1];
    assert_eq!(
        acme.company, "Acme Payments",
        "the employer is salvaged from the unparsed line, never guessed"
    );
    assert_eq!(acme.dates, "2018 - 2021");
    assert_eq!(acme.bullets.len(), 2, "got {:?}", acme.bullets);
}

/// The other half of the same rule: a line that is NOT entry-shaped must keep
/// continuing the entry above it. "2021 to Present" on its own line is the
/// second half of the round-4 fixture's header, not a new employer.
#[test]
fn a_bare_date_line_does_not_open_a_second_role() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Acme Payments — Senior Backend Engineer
2021 to Present
- Shipped Docker containers onto a Kubernetes cluster
";
    let set = extract_evidence(resume, "Docker Kubernetes backend engineer");
    assert_eq!(
        set.roles.len(),
        1,
        "a continuation line is not a new employer; got {:?}",
        set.roles
    );
}

/// …and neither is prose that merely ENDS in a year. "Mentioning a date" is a
/// far weaker signal than "having a date column", and reading the two as the
/// same thing would turn an unbulleted sentence into an employer called
/// "Owned the ledger rewrite".
#[test]
fn prose_ending_in_a_year_does_not_open_a_role() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Senior Engineer | Acme Payments | 2021 - Present
Owned the ledger rewrite, delivered in 2019
- Shipped Docker containers onto a Kubernetes cluster
";
    let set = extract_evidence(resume, "Docker Kubernetes backend engineer");
    assert_eq!(
        set.roles.len(),
        1,
        "a sentence that mentions a year is not an entry line; got {:?}",
        set.roles
    );
    assert_eq!(set.roles[0].company, "Acme Payments");
}

// ── PR #963 round-7 findings ────────────────────────────────────────────────

/// R7-F1(a) — `is_date_only` gated on [`looks_like_date_span`], which is
/// satisfied by a SINGLE BARE YEAR, so an ordinary promotion note between two
/// entries ("Promoted to Staff Engineer, 2022") read as an employer plus a date
/// column and opened a role. "Mentioning a year" is not "having a date column";
/// the column needs a span separator, an open end or a month.
#[test]
fn a_promotion_note_between_two_entries_is_not_an_employer() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Senior Engineer | Acme Payments | 2021 - Present
- Cut checkout latency with a Redis cache in front of the ledger service
Promoted to Staff Engineer, 2022
- Shipped Docker containers onto a Kubernetes cluster

Backend Developer | Globex Logistics | 2018 - 2021
- Built the billing API in Python and PostgreSQL
";
    let set = extract_evidence(resume, "Docker Kubernetes Redis Python backend engineer");
    let companies: Vec<&str> = set.roles.iter().map(|r| r.company.as_str()).collect();
    assert!(
        !companies.iter().any(|c| c.contains("Promoted")),
        "a sentence is never an employer; got {companies:?}"
    );
    assert_eq!(
        companies,
        vec!["Acme Payments", "Globex Logistics"],
        "two real entries, and nothing between them"
    );

    // The note and the bullet under it stay with the employer above — the
    // promotion happened AT Acme, and no role was opened to strand them in.
    let acme = &set.roles[0];
    let texts: Vec<&str> = acme.bullets.iter().map(|b| b.text.as_str()).collect();
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Promoted to Staff Engineer")),
        "the note is kept as text under the entry it continues; got {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("Docker")),
        "the bullet after the note belongs to the same employer; got {texts:?}"
    );
}

/// The unit half of the same rule, both directions: every shape that IS a date
/// column, and the bare year that is not.
#[test]
fn a_date_column_needs_more_than_a_year_in_it() {
    for column in [
        "2018 - 2021",
        "2018 – 2021",
        "05/2018 – 07/2021",
        "Jan 2018 - Mar 2021",
        "2021 - Present",
        "2021 - Heute",
        "2021 –",
        "Jan 2022",
    ] {
        assert!(is_date_only(column), "{column:?} is a date column");
    }
    for prose in ["2022", "delivered in 2019", "40 warehouse sites", ""] {
        assert!(!is_date_only(prose), "{prose:?} is not a date column");
    }
}

/// R7-F1(b) — the salvage contradicted its own contract. A label with no comma
/// left in it gives [`split_two_space_label`] nothing to split, and its
/// comma-less arm hands the WHOLE LABEL back as the company — so the role
/// opened by an entry-shaped line was named after the sentence on it. The
/// employer is salvaged or it is empty; it is never the verbatim line.
#[test]
fn an_unresolvable_entry_label_opens_an_unattributed_role() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Senior Engineer | Globex Logistics | 2015 - 2018
- Built the billing API in Python and PostgreSQL

Led the platform rewrite, Jan 2019 - Dec 2021
- Shipped Docker containers onto a Kubernetes cluster
- Cut checkout latency with a Redis cache in front of the ledger service
";
    let set = extract_evidence(resume, "Docker Kubernetes Redis Python backend engineer");
    assert_eq!(set.roles.len(), 2, "the date column still opens a role");
    assert_eq!(
        set.roles[0].company, "Globex Logistics",
        "and the previous employer does not absorb it (R6-F7 stays fixed)"
    );

    let salvaged = &set.roles[1];
    assert_eq!(
        salvaged.company, "",
        "an unresolvable label is an UNATTRIBUTED role, not an invented employer"
    );
    assert_eq!(salvaged.title, "");
    assert_eq!(salvaged.dates, "Jan 2019 - Dec 2021", "the column is kept");
    // Refusing to name an employer must not silently delete the line either.
    let texts: Vec<&str> = salvaged.bullets.iter().map(|b| b.text.as_str()).collect();
    assert_eq!(
        texts,
        vec![
            "Led the platform rewrite",
            "Shipped Docker containers onto a Kubernetes cluster",
            "Cut checkout latency with a Redis cache in front of the ledger service",
        ],
        "the unresolved label stays as text in its own bucket"
    );
}

// ── PR #963 round-8 findings ────────────────────────────────────────────────

/// R8-F1 — `EXPERIENCE_HEADINGS` carries the bare substring `career`, and the
/// experience test runs BEFORE the summary one, so "Career Summary" (and
/// "Career Objective", and "Career Profile") classified as EXPERIENCE. The
/// prose under a summary heading then reached the experience arm and became
/// work bullets under an invented, unattributed role.
#[test]
fn a_career_summary_heading_is_a_summary_not_experience() {
    for heading in [
        "Career Summary",
        "CAREER OBJECTIVE",
        "Career Profile",
        "Career Objective Statement",
    ] {
        assert_eq!(
            classify_section(heading),
            SectionKind::Summary,
            "{heading:?} names a summary, not a work history"
        );
    }
    // …and a career heading that names no summary is still Experience.
    for heading in ["Career History", "CAREER", "Career Highlights"] {
        assert_eq!(
            classify_section(heading),
            SectionKind::Experience,
            "{heading:?} still names a work history"
        );
    }
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

/// R8-F8 — with R7's gate in place the comma-less salvage's remaining reachable
/// input is an ORDINARY entry line: "Acme Payments, Jan 2019 - Dec 2021" carries
/// a real date column and a label that is nothing BUT the employer, and R7
/// filed it as an unattributed role with the employer surviving only as bullet
/// text.
#[test]
fn a_comma_less_entry_label_names_the_employer() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Senior Engineer | Globex Logistics | 2015 - 2018
- Built the billing API in Python and PostgreSQL

Acme Payments, Jan 2019 - Dec 2021
- Shipped Docker containers onto a Kubernetes cluster
- Cut checkout latency with a Redis cache in front of the ledger service
";
    let set = extract_evidence(resume, "Docker Kubernetes Redis Python backend engineer");
    assert_eq!(set.roles.len(), 2, "got {:?}", set.roles);
    assert_eq!(set.roles[0].company, "Globex Logistics");

    let acme = &set.roles[1];
    assert_eq!(
        acme.company, "Acme Payments",
        "everything in front of a real date column, with no comma left in it, IS the employer"
    );
    assert_eq!(acme.title, "");
    assert_eq!(acme.dates, "Jan 2019 - Dec 2021");
    let texts: Vec<&str> = acme.bullets.iter().map(|b| b.text.as_str()).collect();
    assert_eq!(
        texts,
        vec![
            "Shipped Docker containers onto a Kubernetes cluster",
            "Cut checkout latency with a Redis cache in front of the ledger service",
        ],
        "an attributed label is role metadata, never repeated as its own bullet"
    );
}

/// The discriminator that keeps R7-F1(b) closed, both directions. A label is
/// the employer only when it READS like a name; a sentence never does.
#[test]
fn a_comma_less_entry_label_is_an_employer_only_when_it_reads_like_a_name() {
    for name in [
        "Acme Payments",
        "ACME PAYMENTS",
        "Nordwind Systeme GmbH",
        "IBM",
        "Johnson & Johnson",
        "3M Deutschland",
    ] {
        assert_eq!(
            salvage_entry_label(name),
            Some((name.to_string(), String::new())),
            "{name:?} reads as an employer"
        );
    }
    for prose in [
        "Led the platform rewrite",
        "Promoted to Staff Engineer",
        "Owned the ledger rewrite",
        "Rebuilt the settlement pipeline end to end for the payments group",
        "acme payments",
    ] {
        assert_eq!(
            salvage_entry_label(prose),
            None,
            "{prose:?} is a sentence, and a sentence is never an employer"
        );
    }
}

/// The posting used by the ordering test: `kubernetes` ×3, `docker` ×2,
/// `terraform` ×2, everything else exactly once, and no accidental repeat that
/// would join the weighted group.
const WEIGHTED_JOB: &str = "\
Kubernetes is the platform. Kubernetes schedules every Docker workload. \
Kubernetes handles rollout. Docker images come from CI. Terraform provisions \
infrastructure. Terraform manages networking. Ansible configures hosts.";

/// R8 follow-up — both skills lists were sorted ALPHABETICALLY, purely for
/// determinism. Every consumer truncates (`agent::tools_quality`'s
/// `.take(MAX_SKILLS)`), so what survived was an alphabetical PREFIX of the gap
/// list: "ansible" kept, "terraform" cut, and `skillsTruncated` reports only a
/// COUNT, so nothing downstream can see the bias. Relevance-first ordering makes
/// a truncated list the top-N by construction; alphabetical is the tiebreak, so
/// determinism is unchanged.
#[test]
fn skills_lists_lead_with_the_postings_own_priorities() {
    let resume = "EXPERIENCE\n\n\
                  Senior Engineer | Acme | 2021 - 2024\n\
                  - Ran Docker on Kubernetes\n";
    let set = extract_evidence(resume, WEIGHTED_JOB);

    // The posting says "Kubernetes" three times and "Docker" twice; alphabetical
    // order puts them the other way round.
    assert_eq!(
        set.skills_present,
        vec!["kubernetes".to_string(), "docker".to_string()],
        "the present list leads with what the posting asks for most"
    );

    // The gap list leads with the twice-named Terraform, ahead of every
    // once-named term including alphabetically-earlier "ansible".
    assert_eq!(
        set.skills_absent.first().map(String::as_str),
        Some("terraform"),
        "got {:?}",
        set.skills_absent
    );
    // …and inside the once-named group the tiebreak is alphabetical, which is
    // what keeps the output deterministic across runs.
    assert_eq!(
        set.skills_absent.get(1).map(String::as_str),
        Some("ansible"),
        "got {:?}",
        set.skills_absent
    );
    let tied = &set.skills_absent[1..];
    assert!(
        tied.windows(2).all(|w| w[0] <= w[1]),
        "equally-relevant terms stay alphabetical; got {tied:?}"
    );

    // Same input, same output — the relevance map cannot introduce HashMap
    // iteration order into a user-visible list.
    assert_eq!(
        extract_evidence(resume, WEIGHTED_JOB).skills_absent,
        set.skills_absent
    );
}

/// The curated-language gate is what `validate::content::ats` reads before it
/// dares report a density number, so the two halves must never drift: a
/// language claiming curation with no list behind it re-opens R5-F5, and a
/// curated language reported as uncurated silently switches the check off.
#[test]
fn curated_function_word_languages_match_the_lists_behind_them() {
    // German is curated AND has a list.
    assert!(has_curated_function_words("de"));
    assert!(!function_words("de").is_empty());
    // English is curated by the kernel's own STOPWORDS — an empty list here
    // means "already filtered", not "unfiltered".
    assert!(has_curated_function_words("en"));
    assert!(function_words("en").is_empty());
    // Everything else is uncurated, and says so.
    for lang in ["fr", "es", "it", "nl", "pt", "zz", ""] {
        assert!(
            !has_curated_function_words(lang),
            "{lang} has no curated function-word list"
        );
        assert!(function_words(lang).is_empty());
    }
}

// ── PR #963 round-9 findings ────────────────────────────────────────────────

/// R9-F1 — the pipe/middot arm picked its date column with
/// `looks_like_date_span`, which is TRUE for a bare present-tense marker
/// (`is_open_ended` fires on the word alone, with no year anywhere). An
/// employer whose NAME is or contains one — Current (current.com) is a real
/// fintech, Current Health a real medtech, "Aktuell" opens plenty of German
/// company names — was therefore selected as the date column AND filtered out
/// of the segments, so the job TITLE was recorded as the employer and the
/// employer as the date.
#[test]
fn the_pipe_form_does_not_read_an_employer_named_current_as_the_date_column() {
    let (company, title, dates) = split_line("Senior Engineer | Current | 2021 - Present");
    assert_eq!(
        company, "Current",
        "the employer is the company, not the date"
    );
    assert_eq!(title, "Senior Engineer");
    assert_eq!(dates, "2021 - Present");

    // The same word inside a longer name, and the German marker.
    let (company, title, _) = split_line("Senior Engineer | Current Health | 2021 - Present");
    assert_eq!(company, "Current Health");
    assert_eq!(title, "Senior Engineer");
    let (company, title, _) = split_line("Entwicklerin | Aktuell Media GmbH | 2018 - 2021");
    assert_eq!(company, "Aktuell Media GmbH");
    assert_eq!(title, "Entwicklerin");

    // The other half of the boundary: every date column the parser actually
    // hands this arm must still resolve. The rows are the shapes
    // `export::parser`'s `DATE_RE`/`SOLO_DATE_RE` admit — a lone year and a
    // separator word both appear here, and `is_date_only` would reject both
    // (see `is_date_column_segment` for why it is not the test used).
    for (line, expected) in [
        (
            "Senior Engineer | Acme Corp | 2021 - Present",
            "2021 - Present",
        ),
        (
            "Senior Engineer | Acme Corp | Jan 2018 - Mar 2021",
            "Jan 2018 - Mar 2021",
        ),
        ("Senior Engineer | Acme Corp | 2018 to 2021", "2018 to 2021"),
        (
            "Senior Engineer | Acme Corp | 2021 bis Heute",
            "2021 bis Heute",
        ),
        ("Senior Engineer | Acme Corp | 2022", "2022"),
    ] {
        let (company, title, dates) = split_line(line);
        assert_eq!(
            (company.as_str(), title.as_str(), dates.as_str()),
            ("Acme Corp", "Senior Engineer", expected),
            "{line:?}"
        );
    }
}

/// A French posting: "pour" ×4 and "avec" ×3 outnumber every term that names a
/// skill, which is what an unfiltered vocabulary looks like in any language
/// whose function words nobody has listed yet.
const FR_JOB: &str = "\
Développeur backend pour notre plateforme de paiement. Vous concevez des \
services pour nos clients européens, pour la fiabilité du service et pour \
l'équipe produit. Nous cherchons une personne à l'aise avec Kubernetes, avec \
Terraform et avec Ansible. Kubernetes orchestre nos conteneurs en production.";

const FR_RESUME: &str = "EXPÉRIENCE\n\n\
                         Ingénieur Backend | Acme Payments | 2021 - 2024\n\
                         - Déploiement des conteneurs sur Kubernetes pour la plateforme de paiement\n";

/// A German posting whose most-repeated term is a real requirement, because
/// `FUNCTION_WORDS_DE` removes the fillers before the frequencies are read.
const DE_WEIGHTED_JOB: &str = "\
Wir suchen eine Backend-Entwicklerin. Kubernetes betreibt unsere Dienste. \
Kubernetes skaliert die Plattform. Terraform provisioniert die Infrastruktur. \
Terraform verwaltet die Netzwerke. Ansible konfiguriert die Hosts.";

/// R10-F2 — the skills split filters through `function_words(lang)` but never
/// asked [`has_curated_function_words`], and for `fr`/`es`/`it`/`nl`/`pt` that
/// list is EMPTY. Round 8 then ordered both lists by how often the POSTING
/// states each term, so the words the filter would have removed — the ones a
/// posting repeats most — sorted to the TOP of the gap list a generation prompt
/// consumes and truncates. The ordering made an uncurated language WORSE, not
/// better.
///
/// The degrade chosen here, and why, is documented at the call site: the
/// relevance key is switched off where it cannot mean what it claims, so the
/// list falls back to its deterministic alphabetical order and makes no
/// relevance claim at all.
#[test]
fn an_uncurated_language_makes_no_relevance_claim_about_its_gap_list() {
    let set = extract_evidence(FR_RESUME, FR_JOB);

    assert!(
        set.skills_absent.windows(2).all(|w| w[0] <= w[1]),
        "with no function-word list, posting frequency ranks the FILLERS first, so \
         the split may not claim relevance; got {:?}",
        set.skills_absent
    );

    // The residual, pinned rather than hidden: the fillers are still IN the
    // list. Only a curated `function_words("fr")` can remove them, and adding
    // one re-enables the relevance order in the same edit.
    assert!(
        set.skills_absent.iter().any(|s| s == "avec"),
        "the filler is still listed — this fix demotes it, it does not filter it; got {:?}",
        set.skills_absent
    );

    // The guard: a CURATED language keeps the round-8 relevance order, so the
    // switch bites exactly where the filter is missing and nowhere else.
    let de_set = extract_evidence(
        "BERUFSERFAHRUNG\n\nEntwicklerin | Acme | 2021 - 2024\n- Dienste auf Kubernetes betrieben\n",
        DE_WEIGHTED_JOB,
    );
    assert_eq!(
        de_set.skills_absent.first().map(String::as_str),
        Some("terraform"),
        "German is curated, so frequency still ranks real requirements; got {:?}",
        de_set.skills_absent
    );
}

/// R11-F3 — [`split_two_space_label`]'s comma-less arm returns the whole label
/// as the company with no location test, so a label made of NOTHING but
/// geography names a CITY as the employer.
///
/// The reviewer's "Berlin, Germany" reaches that arm through the peel loop
/// above it: the loop strips location-only comma tails one at a time, and what
/// it leaves ("Berlin") is comma-less and unguarded. The pipe arm and the
/// two-space arm both refuse to name a city; the fallback did not.
#[test]
fn an_all_geography_entry_label_names_no_employer() {
    // The reviewer's exact shape, and the bare single-token twin.
    assert_eq!(
        split_two_space_label("Berlin, Germany"),
        (String::new(), String::new()),
        "a label that is only geography identifies no employer"
    );
    assert_eq!(
        split_two_space_label("Berlin"),
        (String::new(), String::new())
    );

    // …and the shape a user actually uploads: an extracted PDF whose entry line
    // carries only the location and the date column, with the employer on the
    // line above it.
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Berlin, Germany, 2018 - 2021
- Shipped Docker containers onto a Kubernetes cluster
";
    let set = extract_evidence(resume, "Docker Kubernetes backend engineer");
    let companies: Vec<&str> = set.roles.iter().map(|r| r.company.as_str()).collect();
    assert!(
        !companies.contains(&"Berlin"),
        "a city is never an employer; got {companies:?}"
    );

    // The guard: a real employer with a location tail still resolves.
    assert_eq!(
        split_two_space_label("Globex Logistics, Munich, Germany"),
        ("Globex Logistics".to_string(), String::new())
    );
}

/// R11-F4 — the pipe arm reads `[title, company]` positionally, on the stated
/// justification that it is "the order every template in this repo renders".
/// That is true of what this app GENERATES and false of what a user UPLOADS:
/// "Acme Corp | Senior Engineer | 2021 - Present" is an ordinary company-first
/// résumé line, and reading it positionally records the employer as
/// "Senior Engineer" — which then merges every entry that shares a title into
/// one employer and hands the generation prompt a role at a company called
/// "Senior Engineer".
#[test]
fn a_company_first_pipe_entry_names_the_company_not_the_title() {
    let (company, title, dates) = split_line("Acme Corp | Senior Engineer | 2021 - Present");
    assert_eq!(company, "Acme Corp");
    assert_eq!(title, "Senior Engineer");
    assert_eq!(dates, "2021 - Present");

    // The pinned title-first reading is untouched wherever the legal form does
    // not say otherwise — including the case where the COMPANY carries one.
    let (company, title, _) = split_line("Senior Engineer | Acme Corp | 2021 - Present");
    assert_eq!(company, "Acme Corp");
    assert_eq!(title, "Senior Engineer");
    let (company, title, _) = split_line("IT-Beraterin | IBM Deutschland GmbH | 2015 - 2018");
    assert_eq!(company, "IBM Deutschland GmbH");
    assert_eq!(title, "IT-Beraterin");

    // The harm the drift check cannot mask: `extract_evidence` is what the
    // generation prompt reads, and it was told the employer was a job title.
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE

Acme Corp | Senior Engineer | 2021 - Present
- Shipped Docker containers onto a Kubernetes cluster

Globex Ltd | Senior Engineer | 2018 - 2021
- Built the billing API for forty warehouse sites
";
    let set = extract_evidence(resume, "Docker Kubernetes backend engineer");
    let companies: Vec<&str> = set.roles.iter().map(|r| r.company.as_str()).collect();
    assert_eq!(companies, vec!["Acme Corp", "Globex Ltd"]);
}
