//! The credential calibration corpus: every document the repo owns that is known to be
//! truthful, the ones carrying a known invention, and the fixtures the tenure and
//! certification tests share.

use super::support::*;

/// The corpus `tests/eval.rs` grades its extraction shape over. Read here too,
/// because "which documents are known-truthful" is the question this
/// calibration is asking, and those two are the only full résumés in the repo
/// that were never written as validator input.
const CORPUS_SWE: &str = include_str!("../../../../tests/corpus/synthetic_swe.txt");

const CORPUS_DESIGNER: &str = include_str!("../../../../tests/corpus/synthetic_designer.txt");

/// A German résumé generated from `en_source_resume.txt` — same employers,
/// same dates, same institution, same eight years, written in the other
/// language. This is the document every value-comparing check has to stay
/// silent on, and the one the repo did not previously own.
const DE_FROM_EN_SOURCE: &str = "Jane Doe\n\
     jane.doe@example.com | +49 30 1234567 | github.com/janedoe\n\n\
     PROFIL\n\n\
     Acht Jahre Erfahrung als Backend-Entwicklerin, überwiegend im Zahlungsverkehr.\n\n\
     BERUFSERFAHRUNG\n\n\
     Senior Backend Engineer | Acme Payments | 2021 - Heute\n\
     - Die Wartezeit an der Kasse von 480ms auf 90ms gesenkt\n\n\
     Backend Developer | Globex Logistics | 2018 - 2021\n\
     - Die Abrechnungsschnittstelle in Python und PostgreSQL gebaut\n\n\
     AUSBILDUNG\n\n\
     BSc Informatik, TU Berlin, 2014 - 2018\n";

/// The same document, except the institution is rendered with its GERMAN city
/// name — the correct translation of an English source's "Technical University
/// of Munich", and the case that decides whether A2c can compare values.
const DE_TRANSLATED_INSTITUTION: &str = "Jana Mustermann\n\n\
     AUSBILDUNG\n\n\
     BSc Informatik, Technische Universität München, 2014 - 2018\n";

const EN_SOURCE_MUNICH: &str = "Jane Doe\n\n\
     EXPERIENCE\n\n\
     Backend Developer | Globex Logistics | 2018 - 2021\n\
     - Built the billing API in Python\n\n\
     EDUCATION\n\n\
     BSc Computer Science, Technical University of Munich, 2014 - 2018\n";

/// A source that actually HOLDS certifications. Without one, a certification
/// false-positive rate of zero would only mean the corpus never mentions a
/// certification — a number that measures the fixtures, not the check.
const EN_SOURCE_CERTIFIED: &str = "Jane Doe\n\n\
     EXPERIENCE\n\n\
     Senior Backend Engineer | Acme Payments | 2021 - Present\n\
     - Ran the settlement platform on Kubernetes\n\n\
     CERTIFICATIONS\n\n\
     AWS Certified Solutions Architect – Associate\n\
     Certified Kubernetes Administrator (CKA)\n";

/// The same certifications, re-worded the way a tailored English résumé would.
const EN_CERT_REWORDED: &str = "Jane Doe\n\n\
     CERTIFICATIONS\n\n\
     AWS Certified Solutions Architect\n\
     CKA\n";

/// …and translated into German, which is the shape that decides whether a
/// certification check can compare values at all.
const DE_CERT_FROM_EN_SOURCE: &str = "Jane Doe\n\n\
     ZERTIFIZIERUNGEN\n\n\
     Zertifizierter AWS Solutions Architect\n\
     Zertifizierter Kubernetes-Administrator (CKA)\n";

/// A Spanish résumé whose current role ends in `Actualidad` — the ordinary
/// Spanish spelling of "Present", which `PRESENT_MARKERS` does not carry.
const ES_SOURCE: &str = "Ana García\n\n\
     EXPERIENCIA\n\n\
     Ingeniera de Backend | Acme Pagos | 2015 - Actualidad\n\
     - Construyó la plataforma de liquidación en Rust\n\n\
     Desarrolladora | Globex | 2011 - 2015\n\
     - Mantuvo la API de facturación en Python\n";

