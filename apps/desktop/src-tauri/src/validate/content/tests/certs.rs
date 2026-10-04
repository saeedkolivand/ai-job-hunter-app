//! `factual.unsourced_certification` and `factual.unsourced_credential`: the acronym arm and
//! the issuer arm.

use super::credential_corpus::*;
use super::{support::*, *};

/// A2b positive: a certification the source never names, quoted whole.
#[test]
fn a_certification_the_source_never_names_is_critical() {
    let generated = "Jane Doe\n\nCERTIFICATIONS\n\n\
         AWS Certified Solutions Architect - Professional\n";
    let report = report_against(generated, EN_SOURCE);
    // The PROSE arm, so the Warning code — see
    // `the_acronym_arm_is_critical_and_the_prose_arm_is_a_warning`.
    assert_eq!(
        first_evidence(&report, FACTUAL_UNSOURCED_CREDENTIAL),
        Some("AWS Certified Solutions Architect - Professional"),
        "the finding must fire and quote the entry; report carried {:?}",
        codes(&report)
    );
}

/// A2b: an acronym is checked against the source case-INSENSITIVELY, so a
/// source that writes it in a skills line still supports the claim — while the
/// generated side only reads it as a certification in caps.
#[test]
fn a_certification_acronym_is_supported_by_any_casing_in_the_source() {
    let generated = "Jane Doe\n\nCERTIFICATIONS\n\nPMP\n";
    let with_source = format!("{EN_SOURCE}\n\nCERTIFICATIONS\n\npmp (2021)\n");
    silent(
        &report_against(generated, &with_source),
        FACTUAL_UNSOURCED_CERTIFICATION,
    );
    assert!(
        codes(&report_against(generated, EN_SOURCE)).contains(&FACTUAL_UNSOURCED_CERTIFICATION),
        "without the source entry the same document must be reported — otherwise the \
         assertion above passes because nothing ever fires"
    );
}

/// A2b negative, and the one that decides whether this check can ship at all:
/// a source that HOLDS the certification must clear a reworded restatement of
/// it.
#[test]
fn a_reworded_certification_the_source_holds_is_not_reported() {
    let source = "Jane Doe\n\nCERTIFICATIONS\n\n\
         AWS Certified Solutions Architect - Associate\n\
         Certified Kubernetes Administrator (CKA)\n";
    let generated = "Jane Doe\n\nCERTIFICATIONS\n\nAWS Certified Solutions Architect\nCKA\n";
    silent(
        &report_against(generated, source),
        FACTUAL_UNSOURCED_CERTIFICATION,
    );
}

/// A2b cross-language: German output, English source, same issuer. The
/// comparison is on the ISSUER KEY, which is a proper noun in both languages —
/// `Zertifizierter` and `Certified` never have to match each other.
#[test]
fn a_german_certification_generated_from_an_english_source_matches_on_the_issuer() {
    let source = "Jane Doe\n\nCERTIFICATIONS\n\nAWS Certified Solutions Architect - Associate\n";
    let truthful =
        "Jana Mustermann\n\nZERTIFIZIERUNGEN\n\nZertifizierter AWS Solutions Architect\n";
    let invented = "Jana Mustermann\n\nZERTIFIZIERUNGEN\n\nZertifizierter Cisco Netzwerkexperte\n";

    silent(
        &report_against(truthful, source),
        FACTUAL_UNSOURCED_CERTIFICATION,
    );
    assert!(
        codes(&report_against(invented, source)).contains(&FACTUAL_UNSOURCED_CREDENTIAL),
        "a Cisco certification on an AWS-only source is an invention in any language"
    );
}

/// A2b mutation check: delete the FEATURE — the issuer requirement — and
/// "certified" on its own becomes a credential.
///
/// A bullet about certifying a release, or about a certified data centre, is
/// ordinary résumé prose. It must produce nothing, and this is the assertion
/// that goes red if the "an issuer must sit near the certification word" rule
/// is dropped in favour of matching the word alone.
#[test]
fn a_certification_word_with_no_issuer_near_it_is_not_a_credential() {
    let generated = "Jane Doe\n\nEXPERIENCE\n\n\
         Backend Developer | Globex Logistics | 2018 - 2020\n\
         - Certified the settlement release each Thursday against the ISO checklist\n\
         - Moved the ledger into a certified data centre in Frankfurt\n";
    let report = report_against(generated, EN_SOURCE_FOUR_YEARS);
    silent(&report, FACTUAL_UNSOURCED_CERTIFICATION);
}

