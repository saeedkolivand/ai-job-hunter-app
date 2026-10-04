use super::{support::*, *};

// ── lexical: search_any (QueryMode::Any) ──────────────────────────────────────

/// Three docs whose text overlaps the way a help corpus's does: every one of
/// them contains some of a question's words, only one contains most.
fn question_docs<'a>() -> Vec<LexicalDoc<'a>> {
    vec![
        doc(
            "export",
            "How do I export as PDF or DOCX?",
            "",
            "Press Export above a finished document and choose PDF, DOCX or TXT.",
        ),
        doc(
            "stored",
            "Where do my generated documents go?",
            "",
            "Open Documents in the sidebar; every finished run is saved there.",
        ),
        doc(
            "unrelated",
            "What is Autopilot?",
            "",
            "Autopilot watches a saved search and scores new postings for you.",
        ),
    ]
}

/// Two entries for the stopword checks: one whose own text is function-word heavy
/// ("How does the search box work?"), one about the topic a question asks for.
fn ollama_docs<'a>() -> Vec<LexicalDoc<'a>> {
    vec![
        doc(
            "p1",
            "How does the search box work?",
            "",
            "It filters the list.",
        ),
        doc(
            "p2",
            "Connect Ollama",
            "",
            "Point the app at a local model.",
        ),
    ]
}

/// The whole reason `search_any` exists: a real user question is a SENTENCE,
/// and under the implicit AND every one of its function words has to appear
/// in the document too — which no document does, so the arm returns nothing.
/// Mutation-visible in the strongest way available: the same query, the same
/// index, the two modes, asserted against each other.
#[test]
fn lexical_search_any_answers_a_full_sentence_that_the_implicit_and_returns_nothing_for() {
    let docs = question_docs();
    let index = LexicalIndex::build(&docs).expect("build must succeed");
    let question = "How do I export my resume as a PDF?";

    assert!(
        index
            .search(question, 10)
            .expect("must not error")
            .is_empty(),
        "the AND mode is what this test is contrasted against: if it starts matching, the \
         premise of search_any has changed"
    );
    let any = index.search_any(question, 10, &[]).expect("must not error");
    assert_eq!(
        any.first().map(String::as_str),
        Some("export"),
        "the entry matching the MOST of the question's terms must rank first; got {any:?}"
    );
}

/// `bm25()` over an OR ranks by how many (and how rare) the matched terms
/// are, so the document sharing more of the question outranks one sharing a
/// single common word — the property the help chat's top-3 grounding rests on.
#[test]
fn lexical_search_any_ranks_by_how_many_of_the_questions_terms_matched() {
    let docs = question_docs();
    let index = LexicalIndex::build(&docs).expect("build must succeed");

    // "finished" is in BOTH the export and the stored doc, so this is a
    // ranking rather than a lookup; "documents" and "saved" are only in one.
    let hits = index
        .search_any("Where are my finished documents saved?", 10, &[])
        .expect("must not error");
    assert!(
        hits.len() >= 2,
        "an OR query must match more than the one best entry, or this measures a lookup: \
         {hits:?}"
    );
    assert_eq!(
        hits.first().map(String::as_str),
        Some("stored"),
        "got {hits:?}"
    );
}

/// The same hardening, in both modes — the point of one shared quoting
/// implementation. Every character FTS5 assigns meaning to stays literal, and
/// the NUL byte that genuinely breaks the parser still surfaces as `Err`
/// rather than a silent empty result.
#[test]
fn lexical_search_any_quotes_operators_exactly_like_the_and_mode() {
    let docs = golang_docs();
    let index = LexicalIndex::build(&docs).expect("build must succeed");

    assert_eq!(
        index
            .search_any("-golang", 10, &[])
            .expect("must not error"),
        vec!["p1".to_string()],
        "a leading `-` must be literal text in this mode too, not FTS5's NOT operator"
    );
    for query in [
        "\"unterminated",
        "NEAR",
        "*",
        ":",
        "()",
        "AND",
        "NOT",
        "OR",
        "why does it say \"Also on\"?",
        "!!!",
    ] {
        index
            .search_any(query, 10, &[])
            .unwrap_or_else(|e| panic!("{query:?} must not error, got {e}"));
    }
    assert!(
        index.search_any("\0", 10, &[]).is_err(),
        "an embedded NUL byte must surface as Err in this mode too"
    );
    assert!(index
        .search_any("   ", 10, &[])
        .expect("must not error")
        .is_empty());
}

/// One-character tokens are dropped: "I" and "a" would each add an OR branch
/// matching most of a corpus while carrying no topical signal. A query made
/// ENTIRELY of them still searches, rather than sanitizing to the empty
/// string that `search` reads as "nothing to search for".
#[test]
fn lexical_search_any_drops_one_character_tokens_but_never_every_token() {
    let docs = vec![
        doc("p1", "A", "", "Just the letter a, alone."),
        doc("p2", "Engineer", "", "Kubernetes and Go."),
    ];
    let index = LexicalIndex::build(&docs).expect("build must succeed");

    // "a" is dropped, so only the real term decides the result.
    assert_eq!(
        index
            .search_any("a Kubernetes", 10, &[])
            .expect("must not error"),
        vec!["p2".to_string()],
        "a one-character token must not drag in the document that merely contains it"
    );
    // …but a query with nothing else left keeps its short tokens.
    assert_eq!(
        index.search_any("a", 10, &[]).expect("must not error"),
        vec!["p1".to_string()],
        "an all-short query must still search, not silently return zero hits"
    );
}