/// Truthful against it: 2011 to today is fifteen years, and it claims fourteen.
const ES_GENERATED: &str = "Ana García\n\n\
     PERFIL\n\n\
     Ingeniera de backend con 14 años de experiencia en pagos.\n\n\
     EXPERIENCIA\n\n\
     Ingeniera de Backend | Acme Pagos | 2015 - Actualidad\n\
     - Construyó la plataforma de liquidación en Rust\n";

/// A French source stating its tenure in a number word outside the en/de table.
const FR_SOURCE: &str = "Camille Dubois\n\n\
     PROFIL\n\n\
     Ingénieure backend avec quinze années d'expérience dans les paiements.\n\n\
     EXPÉRIENCE\n\n\
     Ingénieure Backend | Acme Paiements | 2011 - 2021\n\
     - A construit la plateforme de règlement\n";

/// The same fifteen years, in digits. Truthful; the source says so in words.
const FR_GENERATED: &str = "Camille Dubois\n\n\
     PROFIL\n\n\
     15 années d'expérience en backend, principalement dans les paiements.\n";

/// A two-block employment history whose second heading is outside
/// `classify_section`'s lexicon, so `factual::entries` cannot see its role.
const TWO_BLOCK_SOURCE: &str = "Sam Reed\n\n\
     EXPERIENCE\n\n\
     Principal Engineer | Acme Payments | 2015 - Present\n\
     - Owned the settlement platform end to end\n\n\
     EARLIER ROLES\n\n\
     Developer | Initrode | 2010 - 2015\n\
     - Built the reporting stack in Python\n";

/// Truthful against it: 2010 to today is sixteen years.
const TWO_BLOCK_GENERATED: &str = "Sam Reed\n\n\
     SUMMARY\n\n\
     Principal engineer with 15 years of experience in payments.\n";

/// Ordinary achievements whose verb is "certified" and whose object is an
/// issuer token. Every one of these is a real thing a platform engineer did.
pub(super) const CERT_PROSE: &str = "Jane Doe\n\n\
     EXPERIENCE\n\n\
     Platform Engineer | Acme Payments | 2019 - Present\n\
     - Certified the release on AWS each Thursday before the freeze\n\
     - Migrated 40 services to Docker and certified each image against CIS\n\
     - Ran the Terraform rollout and certified the result with the auditors\n\
     - Coached the Scrum team and certified the runbook with operations\n";

/// A certification named in only ONE of its two forms, in both directions.
const CERT_SOURCE_EXPANSION: &str =
    "Jane Doe\n\nCERTIFICATIONS\n\nCertified Kubernetes Administrator\n";

const CERT_GENERATED_ACRONYM: &str = "Jane Doe\n\nCERTIFICATIONS\n\nCKA\n";

const CERT_SOURCE_ACRONYM: &str = "Jane Doe\n\nCERTIFICATIONS\n\nCKA\nRHCE\nCISSP\n";

const CERT_GENERATED_EXPANSION: &str = "Jane Doe\n\nCERTIFICATIONS\n\n\
     Certified Kubernetes Administrator\n\
     Red Hat Certified Engineer\n\
     Certified Information Systems Security Professional\n";

/// Legacy-system sentences in the SUMMARY, unhyphenated — the register the
/// summary bypass turned into three Criticals.
const LEGACY_IN_SUMMARY: &str = "Jane Doe\n\n\
     SUMMARY\n\n\
     Backend engineer who replaced a 30 year old mainframe, retired 12 years of \
     accumulated schema drift, and cut a 40 year legacy batch to minutes.\n";

/// An education history under a heading the classifier does not know, whose
/// institution carries no marker word either.
const DEGREE_ONLY_SOURCE: &str = "Ravi Menon\n\n\
     EXPERIENCE\n\n\
     Backend Engineer | Acme Payments | 2019 - Present\n\
     - Built the settlement service in Go\n\n\
     QUALIFICATIONS\n\n\
     B.Tech Computer Science, IIT Delhi, 2012 - 2016\n";

/// The same education, written out in full — truthful, and the shape that made
/// the Warning fire.
const DEGREE_ONLY_GENERATED: &str = "Ravi Menon\n\n\
     EDUCATION\n\n\
     B.Tech Computer Science, Indian Institute of Technology Delhi, 2012 - 2016\n";