/// A letter is checked for tenure and certifications too — that is where "I
/// bring 12 years of experience" actually gets written — but the JOB AD may
/// never vouch for either.
///
/// The ad here demands ten years and an AWS certification. Both are statements
/// about the ROLE; letting them count as evidence would make every posting its
/// own alibi for a letter that echoes it back.
#[test]
fn a_letters_credentials_are_measured_against_the_resume_never_the_posting() {
    let ad = "We need 10+ years of experience and an AWS Certified Solutions Architect.";
    let letter = "Jane Doe\n\nDear Hiring Manager,\n\n\
         I bring 10 years of experience to this role, and I am an AWS Certified Solutions \
         Architect.\n\nBest regards,\nJane Doe\n";
    let report = letter_report_for(letter, EN_SOURCE_FOUR_YEARS, ad);
    let fired = codes(&report);
    assert!(
        fired.contains(&FACTUAL_INFLATED_EXPERIENCE),
        "the posting's own '10+ years' must not vouch for the letter; got {fired:?}"
    );
    // The PROSE arm — "an AWS Certified Solutions Architect" is an issuer beside
    // a certification word, which is a Warning since round four.
    assert!(
        fired.contains(&FACTUAL_UNSOURCED_CREDENTIAL),
        "the posting's own certification requirement must not vouch for the letter; got \
         {fired:?}"
    );
}

/// A certification named by its ACRONYM in one document and spelled out in the
/// other is the SAME certification, in both directions.
///
/// This is the defect the sibling critic reproduced: the acronym pass emitted
/// `cka` while the issuer pass emitted `kubernetes`, and the comparison is on
/// keys — so a source holding `Certified Kubernetes Administrator` did not
/// support a generated `CKA`, and a source holding `CKA` did not support the
/// spelled-out form. Both directions produced false Criticals on truthful
/// documents.
///
/// The earlier negative test passed only because its fixture wrote BOTH forms
/// on one line (`Certified Kubernetes Administrator (CKA)`), which is the
/// "passes for the wrong reason" shape — each fixture here names exactly one
/// form.
///
/// Mutation check: drop the key column from `CERT_ACRONYMS` (emit only the
/// acronym) and the first arm goes red.
#[test]
fn a_certification_is_the_same_credential_by_acronym_or_by_name() {
    let expansion = "Jane Doe\n\nCERTIFICATIONS\n\nCertified Kubernetes Administrator\n";
    let acronym = "Jane Doe\n\nCERTIFICATIONS\n\nCKA\n";
    silent(
        &report_against(acronym, expansion),
        FACTUAL_UNSOURCED_CERTIFICATION,
    );
    silent(
        &report_against(expansion, acronym),
        FACTUAL_UNSOURCED_CERTIFICATION,
    );
    // …and an acronym whose long form carries no issuer token at all, which is
    // why `CERT_ACRONYMS` carries the expansion as a third column.
    let spelled_out =
        "Jane Doe\n\nCERTIFICATIONS\n\nCertified Information Systems Security Professional\n";
    silent(
        &report_against("Jane Doe\n\nCERTIFICATIONS\n\nCISSP\n", spelled_out),
        FACTUAL_UNSOURCED_CERTIFICATION,
    );
    // The control: none of the above is silence-by-default.
    assert!(
        codes(&report_against(acronym, EN_SOURCE)).contains(&FACTUAL_UNSOURCED_CERTIFICATION),
        "a source holding no certification at all must still report one"
    );
}

