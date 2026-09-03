//! Deterministic measurement of ONE arm of `commands::help::help_search`:
//! the LEXICAL arm, over the REAL shipped English and German help corpora.
//!
//! **Why only the lexical arm.** It is the arm that runs on every install —
//! `semantic_scoring` defaults to false, so a default install's help search
//! is keyword-only. This file makes NO claim about the dense arm: there is no
//! embedding, no provider and no network anywhere in it. See
//! `tests/lexical_synonym_gaps.rs` for the framing that was rejected
//! ("hybrid finds what keyword misses" conflates two different questions).
//!
//! **Nothing here is a mirror of production.** The corpus is read from the
//! actual `packages/translations/.../en/translation.json` the app ships, and
//! the question→BM25-column mapping and the arm itself are the REAL
//! `commands::help::{to_lexical_doc, run_lexical_arm}`, not a copy — so
//! swapping the column mapping in production makes cases here fail (the
//! mirror shape PR #1091's review rejected in `lexical_synonym_gaps.rs`).
//!
//! **The queries are hand-written user phrasings, not copied questions.**
//! Copying an entry's own title back at it measures nothing — it trivially
//! matches the one document it came from. Each case below is either a short
//! search-box phrasing or a full question someone would really type, and the
//! entry it should reach has to win on BM25 against the other 50.
//!
//! **Two shapes, because the surface has two.** The help page's search box
//! gets 2-4 words; the help CHAT gets a sentence ("How do I export my resume
//! as a PDF?"). The arm ranks both through `run_lexical_arm`, which calls
//! `LexicalIndex::search_any` — OR over the same quoted tokens
//! (`retrieval::lexical::QueryMode`), so `bm25()` ranks by how many of the
//! query's terms an entry matched. Under the implicit AND the postings search
//! box uses, every sentence case below returns ZERO hits, because no help
//! entry contains all of "how", "do", "my", "as" and "PDF" — and on a default
//! install (`semantic_scoring` off) this is the only arm there is.
//!
//! **The bar is top-3, and production's narrowest `limit` is 2.** The two
//! numbers are not the same measurement and this file keeps them apart. Three
//! is `HelpSearchRequestSchema`'s default `limit` and the help chat's default
//! entry budget — the set a default install actually grounds an answer in —
//! so it is the gate. But `resolveHelpChatSizing` halves that budget to
//! [`TOP_N_NARROW`] entries on a small local model, and the same number is
//! passed straight through as the search `limit`, so on that profile a rank-3
//! entry is retrieved and then dropped before the prompt. Retuning a PROMPT
//! budget must not silently redefine what counts as a retrieval regression,
//! so the narrow profile gets a second, INFORMATIONAL column and its own
//! measured floor ([`TOP_2_FLOOR`]) instead of replacing the bar — see that
//! constant for why the prompt budget must not become the gate.
//!
//! **Two corpora, two hand-written case tables.** [`CASES`] measures the
//! English bundle, [`DE_CASES`] the German one. They are not translations of
//! each other: the arm now drops each question's own function words
//! (`commands::help::stopwords`), and which words those are — and what it
//! costs to drop one too many — is a property of the language, not of the
//! feature. The German table carries the over-filtering guard.
//!
//! Run `cargo test --test help_retrieval -- --nocapture` for both tables.

use std::collections::BTreeSet;

use ajh_tauri::commands::help::run_lexical_arm;
use ajh_tauri::ipc_contracts::help::HelpSearchRequestEntry;

/// The shipped English bundle, read at COMPILE time from the real file (the
/// `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), …))` shape
/// `agent_cli::policy`'s own exactness test uses) — never a fixture copy,
/// which could go stale against the corpus this is supposed to measure.
const EN_BUNDLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../packages/translations/src/locales/en/translation.json"
));

/// The shipped German bundle, read the same way. German is not a translation
/// of the English EVAL — it gets its own hand-written cases below, because
/// the retrieval question is different there: German compounds
/// (`zurücksetzen`), FTS5 does no decompounding, and a question's function
/// words are shorter and far more frequent than English's ("ich" is in half
/// the German titles). A stopword list tuned on English and applied to
/// German would ship unmeasured without this.
const DE_BUNDLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../packages/translations/src/locales/de/translation.json"
));

/// The corpus size this file was written against. A hand-set literal, not
/// `entries.len()` compared with itself: if the corpus grows or shrinks, this
/// fails and the cases below get re-checked deliberately rather than the
/// measurement silently changing meaning underneath them.
const EXPECTED_ENTRY_COUNT: usize = 51;

