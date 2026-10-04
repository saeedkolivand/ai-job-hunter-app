//! The present / absent skills split: the display-only function-word filter, the relevance order, and
//! the honest degrade for a language with no curated list.

use super::*;

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

/// The posting used by the ordering test: `kubernetes` ×3, `docker` ×2,
/// `terraform` ×2, everything else exactly once, and no accidental repeat that
/// would join the weighted group.
const WEIGHTED_JOB: &str = "\
Kubernetes is the platform. Kubernetes schedules every Docker workload. \
Kubernetes handles rollout. Docker images come from CI. Terraform provisions \
infrastructure. Terraform manages networking. Ansible configures hosts.";

/// R8 follow-up — both skills lists were sorted ALPHABETICALLY, purely for
/// determinism. Every consumer truncates (the now-deleted `agent::tools_quality`'s
/// `.take(MAX_SKILLS)` was one), so what survived was an alphabetical PREFIX of the gap
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

/// A French posting mixing curated function words the KERNEL now filters
/// (`pour`, `avec`, `nous`, `vous`, …) with inflected verb forms the kernel
/// deliberately leaves as curated-not-exhaustive filler (`cherchons`,
/// `concevez`, `orchestre`) — what a partially-curated vocabulary still looks
/// like in a language `documents::evidence`'s OWN `function_words` map has
/// no list for.
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

    // The residual, pinned rather than hidden: `documents::keywords::STOPWORDS_FR`
    // now curates core French function words (`pour`, `avec`, `nous`, `vous`, …),
    // so those are gone from this list — but it is curated, not exhaustive
    // (deliberately: an inflected verb form is a judgment call left IN the
    // keyword set, same reasoning as `agilen`/`analysierst` in German). An
    // uncurated verb form like "cherchons" ("we search") still leaks through.
    // Only a curated `function_words("fr")` (a DIFFERENT, evidence-module-local
    // list — see that const's doc) can remove it, and adding one re-enables the
    // relevance order in the same edit.
    assert!(
        set.skills_absent.iter().any(|s| s == "cherchons"),
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