/// The other half of that rule: one character is only noise when it is ASCII.
/// A CJK content word is routinely a single character, so dropping by char
/// count alone deleted the only real term in a Chinese or Japanese question —
/// and, since the token that remains here is a normal English one, the
/// all-short fallback never fires to hide it.
///
/// Mutation-visible: drop the `tok.is_ascii() &&` guard in `sanitize_query`
/// and `p1` disappears from the hits below.
#[test]
fn lexical_search_any_keeps_a_one_character_cjk_token() {
    let docs = vec![
        doc("p1", "書", "", "書 alone is a word, not a stray letter."),
        doc("p2", "Engineer", "", "Kubernetes and Go."),
    ];
    let index = LexicalIndex::build(&docs).expect("build must succeed");

    let hits = index
        .search_any("書 Kubernetes", 10, &[])
        .expect("must not error");
    assert!(
        hits.contains(&"p1".to_string()),
        "a one-character CJK token must still contribute an OR branch; got {hits:?}"
    );
    assert!(
        hits.contains(&"p2".to_string()),
        "sanity: the ASCII term must still match its own document; got {hits:?}"
    );
}

/// The trap the drop list is written around: tokens reach `sanitize_query`
/// exactly as typed, so the words a caller passes it ("how", "work") arrive
/// as `"How"` and `"work?"`. Both halves are load-bearing here — the query's
/// first token is capitalised and its last carries punctuation — so the
/// obvious `stopwords.contains(&tok)` fails this test rather than shipping a
/// filter that silently drops nothing.
///
/// Mutation-visible: delete the `.filter(|tok| !is_stopword(…))` line and
/// `p1` reappears; drop the folding inside `is_stopword` and it reappears too.
#[test]
fn lexical_search_any_drops_stopwords_case_and_punctuation_insensitively() {
    let docs = ollama_docs();
    let index = LexicalIndex::build(&docs).expect("build must succeed");
    let question = "How do I connect Ollama so it works?";

    assert!(
        index
            .search_any(question, 10, &[])
            .expect("must not error")
            .contains(&"p1".to_string()),
        "premise: with no drop list the function words alone pull in the unrelated entry"
    );
    assert_eq!(
        index
            .search_any(question, 10, &["how", "do", "so", "it"])
            .expect("must not error"),
        vec!["p2".to_string()],
        "`How` and `works?` must fold to `how`/`works` before the membership check"
    );
}

/// A question made ENTIRELY of function words still searches. Without the
/// fallback the query sanitizes to the empty string, `search_in`
/// short-circuits to `Ok(vec![])`, and `commands::help` reports the arm as
/// `Ran` with zero hits — indistinguishable from an honest miss, and on a
/// default install (`semantic_scoring` off) that is the only arm there is.
///
/// Mutation-visible: remove the `if tokens.is_empty()` fallback and this
/// returns nothing.
#[test]
fn lexical_search_any_falls_back_when_every_token_is_a_stopword() {
    let docs = vec![doc("p1", "What is it?", "", "A question about a thing.")];
    let index = LexicalIndex::build(&docs).expect("build must succeed");

    assert_eq!(
        index
            .search_any("What is it?", 10, &["what", "is", "it"])
            .expect("must not error"),
        vec!["p1".to_string()],
        "an all-stopword question must still search, not silently return zero hits"
    );
}

/// The half the token-list fallback above cannot see: the drop list leaves a
/// NON-empty expression that matches no row. "Where is my stuff?" keeps only
/// `stuff`, which is in no document, so the filtered query returns zero hits
/// and the help arm reports `Ran` with nothing to show — for a question the
/// unfiltered expression answers.
///
/// Mutation-visible: delete the result-set rerun in `search_in` and the last
/// assertion returns an empty list.
#[test]
fn lexical_search_any_reruns_unfiltered_when_the_filtered_query_matched_nothing() {
    let docs = vec![
        doc(
            "p1",
            "Where are my generated documents stored?",
            "",
            "Under the app data directory.",
        ),
        doc(
            "p2",
            "Connect Ollama",
            "",
            "Point the app at a local model.",
        ),
    ];
    let index = LexicalIndex::build(&docs).expect("build must succeed");
    let question = "Where is my stuff?";
    let stopwords = &["where", "is", "my"];

    assert!(
        index
            .search_any(question, 10, &[])
            .expect("must not error")
            .contains(&"p1".to_string()),
        "premise: unfiltered, the question's own function words already reach the entry"
    );
    assert!(
        index
            .search_any("stuff", 10, &[])
            .expect("must not error")
            .is_empty(),
        "premise: `stuff` is in no document, so the FILTERED expression matches no row \
         (a non-empty MATCH the token-list fallback cannot detect)"
    );
    assert_eq!(
        index
            .search_any(question, 10, stopwords)
            .expect("must not error"),
        vec!["p1".to_string()],
        "a drop list may cost ranking, never the whole answer: a filtered query that \
         matched NOTHING must be re-run unfiltered"
    );
}

/// …and the rerun must not fire when the filtered query DID match: re-running
/// unfiltered there would put the function-word branches back and undo the
/// drop list entirely.
///
/// Mutation-visible: change the `!hits.is_empty()` early return in
/// `search_in` to always rerun and `p2` reappears.
#[test]
fn lexical_search_any_does_not_rerun_when_the_filtered_query_matched_something() {
    let docs = ollama_docs();
    let index = LexicalIndex::build(&docs).expect("build must succeed");

    assert_eq!(
        index
            .search_any(
                "How do I connect Ollama so it works?",
                10,
                &["how", "do", "so", "it"]
            )
            .expect("must not error"),
        vec!["p2".to_string()],
        "the filtered query has a hit, so the unfiltered expression (which pulls in p1 on \
         `how`/`do`) must never run"
    );
}