/// How deep a hit counts. Three is the request's own default `limit` — the
/// number of entries the help chat actually grounds an answer in — so this
/// measures the set the feature uses, not a looser one chosen to pass.
const TOP_K: usize = 3;

/// The narrowest `limit` production ever sends. `resolveHelpChatSizing`
/// (`packages/prompts/.../help-chat.ts`) drops the chat's entry budget to 2
/// for small local models, and `use-help-chat` passes that same number
/// through as `help:search`'s `limit` — so on that profile an entry at rank 3
/// is retrieved and then never reaches the prompt.
const TOP_N_NARROW: usize = 2;

/// The bar stays [`TOP_K`], not [`TOP_N_NARROW`], and that is a deliberate
/// choice rather than the looser number: the blocking assertion below is what
/// the DEFAULT profile grounds its answers in, and every case here was
/// hand-written against a 3-deep set. Tightening the gate to 2 would put the
/// small-model profile's prompt budget in charge of what counts as a
/// retrieval regression, which is backwards: shrinking that budget is a
/// PROMPT decision, and it must not silently redefine this eval.
///
/// So top-2 is measured and reported alongside, with its own floor. Every
/// case clears it today (run the test — the table prints both columns), and
/// the number is then written down here BY HAND: it says "this many cases
/// survive the narrow profile today", so a ranking change that quietly pushes
/// a case from rank 2 to rank 3 fails here even while the top-3 gate stays
/// green. A floor, never a target: raise it when the measurement rises, and
/// never lower it to make a change pass — a case that drops out is a finding
/// to judge (which entry took its place, and does that entry answer the
/// question?), the same judgement `CASES`' own `why` column exists for.
///
/// It was 17 before the help arm began dropping the question's own function
/// words (`commands::help::stopwords`); the case it cost was "How do I
/// connect Ollama so the AI features work?", which ranked 3rd behind an
/// entry that shares only the word "work". This obligation runs in one
/// direction: the measurement rose, so the floor rose with it, and the Ollama
/// case can no longer slip back.
const TOP_2_FLOOR: usize = 18;

/// Build the help entries exactly as the renderer does: one entry per
/// `support.faq.<section>Questions.<leaf>` node, `id` the dotted leaf path,
/// `title` the question (`q`), `body` the answer (`a`).
fn corpus() -> Vec<HelpSearchRequestEntry> {
    corpus_from(EN_BUNDLE)
}

/// The German corpus, same ids and same shape (the parity test in
/// `apps/desktop/src/renderer/i18n` is what keeps the two key sets equal).
fn corpus_de() -> Vec<HelpSearchRequestEntry> {
    corpus_from(DE_BUNDLE)
}

fn corpus_from(bundle: &str) -> Vec<HelpSearchRequestEntry> {
    let root: serde_json::Value = serde_json::from_str(bundle).expect("bundle parses");
    let faq = root
        .get("support")
        .and_then(|s| s.get("faq"))
        .and_then(serde_json::Value::as_object)
        .expect("support.faq exists in the bundle");
    let mut entries = Vec::new();
    for (section, node) in faq {
        // Sibling scalar keys (`title`, `subtitle`, each section's label) sit
        // in the same object; only the `*Questions` maps hold entries.
        if !section.ends_with("Questions") {
            continue;
        }
        let Some(questions) = node.as_object() else {
            continue;
        };
        for (leaf, entry) in questions {
            let q = entry.get("q").and_then(serde_json::Value::as_str);
            let a = entry.get("a").and_then(serde_json::Value::as_str);
            let (Some(q), Some(a)) = (q, a) else {
                panic!("{section}.{leaf} is missing `q` or `a`");
            };
            entries.push(HelpSearchRequestEntry {
                id: format!("{section}.{leaf}"),
                title: q.to_string(),
                body: a.to_string(),
            });
        }
    }
    entries
}

/// One hand-written phrasing and the entry it must reach.
struct Case {
    /// What a user types into the help box.
    query: &'static str,
    /// The `support.faq` leaf path that answers it.
    expected: &'static str,
    /// Why this phrasing is the natural one for that entry — recorded so a
    /// later edit can tell "the corpus changed" apart from "the case was
    /// weak".
    why: &'static str,
}

