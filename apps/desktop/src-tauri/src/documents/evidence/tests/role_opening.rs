//! Which résumé lines open a role under Experience, which join the entry above, and who gets named
//! the employer.

use super::*;

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

/// R6-F7 — the round-5 Contact-arm rescue appends to `roles.last()`, so a
/// mid-section entry line the parser did not recognise (and every bullet under
/// it) landed under the PREVIOUS employer: one role absorbed another's header
/// line and all of its work.
///
/// Run once with a closed span and once with a `Today` column — the user-visible
/// consequence of the `is_date_only` gap.
#[test]
fn an_unparsed_entry_line_opens_its_own_role_instead_of_joining_the_last() {
    for (acme_line, dates) in [
        ("Acme Payments, Berlin, 2018 - 2021", "2018 - 2021"),
        // The same shape with the date column spelled `Today`: before `is_date_only` knew
        // it, `trailing_date_column` returned `None`, this arm fell through to
        // `attach_to_role`, and Acme's header line AND both of its bullets landed on
        // Globex's role.
        ("Acme Payments, Berlin, 2020 - Today", "2020 - Today"),
    ] {
        let resume = format!(
            "\
Jane Doe
jane@example.com

EXPERIENCE

Senior Engineer | Globex Logistics | 2015 - 2018
- Built the billing API in Python and PostgreSQL

{acme_line}
- Shipped Docker containers onto a Kubernetes cluster
- Cut checkout latency with a Redis cache in front of the ledger service
"
        );
        let set = extract_evidence(&resume, "Docker Kubernetes Redis Python backend engineer");
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
        assert_eq!(acme.dates, dates);
        assert_eq!(acme.bullets.len(), 2, "got {:?}", acme.bullets);
    }
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