/// Vendor marketing terms applied to PRODUCTS. Grammatically identical to a
/// credential — issuer, certification word, adjacent — and none of them
/// certifies a person.
const CERT_PROSE_ATTRIBUTIVE: &str = "Jane Doe\n\n\
     EXPERIENCE\n\n\
     Platform Engineer | Acme Payments | 2019 - Present\n\
     - Shipped Docker Certified images to the internal registry\n\
     - Standardised on Red Hat certified build images for the fleet\n\
     - Moved the ledger onto VMware certified storage arrays\n\
     - Ran a Certified Scrum team through the settlement rewrite\n\
     - Deployed onto Kubernetes certified clusters in two regions\n\
     - Kept the reporting stack on Oracle certified hardware\n\
     - Replaced the Cisco certified network gear in the Frankfurt rack\n";

/// A tenure stated in DECADES: no year-word to anchor on, so neither the number
/// table nor the unreadable-quantifier guard used to see it.
const DECADE_SOURCE: &str = "Jane Doe\n\n\
     SUMMARY\n\n\
     Backend engineer with over a decade of experience in payments.\n\n\
     EXPERIENCE\n\n\
     Senior Backend Engineer | Acme Payments | 2019 - 2021\n\
     - Built the settlement platform in Rust\n";

/// Truthful against it — the source states a tenure this file cannot put a
/// number on, so the comparison is unmakeable rather than lost.
const DECADE_GENERATED: &str = "Jane Doe\n\n\
     SUMMARY\n\n\
     Backend engineer with 14 years of experience in payments.\n";

/// A German date column opened with a NUMERIC month. `is_open_ended` wants a
/// year within one word of the opener, so `Seit 03/2016` read as closed at its
/// own start year.
const NUMERIC_MONTH_SOURCE: &str = "Jana Mustermann\n\n\
     BERUFSERFAHRUNG\n\n\
     Senior Backend Engineer | Acme Payments | Seit 03/2016\n\
     - Die Abrechnungsplattform in Rust gebaut\n";

/// Truthful against it: 2016 to today is ten years.
const NUMERIC_MONTH_GENERATED: &str = "Jana Mustermann\n\n\
     PROFIL\n\n\
     10 Jahre Erfahrung im Zahlungsverkehr.\n";

/// `$X per year` is close to the most common quantified-impact phrasing on a
/// résumé, and every bullet here carries a year-word with a quantifier the
/// number table cannot read.
const PER_YEAR_IMPACT_SOURCE: &str = "Jane Doe\n\n\
     SUMMARY\n\n\
     Backend engineer with 3 years of experience in payments.\n\n\
     EXPERIENCE\n\n\
     Backend Engineer | Acme Payments | 2019 - 2021\n\
     - Cut cloud spend by 1.2M USD per year\n\
     - Reported year over year growth of 40% to the board\n\
     - Ran the fiscal year close for two entities\n\
     - Mentored two interns last year\n";

/// Truthful against it.
const PER_YEAR_IMPACT_GENERATED: &str = "Jane Doe\n\n\
     SUMMARY\n\n\
     Backend engineer with 3 years of experience in payments.\n";

/// The same source, with an INFLATED restatement. This is the row that proves
/// the unreadable-tenure guard is a guard and not an off switch: if any of
/// those four bullets silences the check, this invention goes unreported.
const PER_YEAR_IMPACT_INFLATED: &str = "Jane Doe\n\n\
     SUMMARY\n\n\
     Backend engineer with 20 years of experience in payments.\n";

