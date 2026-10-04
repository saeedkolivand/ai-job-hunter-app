//! `split_entry` / `split_two_space_label` / `salvage_entry_label`: which segment of an entry line is
//! the employer, which the title, and when nothing is named at all.

use super::*;
use crate::export::parser::parse_resume;
use crate::export::types::LineKind;

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
    // A known city as the tail.
    let (company, title, dates) = split_line("Acme GmbH, Berlin    2021 - Present");
    assert_eq!(company, "Acme GmbH");
    assert_eq!(title, "");
    assert_eq!(dates, "2021 - Present");

    // An UNLISTED city: the legal form in the head is the evidence.
    let (company, title, _) = split_line("Nordwind Systeme GmbH, Ingolstadt    2018 - 2021");
    assert_eq!(company, "Nordwind Systeme GmbH");
    assert_eq!(title, "");

    // City plus country.
    let (company, _, _) = split_line("Globex Logistics, Munich, Germany    2018 - 2021");
    assert_eq!(company, "Globex Logistics");

    // Nothing says otherwise → the title-first reading is untouched.
    let (company, title, _) = split_line("Senior Engineer, Acme Corp    2021 - Present");
    assert_eq!(company, "Acme Corp");
    assert_eq!(title, "Senior Engineer");
}

/// R5-F7 — `has_legal_form` tested EVERY token, and `LEGAL_FORMS` carries the
/// ordinary English title words `group`, `company` and `holding`. So the head of
/// "Group Product Manager, Acme Payments" looked like a company name, the
/// company-first rule fired, and the employer was recorded as "Group Product
/// Manager" with the real one discarded. A legal form is a SUFFIX.
#[test]
fn a_title_starting_with_an_ambiguous_legal_word_still_resolves_title_first() {
    let (company, title, _) = split_line("Group Product Manager, Acme Payments    2021 - Present");
    assert_eq!(company, "Acme Payments");
    assert_eq!(title, "Group Product Manager");

    // The same for the other two ambiguous English words in the list.
    let (company, title, _) = split_line("Company Secretary, Globex Logistics    2018 - 2021");
    assert_eq!(company, "Globex Logistics");
    assert_eq!(title, "Company Secretary");

    // …and a genuine trailing legal form still names the company first.
    let (company, title, _) = split_line("Nordwind Systeme GmbH, Ingolstadt    2018 - 2021");
    assert_eq!(company, "Nordwind Systeme GmbH");
    assert_eq!(title, "");
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
