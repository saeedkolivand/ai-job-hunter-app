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
    assert!(
        set.skills_present.windows(2).all(|w| w[0] <= w[1]),
        "skills_present must be sorted for deterministic output"
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
