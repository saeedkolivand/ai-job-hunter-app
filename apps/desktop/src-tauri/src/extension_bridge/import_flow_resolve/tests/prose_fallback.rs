//! #1408: the prose-only branch of `looks_like_job` (no JSON-LD, ATS ref, or stem in the
//! title/path). Fixtures are REAL extracted text captured 2026-10 (see `tests/fixtures`):
//! Wikipedia article text as `parse_from_html` returned it, and Greenhouse/Lever posting
//! bodies from their public board APIs, scored here with the ATS ref and title stripped so
//! only the description speaks.

use super::super::*;

use super::super::super::test_support::sample_posting;

const WIKI_SOFTWARE_ENGINEERING: &str = include_str!("fixtures/wikipedia_software_engineering.txt");
const WIKI_TECHNICAL_WRITER: &str = include_str!("fixtures/wikipedia_technical_writer.txt");
const ANTHROPIC_COMPANY: &str = include_str!("fixtures/anthropic_company.txt");
const LEVER_PALANTIR: &str = include_str!("fixtures/lever_palantir_posting.txt");
const GREENHOUSE_ANTHROPIC: &str = include_str!("fixtures/greenhouse_anthropic_posting.txt");
const GREENHOUSE_STRIPE: &str = include_str!("fixtures/greenhouse_stripe_posting.txt");

/// A bare `source: "url"` page with a neutral title and path: the description is the only signal.
fn prose_only(url: &str, desc: &str) -> crate::scraping::types::JobPosting {
    let mut p = sample_posting(url, "Co", "Overview");
    p.source = "url".into();
    p.description = Some(desc.to_string());
    p
}

#[test]
fn refuses_long_encyclopedia_prose_that_mentions_job_words() {
    for (url, text) in [
        (
            "https://en.wikipedia.org/wiki/Software_engineering",
            WIKI_SOFTWARE_ENGINEERING,
        ),
        (
            "https://en.wikipedia.org/wiki/Technical_writer",
            WIKI_TECHNICAL_WRITER,
        ),
    ] {
        // Premise: old rule (>= 300 chars, >= 2 distinct stems) accepted these.
        assert!(text.chars().count() >= MIN_JOB_DESCRIPTION_CHARS);
        assert!(job_stem_stats(&text.to_lowercase()).0.len() >= 2, "{url}");
        assert!(
            !looks_like_job(&prose_only(url, text), None, false),
            "{url}"
        );
    }
}

/// Captured by the extension from anthropic.com/company: dense enough (7 per 1000) and 2+ stems,
/// but no requirement/qualification stem -- the exact text the import stored (#1408).
#[test]
fn refuses_the_anthropic_company_page() {
    let mut p = prose_only("https://anthropic.com/company", ANTHROPIC_COMPANY);
    p.title = "Making AI systems you can rely on".into();
    assert!(!looks_like_job(&p, None, false));
}

#[test]
fn refuses_a_page_over_the_length_cap_even_when_dense() {
    // Dense real posting text repeated past the cap: only the upper bound can refuse it.
    let long = LEVER_PALANTIR.repeat(7);
    assert!(long.chars().count() > MAX_JOB_DESCRIPTION_CHARS);
    assert!(!looks_like_job(
        &prose_only("https://acme.example/p/1", &long),
        None,
        false
    ));
}

#[test]
fn still_accepts_real_postings_without_json_ld_or_ats_ref() {
    for (name, text) in [
        ("lever", LEVER_PALANTIR),
        ("greenhouse anthropic", GREENHOUSE_ANTHROPIC),
        ("greenhouse stripe", GREENHOUSE_STRIPE),
    ] {
        assert!(
            looks_like_job(&prose_only("https://acme.example/p/1", text), None, false),
            "{name}"
        );
    }
}

// Postings with no "requirement"/"qualification" word. English is a custom careers-page ad;
// fr/it/nl/pl are SHORT REALISTIC EXCERPTS written in each language's usual posting vocabulary
// (not captured pages): no public non-ATS posting in those languages was reachable.
const NO_REQUIREMENT_WORD_POSTINGS: &[(&str, &str)] = &[
    (
        "en",
        "We are hiring a Senior Backend Engineer to join our payments team. This open position is \
         remote-friendly and you will own services from design to production. What you'll bring: \
         You have five years building distributed systems in Go or Rust. Must have: strong SQL. \
         Nice to have: Kafka. Experience with on-call rotations helps. Apply with your CV and a \
         short note about a system you are proud of; our careers team replies within a week. \
         We offer a competitive salary, a learning budget and a friendly team that ships weekly.",
    ),
    (
        "fr",
        "Nous recrutons un développeur backend pour rejoindre notre équipe paiements. Ce poste \
         est ouvert à Paris ou en télétravail. Votre profil : cinq ans d'expérience en Rust ou \
         Go, de bonnes bases en SQL et le goût du travail en équipe. Vos missions : concevoir et \
         livrer des services fiables. Postulez dès aujourd'hui avec votre CV, notre équipe \
         recrutement vous répond sous une semaine. Nous offrons un salaire attractif, des \
         formations et une équipe bienveillante qui livre chaque semaine de nouvelles fonctions.",
    ),
    (
        "it",
        "Cerchiamo uno sviluppatore backend per entrare nel nostro team pagamenti. La posizione \
         è aperta a Milano o in remoto. Requisiti: cinque anni di esperienza con Rust o Go e \
         buone basi di SQL. Le tue competenze guideranno la progettazione di servizi affidabili. \
         Candidati oggi con il tuo CV, il nostro team di selezione risponde entro una settimana. \
         Offriamo una retribuzione competitiva, formazione continua e un team accogliente che \
         rilascia nuove funzioni ogni settimana, con un lavoro stimolante ogni giorno.",
    ),
    (
        "nl",
        "Wij zoeken een backend developer die ons betaalteam komt versterken. Deze vacature is \
         beschikbaar in Amsterdam of remote. Jouw profiel: vijf jaar ervaring met Rust of Go en \
         goede kennis van SQL. Solliciteer vandaag nog met je cv, ons recruitmentteam reageert \
         binnen een week. Wij bieden een marktconform salaris, opleidingsbudget en een prettig \
         team dat elke week nieuwe functies uitrolt, met uitdagend werk en veel vrijheid voor \
         iedere nieuwe collega die bij ons komt werken en groeien in een open werksfeer.",
    ),
    (
        "pl",
        "Poszukujemy programisty backend do naszego zespołu płatności. To stanowisko jest dostępne \
         w Warszawie lub zdalnie. Wymagania: pięć lat doświadczenia w Rust lub Go oraz dobra \
         znajomość SQL. Aplikuj już dziś, wysyłając CV, nasz zespół rekrutacja odpowie w ciągu \
         tygodnia. Oferujemy konkurencyjne wynagrodzenie, budżet szkoleniowy i przyjazny zespół, \
         który co tydzień wdraża nowe funkcje, a praca u nas to ciekawe wyzwania każdego dnia i \
         spora swoboda dla każdej nowej osoby w zespole.",
    ),
];

#[test]
fn accepts_real_shaped_postings_with_no_requirement_word() {
    for (lang, text) in NO_REQUIREMENT_WORD_POSTINGS {
        assert!(text.chars().count() >= MIN_JOB_DESCRIPTION_CHARS, "{lang}");
        assert!(
            looks_like_job(
                &prose_only("https://acme.example/open-roles/1", text),
                None,
                false
            ),
            "{lang}"
        );
    }
}