/// Phrasings a user would type, each written before it was run.
///
/// There is no stemmer, so these use the corpus's own vocabulary the way a
/// user would stumble onto it. The first ten are SEARCH-BOX shaped (2-4
/// words); the eight after them are CHAT shaped — whole questions, function
/// words and punctuation included, which is what the help chat sends.
///
/// Every case is CONTESTED: the `contenders` column in the printed table is
/// how many of the 51 entries matched at all, and
/// [`every_case_is_a_ranking_decision_not_a_lookup`] fails if any case had
/// only one. A phrasing that matches exactly one entry proves nothing about
/// ranking — it is a `grep`, and a table full of them would read as a passing
/// retrieval eval while measuring no retrieval at all. The sentence cases are
/// contested by construction (an OR over ~8 tokens matches most of the
/// corpus), which is exactly why their RANK, not their match, is the
/// measurement.
const CASES: &[Case] = &[
    Case {
        query: "default resume",
        expected: "documentsQuestions.multipleResumes",
        why: "the most contested phrasing in the corpus; \"which résumé gets used\" answers it",
    },
    Case {
        query: "tailor resume",
        expected: "aiGenerateQuestions.tailorRun",
        why: "must beat the two getting-started entries, which also talk about tailoring",
    },
    Case {
        query: "save a job",
        expected: "findingJobsQuestions.saveAJob",
        why: "must beat the tracking entry, which describes the same button from the other side",
    },
    Case {
        query: "ai provider",
        expected: "aiSetupQuestions.chooseProvider",
        why: "11 entries mention both words; only the one whose QUESTION is \"which AI provider should I pick\" answers it. The load-bearing case for the column mapping: with `title`/`description` swapped this entry falls to rank 4 and this case fails",
    },
    Case {
        query: "wizard again",
        expected: "gettingStartedQuestions.replayWizard",
        why: "replaying the intro wizard, not the entries that merely mention a wizard step",
    },
    Case {
        query: "resume score",
        expected: "matchScoreQuestions.whatIsScore",
        why:
            "the one case below rank 1 — `saveResumeToScore` outranks it, so top-3 is load-bearing",
    },
    Case {
        query: "export pdf",
        expected: "aiGenerateQuestions.exportDoc",
        why: "must beat `whereStored`, which also names the export formats",
    },
    Case {
        query: "reset everything",
        expected: "generalQuestions.resetEverything",
        why: "must beat the privacy/export entry, which also covers clearing data",
    },
    Case {
        query: "no results linkedin",
        expected: "jobScrapingQuestions.linkedinNoResults",
        why: "the LinkedIn troubleshooting entry, not the board list that also names LinkedIn",
    },
    Case {
        query: "browser window",
        expected: "accountsSessionsQuestions.browserWindowNotOpen",
        why: "the popup-does-not-open entry, not the LinkedIn entry that also opens one",
    },
    // ── Chat-shaped: whole questions, exactly what `use-help-chat` sends ──
    Case {
        query: "How do I export my resume as a PDF?",
        expected: "aiGenerateQuestions.exportDoc",
        why: "the load-bearing case for OR: under the implicit AND no entry contains all of how/do/my/as/PDF, so this returned nothing at all",
    },
    Case {
        query: "How do I connect Ollama so the AI features work?",
        expected: "aiFeaturesQuestions.aiDoesNothing",
        why: "EXPECTATION CORRECTED AFTER MEASURING, and recorded as such: this was first written against `aiSetupQuestions.chooseProvider`, which ranks 6th. Re-reading both answers, the entry that actually answers THIS question — connect Ollama so the AI works — is the troubleshooting one (\"Ollama (Local)\" needs the Ollama server running with a model pulled, plus the wizard's Local (Ollama) tab); chooseProvider answers which provider to pick and where the API key goes. Two defensible siblings, so if a later ranking change swaps them, re-judge rather than assuming a regression",
    },
    Case {
        query: "What data leaves my computer when I use this app?",
        expected: "privacyQuestions.whatLeaves",
        why: "the privacy entry, against a corpus where \"app\" and \"data\" are everywhere",
    },
    Case {
        query: "Why does LinkedIn return no results?",
        expected: "jobScrapingQuestions.linkedinNoResults",
        why: "must beat scrapingZeroJobs and linkedinGuestMode, which share most of these words",
    },
    Case {
        query: "Where are my generated documents stored?",
        expected: "aiGenerateQuestions.whereStored",
        why: "\"generated\" and \"documents\" both appear in half the corpus; only this entry answers where they GO",
    },
    Case {
        query: "How do I pair the browser extension?",
        expected: "extensionQuestions.pairExtension",
        why: "must beat extensionActions and browserWindowNotOpen, both about the browser",
    },
    Case {
        query: "Can I keep more than one résumé?",
        expected: "documentsQuestions.multipleResumes",
        why: "also pins diacritic folding: the query is accented and half the corpus writes \"resume\" both ways",
    },
    Case {
        query: "What does needs review mean?",
        expected: "aiGenerateQuestions.needsReview",
        why: "four other entries define a term with \"means\"; the quoted feature name has to win",
    },
];

