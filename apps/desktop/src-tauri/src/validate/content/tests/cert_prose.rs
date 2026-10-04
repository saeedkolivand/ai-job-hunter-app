//! The issuer arm's false-positive guards: "certified" as a verb, as a vendor's adjective,
//! and inside a derivational noun.

use super::credential_corpus::CERT_PROSE;
use super::{support::*, *};

/// `line` as the one bullet of a platform engineer's role.
fn platform_bullet(line: &str) -> String {
    format!(
        "Jane Doe\n\nEXPERIENCE\n\n\
         Platform Engineer | Acme Payments | 2019 - Present\n{line}\n"
    )
}

/// Each of `lines`, as a role's only bullet, raises none of `codes`.
#[track_caller]
fn assert_bullets_raise_nothing(lines: &[&str], codes: &[&str]) {
    for line in lines {
        let report = report_against(&platform_bullet(line), EN_SOURCE);
        for code in codes {
            silent(&report, code);
        }
    }
}

/// Each of `credentials`, listed under a CERTIFICATIONS heading the source lacks,
/// is reported; `why` completes the failure message.
#[track_caller]
fn assert_credentials_reported(credentials: &[&str], why: &str) {
    for credential in credentials {
        let generated = format!("Jane Doe\n\nCERTIFICATIONS\n\n{credential}\n");
        assert!(
            codes(&report_against(&generated, EN_SOURCE)).contains(&FACTUAL_UNSOURCED_CREDENTIAL),
            "{credential} {why}"
        );
    }
}

/// "Certified" is a past-tense VERB at least as often as it is an adjective,
/// and every bullet here is a real thing a platform engineer did.
///
/// The issuer pass used to accept any issuer token within 60 characters of any
/// certification word, so all four fired and the user was told to delete an
/// achievement. The claims side now requires the two to be ADJACENT tokens —
/// which is what "AWS Certified" is and what "certified … on AWS" is not.
///
/// Mutation check: give `Side::Claims` the same window `Side::Source` uses and
/// this goes red.
#[test]
fn a_certification_word_used_as_a_verb_is_not_a_credential() {
    let report = report_against(CERT_PROSE, EN_SOURCE);
    silent(&report, FACTUAL_UNSOURCED_CERTIFICATION);
    silent(&report, FACTUAL_UNSOURCED_CREDENTIAL);

    // The shape that needs ADJACENCY specifically, and that the role-noun rule
    // cannot reject on its own: a verb, an issuer several words away from it,
    // and a role noun sitting exactly where a credential would put one.
    // Without the adjacency test these read as credentials.
    assert_bullets_raise_nothing(
        &[
            "- Certified the release for our AWS solutions architect to sign off",
            "- Certified each image before handing it to the Docker platform engineer",
            "- Zertifizierte die Freigabe für unseren AWS Solutions Architect",
        ],
        &[
            FACTUAL_UNSOURCED_CERTIFICATION,
            FACTUAL_UNSOURCED_CREDENTIAL,
        ],
    );

    // The control: the adjectival form on the same issuer IS reported.
    let adjectival = "Jane Doe\n\nCERTIFICATIONS\n\nAWS Certified Solutions Architect\n";
    assert!(
        codes(&report_against(adjectival, EN_SOURCE)).contains(&FACTUAL_UNSOURCED_CREDENTIAL),
        "adjacency is the discriminator, not silence"
    );
}

/// `Docker Certified images` is a vendor's word for a PRODUCT, and it is
/// grammatically identical to a credential: issuer, certification word,
/// adjacent. Seven of seven fired once adjacency alone was the rule.
///
/// A real certification names a role its holder is certified to fill, so the
/// claims side requires one within `CERT_ROLE_NOUN_WINDOW_TOKENS`.
///
/// Mutation check: drop `names_a_certified_role` from the claims arm and the
/// whole first loop goes red.
#[test]
fn an_issuer_certified_product_is_not_a_credential() {
    assert_bullets_raise_nothing(
        &[
            "- Shipped Docker Certified images to the internal registry",
            "- Standardised on Red Hat certified build images for the fleet",
            "- Moved the ledger onto VMware certified storage arrays",
            "- Ran a Certified Scrum team through the settlement rewrite",
            "- Deployed onto Kubernetes certified clusters in two regions",
            "- Kept the reporting stack on Oracle certified hardware",
            "- Replaced the Cisco certified network gear in the Frankfurt rack",
        ],
        &[FACTUAL_UNSOURCED_CERTIFICATION],
    );
    // The control: the same issuers, certifying PEOPLE, are still reported.
    assert_credentials_reported(
        &[
            "Docker Certified Associate",
            "Red Hat Certified Engineer",
            "VMware Certified Professional",
            "Certified Scrum Master",
            "Certified Kubernetes Administrator",
            "Oracle Certified Professional",
            "Cisco Certified Network Associate",
        ],
        "certifies a person and must still be reported",
    );
}

/// `engineer` matched as a SUBSTRING of `engineers`, anywhere in 48 characters,
/// so every vendor term from the round before revived with one extra word.
///
/// Mutation check: widen `CERT_ROLE_NOUN_WINDOW_TOKENS`, or drop the
/// `names_a_certified_role` term, and these go red.
#[test]
fn a_vendor_term_is_not_a_credential_however_the_sentence_ends() {
    assert_bullets_raise_nothing(
        &[
            "- Standardised on Docker Certified base images for every engineer",
            "- Led a Certified Scrum team of eight engineers through the migration",
            "- Moved the ledger onto VMware certified storage arrays our engineers own",
            "- Deployed onto Kubernetes certified clusters that two engineers maintain",
            "- Kept the reporting stack on Oracle certified hardware for the engineers",
            "- Replaced the Cisco certified network gear our engineers had outgrown",
            "- Shipped Red Hat certified build images to every engineer on the team",
        ],
        &[
            FACTUAL_UNSOURCED_CERTIFICATION,
            FACTUAL_UNSOURCED_CREDENTIAL,
        ],
    );
    // The control: the role noun sitting where a credential puts it.
    assert_credentials_reported(
        &[
            "Docker Certified Associate",
            "AWS Certified Solutions Architect",
            "Certified Kubernetes Administrator",
            "AWS Certified Security - Specialty",
        ],
        "names the role it certifies and must be reported",
    );
}

/// A derivational noun built on a [`credentials::ROLE_NOUNS`] entry is NOT the
/// role itself: `architecture` contains `architect`, `engineering` contains
/// `engineer`, `expertise` contains `expert` — none of them name a person, and
/// a bare `contains` used to let all three satisfy `names_a_role` on the
/// certification arm's role-noun window, turning an ordinary sentence about a
/// PRODUCT or a PRACTICE into an invented credential.
///
/// Mutation check: revert `role_noun_matches` to `t.contains(role)` and every
/// line here goes red.
#[test]
fn a_derivational_noun_is_not_a_credentialed_role_either() {
    assert_bullets_raise_nothing(
        &[
            "- Wrote the AWS Certified cloud architecture guide for new hires",
            "- Ran the AWS Certified release engineering runbook for the team",
            "- Documented the AWS Certified support expertise wiki for on-call",
        ],
        &[
            FACTUAL_UNSOURCED_CERTIFICATION,
            FACTUAL_UNSOURCED_CREDENTIAL,
        ],
    );
}