/// `split_sections` promotes a lone `CISSP` to a section heading, so the
/// credential lived in `Section::heading` and no line-scan could see it.
///
/// Sixteen of the twenty-three curated acronyms are four characters or more and
/// were invisible this way — a hole in the one arm whose evidence justifies a
/// Critical.
///
/// Mutation check: stop scanning `section.heading` in `cert_claims` and the
/// four-character arms go red.
#[test]
fn a_bare_acronym_under_a_certifications_heading_is_still_reported() {
    for acronym in ["CISSP", "PRINCE2", "RHCSA", "CCNA", "PMP"] {
        let generated = format!("Jane Doe\n\nCERTIFICATIONS\n\n{acronym}\n");
        let report = report_against(&generated, EN_SOURCE);
        assert_eq!(
            first_evidence(&report, FACTUAL_UNSOURCED_CERTIFICATION),
            Some(acronym),
            "{acronym} alone under a heading is still a claim; report carried {:?}",
            codes(&report)
        );
    }
    // The negative half: the same acronyms, present in the source, stay silent.
    for acronym in ["CISSP", "PRINCE2", "RHCSA", "CCNA", "PMP"] {
        let generated = format!("Jane Doe\n\nCERTIFICATIONS\n\n{acronym}\n");
        let source = format!("{EN_SOURCE}\n\nCERTIFICATIONS\n\n{acronym}\n");
        silent(
            &report_against(&generated, &source),
            FACTUAL_UNSOURCED_CERTIFICATION,
        );
    }
}

/// The two arms rest on different evidence and therefore carry different
/// severities — and the Critical must not be reachable from the prose path.
#[test]
fn the_acronym_arm_is_critical_and_the_prose_arm_is_a_warning() {
    let acronym = report_against("Jane Doe\n\nCERTIFICATIONS\n\nCISSP\n", EN_SOURCE);
    assert_eq!(
        fired(&acronym, FACTUAL_UNSOURCED_CERTIFICATION)[0].severity,
        Severity::Critical,
        "a curated uppercase token is bounded evidence"
    );
    silent(&acronym, FACTUAL_UNSOURCED_CREDENTIAL);

    let phrase = report_against(
        "Jane Doe\n\nCERTIFICATIONS\n\nAWS Certified Solutions Architect\n",
        EN_SOURCE,
    );
    assert_eq!(
        fired(&phrase, FACTUAL_UNSOURCED_CREDENTIAL)[0].severity,
        Severity::Warning,
        "the prose arm reads unbounded language and has been measured wrong twice"
    );
    silent(&phrase, FACTUAL_UNSOURCED_CERTIFICATION);
}

/// One credential written both ways is one finding, and it is the ACRONYM one —
/// the stronger evidence of the two. Letting document order pick would let it
/// decide a severity.
#[test]
fn a_certification_named_in_both_forms_is_reported_once_by_its_stronger_arm() {
    let generated = "Jane Doe\n\nCERTIFICATIONS\n\n\
         Certified Kubernetes Administrator\n\
         CKA\n";
    let report = report_against(generated, EN_SOURCE);
    assert_eq!(
        fired(&report, FACTUAL_UNSOURCED_CERTIFICATION).len(),
        1,
        "one credential, one finding; report carried {:?}",
        codes(&report)
    );
    silent(&report, FACTUAL_UNSOURCED_CREDENTIAL);
}

/// Two DIFFERENT acronyms sharing an issuer are two findings, not one.
///
/// `CKA` and `CKS` both key to `kubernetes` (the issuer, not the credential),
/// and deduping on that shared key alone silently dropped the second invented
/// certification — the user fixed CKA and shipped CKS untouched. Same shape
/// for `CCNA`/`CCNP` on `cisco`.
///
/// Mutation check: dedup on `keys.first()` (the issuer key) again, for every
/// arm, and the `len()` assertion drops to 1.
#[test]
fn two_different_acronyms_from_one_issuer_are_two_findings() {
    for (first, second) in [("CKA", "CKS"), ("CCNA", "CCNP")] {
        let generated = format!("Jane Doe\n\nCERTIFICATIONS\n\n{first}\n{second}\n");
        let report = report_against(&generated, EN_SOURCE);
        let evidence = fired_evidence(&report, FACTUAL_UNSOURCED_CERTIFICATION);
        assert_eq!(
            evidence.len(),
            2,
            "{first} and {second} are two distinct invented certifications; report carried {:?}",
            codes(&report)
        );
        assert!(evidence.contains(&first) && evidence.contains(&second));
    }
}