/// German phrasings, hand-written against the DE bundle's own vocabulary —
/// not translations of [`CASES`]. Same two shapes (search-box, then whole
/// sentences), same rules: never an entry's own title copied back at it, and
/// every case contested.
///
/// The German list exists because the failure mode is different here. A
/// German question's function words are shorter and far more frequent than
/// English's, and German compounds: FTS5 does no decompounding, so a token
/// dropped by an over-eager stopword list cannot be recovered from the
/// compound that contains it. `Wie setze ich alles zurück?` below is that
/// case, kept deliberately as the over-filtering guard — see its `why`.
const DE_CASES: &[Case] = &[
    Case {
        query: "Lebenslauf anpassen",
        expected: "aiGenerateQuestions.tailorRun",
        why: "\"Lebenslauf\" is in a third of the corpus; only this entry is about ADAPTING one to a posting",
    },
    Case {
        query: "PDF exportieren",
        expected: "aiGenerateQuestions.exportDoc",
        why: "must beat `whereStored` and `importFormats`, which both name file formats too",
    },
    Case {
        query: "Job behalten",
        expected: "findingJobsQuestions.saveAJob",
        why: "\"Job\" is the single most common word in the corpus; \"behalten\" is what separates saving one from tracking an application",
    },
    Case {
        query: "KI-Anbieter",
        expected: "aiSetupQuestions.chooseProvider",
        why: "the entry whose QUESTION is which provider to pick, not the several that mention one in passing",
    },
    Case {
        query: "Match Score bedeutet",
        expected: "matchScoreQuestions.whatIsScore",
        why: "contested by its three sibling score entries (coverage vs match, the save-to-score button, best matches). Written UNHYPHENATED after measuring: `Match-Score` is quoted as one PHRASE, which only the entry whose title spells it that way can match at all — a 1-contender lookup, not a ranking",
    },
    Case {
        query: "Erweiterung koppeln",
        expected: "extensionQuestions.pairExtension",
        why: "must beat `extensionActions`, which is about the SAME extension on a job page",
    },
    Case {
        query: "Autopilot einrichten",
        expected: "autopilotQuestions.setUpAutopilot",
        why: "must beat `whatIsAutopilot`, which shares the distinctive word",
    },
    Case {
        query: "keine Ergebnisse LinkedIn",
        expected: "jobScrapingQuestions.linkedinNoResults",
        why: "the LinkedIn troubleshooting entry, not `linkedinGuestMode` or the board list that also name LinkedIn",
    },
    // ── Chat-shaped: whole questions, exactly what `use-help-chat` sends ──
    Case {
        query: "Wie setze ich alles zurück?",
        expected: "generalQuestions.resetEverything",
        why: "THE over-filtering guard for the German drop list. The entry's title is \"Ich möchte alles zurücksetzen und neu beginnen\", and FTS5 does no decompounding, so `zurück` never matches `zurücksetzen` — `alles` is the token that carries this case. Putting `alles` on `HELP_STOPWORDS_DE` (it looks exactly like a function word) turns this row into a MISS, which is the measurement that keeps that list honest",
    },
    Case {
        query: "Warum liefert LinkedIn keine Ergebnisse?",
        expected: "jobScrapingQuestions.linkedinNoResults",
        why: "the sentence form of the search-box case above, against the same three siblings — a question whose function words (warum/keine) are exactly what the drop list removes",
    },
    Case {
        query: "Wo landen meine fertigen Anschreiben?",
        expected: "aiGenerateQuestions.whereStored",
        why: "\"Anschreiben\" appears in several entries; only this one answers where they GO",
    },
    Case {
        query: "Kann ich mehrere Lebensläufe speichern?",
        expected: "documentsQuestions.multipleResumes",
        why: "must beat `noResumeYet` and `importFormats`, which are also about having a Lebenslauf on file",
    },
    Case {
        query: "Was bedeutet Prüfung nötig?",
        expected: "aiGenerateQuestions.needsReview",
        why: "two other entries open with \"Was bedeutet\"; the quoted feature name has to win",
    },
    Case {
        query: "Wie sichere ich meine Daten?",
        expected: "privacyQuestions.exportImport",
        why: "must beat `whatLeaves`, which is the other entry about the user's Daten",
    },
];

