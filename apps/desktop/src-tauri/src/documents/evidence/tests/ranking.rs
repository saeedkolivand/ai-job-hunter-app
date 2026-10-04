//! `rank_bullets`: the weakest-first ordering the trim panel cuts from.

use std::collections::HashSet;

use super::*;
use crate::documents::keywords::{detect_locale_tag, languages_align};

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

/// The filter must stay OUT of the scored bullet path: `hits` (and therefore
/// `score`) drives the trim panel's wire payload and its ranking, and the
/// scoring kernel owns that vocabulary — a SEPARATE axis from this module's
/// own `FUNCTION_WORDS_DE` (the display-only `skills_present`/`skills_absent`
/// filter). "profil" is the probe: it's in `FUNCTION_WORDS_DE` (so it would
/// never appear in a `skills_absent`/`skills_present` list) but NOT in the
/// kernel's own `STOPWORDS_DE` (so it survives as a normal keyword the kernel
/// matches on) — proving the two filters are independent. "unsere" no longer
/// works as the probe: `STOPWORDS_DE` now curates it too (this fix's whole
/// point), so it is correctly ABSENT from `hits` regardless of this module's
/// filter ever touching the bullet path.
#[test]
fn the_function_word_filter_does_not_touch_bullet_scores() {
    let job = "Wir suchen eine erfahrene Entwicklerin mit einem Profil für Kubernetes \
               und Docker.";
    let resume = "BERUFSERFAHRUNG\n\n\
                  Senior Engineer | Acme | 2021 - Heute\n\
                  - Profil mit Kubernetes betrieben\n";
    let ranked = rank_bullets(resume, job);
    assert!(
        ranked[0].hits.iter().any(|h| h == "profil"),
        "bullet hits come from the scoring kernel and must be left alone; got {:?}",
        ranked[0].hits
    );
}