/// Every document in this repo that is KNOWN-TRUTHFUL, as
/// `(label, generated, source)`. A corpus résumé is graded against itself: it
/// is a real document, and every credential in it is by definition sourced.
pub(super) fn truthful_documents() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("en_cert_reworded", EN_CERT_REWORDED, EN_SOURCE_CERTIFIED),
        // The registers the first corpus was missing. Each one produced a
        // reproduced false Critical before the fixes in this commit.
        ("es_actualidad", ES_GENERATED, ES_SOURCE),
        ("fr_quinze_ans", FR_GENERATED, FR_SOURCE),
        ("two_block_history", TWO_BLOCK_GENERATED, TWO_BLOCK_SOURCE),
        ("cert_prose_verb", CERT_PROSE, EN_SOURCE),
        (
            "cert_acronym_from_expansion",
            CERT_GENERATED_ACRONYM,
            CERT_SOURCE_EXPANSION,
        ),
        (
            "cert_expansion_from_acronym",
            CERT_GENERATED_EXPANSION,
            CERT_SOURCE_ACRONYM,
        ),
        ("legacy_system_in_summary", LEGACY_IN_SUMMARY, EN_SOURCE),
        // Round two's registers, each one a reproduced false Critical.
        ("cert_prose_attributive", CERT_PROSE_ATTRIBUTIVE, EN_SOURCE),
        ("decade_tenure", DECADE_GENERATED, DECADE_SOURCE),
        (
            "numeric_month_opener",
            NUMERIC_MONTH_GENERATED,
            NUMERIC_MONTH_SOURCE,
        ),
        (
            "per_year_impact",
            PER_YEAR_IMPACT_GENERATED,
            PER_YEAR_IMPACT_SOURCE,
        ),
        (
            "degree_without_marker",
            DEGREE_ONLY_GENERATED,
            DEGREE_ONLY_SOURCE,
        ),
        (
            "de_cert_from_en_source",
            DE_CERT_FROM_EN_SOURCE,
            EN_SOURCE_CERTIFIED,
        ),
        ("en_generated_clean", EN_CLEAN, EN_SOURCE),
        ("en_generated_paraphrased", EN_PARAPHRASED, EN_SOURCE),
        ("de_generated_clean", DE_CLEAN, DE_SOURCE),
        ("de_generated_paraphrased", DE_PARAPHRASED, DE_SOURCE),
        ("en_generated_projects_tier2", EN_PROJECTS_TIER2, EN_SOURCE),
        ("en_generated_projects_tier3", EN_PROJECTS_TIER3, EN_SOURCE),
        ("en_letter_grounded", EN_LETTER_GROUNDED, EN_SOURCE),
        ("corpus_synthetic_swe", CORPUS_SWE, CORPUS_SWE),
        (
            "corpus_synthetic_designer",
            CORPUS_DESIGNER,
            CORPUS_DESIGNER,
        ),
        ("de_generated_from_en_source", DE_FROM_EN_SOURCE, EN_SOURCE),
        (
            "de_translated_institution",
            DE_TRANSLATED_INSTITUTION,
            EN_SOURCE_MUNICH,
        ),
    ]
}

/// Documents carrying a KNOWN invention, so the calibration reports recall next
/// to the false-positive rate. A check that is silent on everything is not a
/// check.
pub(super) fn invented_documents() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "inflated_years",
            "Jane Doe\n\nSUMMARY\n\nBackend engineer with 15+ years of experience in payments.\n",
            EN_SOURCE,
        ),
        (
            "invented_certification",
            "Jane Doe\n\nCERTIFICATIONS\n\nAWS Certified Solutions Architect – Professional\nPMP\n",
            EN_SOURCE,
        ),
        (
            "inflated_years_beside_per_year_impact",
            PER_YEAR_IMPACT_INFLATED,
            PER_YEAR_IMPACT_SOURCE,
        ),
        (
            "invented_education",
            "Jane Doe\n\nEDUCATION\n\nMSc Computer Science, Stanford University, 2012 - 2014\n",
            "Jane Doe\n\nEXPERIENCE\n\nBackend Developer | Globex | 2018 - 2021\n- Built the billing API\n",
        ),
    ]
}

/// A source with a CLOSED career: two roles, 2016–2020, four years, and it
/// states no tenure of its own. The allowance is therefore a pure date
/// computation and cannot move with the calendar — which is what makes the
/// boundary below an absolute number rather than a value derived from the same
/// code the assertion is checking.
pub(super) const EN_SOURCE_FOUR_YEARS: &str = "Jane Doe\n\n\
     EXPERIENCE\n\n\
     Backend Developer | Globex Logistics | 2018 - 2020\n\
     - Built the billing API in Python and PostgreSQL\n\n\
     Junior Developer | Initrode | 2016 - 2018\n\
     - Maintained the reporting jobs\n";

pub(super) fn summary_claiming(text: &str) -> String {
    format!("Jane Doe\n\nSUMMARY\n\n{text}\n")
}