/// The German narrow-limit floor, measured the same way [`TOP_2_FLOOR`] is
/// and read the same way: raise it when the measurement rises, never lower it
/// to make a change pass. Its own number rather than a shared one — the two
/// corpora rank differently and a single floor would let one language's
/// regression hide behind the other's promotion.
const DE_TOP_2_FLOOR: usize = 14;

#[test]
fn the_help_corpus_maps_to_one_entry_per_faq_leaf() {
    let entries = corpus();
    assert_eq!(
        entries.len(),
        EXPECTED_ENTRY_COUNT,
        "the shipped en help corpus changed size — re-check the eval cases below, \
         then update EXPECTED_ENTRY_COUNT deliberately"
    );
    let ids: BTreeSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids.len(), entries.len(), "entry ids must be unique");
    for entry in &entries {
        assert!(
            !entry.title.trim().is_empty(),
            "{} has an empty q",
            entry.id
        );
        assert!(!entry.body.trim().is_empty(), "{} has an empty a", entry.id);
    }
}

/// The corpus↔budget check nothing inside the crate can make. `help_search`
/// embeds at most `HELP_EMBED_MISSES_MAX` cache MISSES per request and reports
/// `dense: "unavailable"` for the whole arm once that budget runs out
/// (all-or-nothing, `commands::help::run_dense_arm`) — so a shipped corpus
/// larger than the cap degrades every cold-cache question to keyword-only,
/// permanently and silently. The constant's own unit tests are written against
/// a synthetic corpus and cannot see that; this file already reads the REAL
/// bundle, so the check belongs here.
///
/// Deliberately `<=`, not `<`: the arm spends one miss per entry, so a corpus
/// exactly at the cap still embeds every entry (`commands::help::test`'s
/// `a_request_at_the_miss_budget_still_runs_the_arm` pins that boundary from
/// the other side).
#[test]
fn the_shipped_corpus_fits_under_the_dense_arms_cache_miss_budget() {
    let entries = corpus();
    assert!(
        entries.len() <= ajh_tauri::commands::help::HELP_EMBED_MISSES_MAX,
        "the shipped en help corpus has {} entries but HELP_EMBED_MISSES_MAX is {}: on a cold \
         cache the dense arm would run out of embed budget and report `unavailable` for every \
         question. Raise HELP_EMBED_MISSES_MAX (read its doc first — it also bounds one \
         request's spend and the permanent rows it writes) rather than editing this test",
        entries.len(),
        ajh_tauri::commands::help::HELP_EMBED_MISSES_MAX,
    );
}

/// One measured case, kept so the whole table can be PRINTED before any
/// assertion fires — a single miss must never hide the other nine rows
/// (`lexical_synonym_gaps`'s own discipline).
struct Row {
    case: &'static Case,
    /// 1-based position of the expected entry within the top [`TOP_K`], or
    /// `None` when it did not appear there.
    rank: Option<usize>,
    /// How many of the 51 entries matched the query AT ALL. `1` means the
    /// case measured a lookup, not a ranking — see [`CASES`]'s doc.
    contenders: usize,
    top: Vec<String>,
    arm_ran: bool,
}

impl Row {
    /// Did the expected entry survive the NARROWEST `limit` production sends
    /// ([`TOP_N_NARROW`])? Informational — the blocking bar is [`TOP_K`].
    fn within_narrow_limit(&self) -> bool {
        self.rank.is_some_and(|r| r <= TOP_N_NARROW)
    }
}

/// `locale` is passed to the REAL arm, not a stopword slice picked here: the
/// locale→drop-list routing (`commands::help::stopwords`) is part of what
/// this file measures, so a mis-mapped locale shows up as a ranking change
/// rather than being papered over by the test choosing the list itself.
fn measure(entries: &[HelpSearchRequestEntry], cases: &'static [Case], locale: &str) -> Vec<Row> {
    let known: BTreeSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    cases
        .iter()
        .map(|case| {
            assert!(
                known.contains(case.expected),
                "case `{}` expects `{}`, which is not in the shipped corpus",
                case.query,
                case.expected
            );
            // The REAL arm, called the way `help_search` calls it (over the
            // whole corpus; `limit` is applied only after fusion).
            let (ranks, status) = run_lexical_arm(entries, case.query, entries.len(), locale);
            let top: Vec<String> = ranks.iter().take(TOP_K).cloned().collect();
            Row {
                case,
                rank: top.iter().position(|id| id == case.expected).map(|i| i + 1),
                contenders: ranks.len(),
                top,
                arm_ran: status == ajh_tauri::commands::hybrid_search::ArmStatus::Ran,
            }
        })
        .collect()
}

fn measure_en() -> Vec<Row> {
    measure(&corpus(), CASES, "en")
}

fn measure_de() -> Vec<Row> {
    measure(&corpus_de(), DE_CASES, "de")
}

/// Print the whole table BEFORE any assertion fires (see [`Row`]), and return
/// `(hits, narrow_hits)` for the caller's own gates.
fn report(locale: &str, rows: &[Row], top_2_floor: usize) -> (usize, usize) {
    println!(
        "\n=== help_retrieval: lexical arm over the real {locale} help corpus ({} entries) ===",
        EXPECTED_ENTRY_COUNT
    );
    println!(
        "{:<52} {:<5} {:<6} {:<11} {:<46} top {TOP_K}",
        "query", "rank", "top-2", "contenders", "expected entry"
    );
    for row in rows {
        println!(
            "{:<52} {:<5} {:<6} {:<11} {:<46} {}",
            row.case.query,
            match row.rank {
                Some(r) => r.to_string(),
                None => "MISS".to_string(),
            },
            if row.within_narrow_limit() {
                "yes"
            } else {
                "-"
            },
            row.contenders,
            row.case.expected,
            row.top.join(", ")
        );
    }
    let hits = rows.iter().filter(|r| r.rank.is_some()).count();
    let narrow_hits = rows.iter().filter(|r| r.within_narrow_limit()).count();
    println!(
        "
{hits} of {} phrasings reach their entry in the lexical top {TOP_K} (the bar)
{narrow_hits} of {} also survive the narrow `limit` of {TOP_N_NARROW} (informational, floor {top_2_floor})
",
        rows.len(),
        rows.len()
    );
    (hits, narrow_hits)
}

/// The gates every locale's table has to clear, in one place so the German
/// eval cannot quietly be held to a weaker bar than the English one.
/// `expected_hits` is the caller's hand-written case count (see the English
/// call site for why it is a literal and not `rows.len()`).
fn assert_table(rows: &[Row], hits: usize, narrow_hits: usize, expected_hits: usize, floor: usize) {
    for row in rows {
        assert!(
            row.arm_ran,
            "the lexical arm reported Unavailable for `{}` — FTS5 itself failed",
            row.case.query
        );
    }
    let missed: Vec<String> = rows
        .iter()
        .filter(|r| r.rank.is_none())
        .map(|r| {
            format!(
                "`{}` (wanted {}: {})",
                r.case.query, r.case.expected, r.case.why
            )
        })
        .collect();
    assert!(
        missed.is_empty(),
        "these phrasings did not reach their entry in the lexical top {TOP_K} (see the table          above for what they DID return): {missed:#?}"
    );
    assert_eq!(
        hits, expected_hits,
        "expected all {expected_hits} hand-written phrasings to hit"
    );
    // The narrow-profile floor (see [`TOP_2_FLOOR`]): a SECOND, weaker gate
    // beside the bar, not a loosening of it. `>=`, not `==`, so a ranking
    // change that promotes a case is not a failure — but a demotion out of
    // the small-model prompt budget is.
    assert!(
        narrow_hits >= floor,
        "only {narrow_hits} of {} phrasings survive the narrow `limit` of {TOP_N_NARROW}, below \
         the measured floor of {floor} — a case fell out of the entry set the small-model \
         help-chat profile actually sends to the prompt (the top-{TOP_K} bar above is still met, \
         which is why this is its own assertion)",
        rows.len()
    );
}

#[test]
fn hand_written_user_phrasings_reach_their_entry_in_the_lexical_top_3() {
    let rows = measure_en();
    let (hits, narrow_hits) = report("en", &rows, TOP_2_FLOOR);
    // Literal, not derived-vs-derived: compared with a hand-written number,
    // so a case quietly deleted from CASES fails here instead of passing.
    assert_table(&rows, hits, narrow_hits, 18, TOP_2_FLOOR);
}

#[test]
fn german_hand_written_user_phrasings_reach_their_entry_in_the_lexical_top_3() {
    let rows = measure_de();
    let (hits, narrow_hits) = report("de", &rows, DE_TOP_2_FLOOR);
    assert_table(&rows, hits, narrow_hits, 14, DE_TOP_2_FLOOR);
}

/// The honesty guard on the tables above: a query that matches exactly ONE of
/// the 51 entries has not been RANKED against anything, so a green row proves
/// only that the words appear in that entry. Every case must have had at
/// least one competitor for the eval to be about retrieval at all.
///
/// It is also the guard that catches OVER-filtering: every stopword removed
/// from a question removes an OR branch, so an over-eager drop list shows up
/// here as contenders collapsing toward 1 long before it shows up as a missed
/// rank.
#[test]
fn every_case_is_a_ranking_decision_not_a_lookup() {
    let weak: Vec<(&str, usize)> = measure_en()
        .iter()
        .chain(measure_de().iter())
        .filter(|r| r.contenders < 2)
        .map(|r| (r.case.query, r.contenders))
        .collect();
    assert!(
        weak.is_empty(),
        "these cases matched fewer than 2 entries, so they measure a lookup rather than a          ranking — replace them with a contested phrasing: {weak:?}"
    );
}

/// The German drop list's own red-on-delete guard.
///
/// The English list is already guarded by [`TOP_2_FLOOR`] — emptying
/// `HELP_STOPWORDS_EN` drops the English measurement from 18 to 17 and that
/// assertion fires. The German one is NOT: with `HELP_STOPWORDS_DE` emptied,
/// every German case above keeps its rank, and only the `contenders` column
/// moves. So the property the German list actually buys — a question's
/// function words stop pulling in half the corpus as OR branches — gets its
/// own assertion here rather than being left to a column nothing checks.
///
/// Both numbers are ABSOLUTE and hand-written from a real run, not one
/// derived value compared with another: a change that broke both directions
/// at once would still fail. The premise (the same question, routed to a
/// locale with no list) is asserted too, so this cannot silently become
/// vacuous if the corpus changes shape.
#[test]
fn the_german_drop_list_narrows_a_german_question_without_costing_the_answer() {
    let entries = corpus_de();
    // A sentence whose content words are `sichere` + `Daten`; everything else
    // in it is a function word.
    let query = "Wie sichere ich meine Daten?";
    let expected = "privacyQuestions.exportImport";

    let (with_list, _) = run_lexical_arm(&entries, query, entries.len(), "de");
    // "xx" is a well-formed tag this module has no list for — the same code
    // path a French install takes, and the honest "no filtering" baseline.
    let (no_list, _) = run_lexical_arm(&entries, query, entries.len(), "xx");

    assert!(
        no_list.len() >= 25,
        "premise: unfiltered, this question's function words must pull in most of the {} \
         entries — got {}. If the corpus changed, re-measure before trusting the numbers below",
        entries.len(),
        no_list.len()
    );
    assert!(
        with_list.len() <= 8,
        "the German drop list must cut this question down to a handful of real contenders — \
         got {}, unfiltered {}",
        with_list.len(),
        no_list.len()
    );
    assert_eq!(
        with_list.first().map(String::as_str),
        Some(expected),
        "…and the narrowing must not cost the answer: the entry that ANSWERS the question \
         still has to rank first"
    );
}

#[test]
fn the_german_help_corpus_maps_to_one_entry_per_faq_leaf() {
    let entries = corpus_de();
    assert_eq!(
        entries.len(),
        EXPECTED_ENTRY_COUNT,
        "the shipped de help corpus changed size — re-check the German eval cases below, \
         then update EXPECTED_ENTRY_COUNT deliberately"
    );
    let en_ids: BTreeSet<String> = corpus().into_iter().map(|e| e.id).collect();
    let de_ids: BTreeSet<String> = entries.iter().map(|e| e.id.clone()).collect();
    assert_eq!(
        en_ids, de_ids,
        "the two bundles must carry the SAME faq leaf ids — the German cases below name ids, \
         and a de-only or en-only leaf would make one eval measure a corpus the other cannot"
    );
}
