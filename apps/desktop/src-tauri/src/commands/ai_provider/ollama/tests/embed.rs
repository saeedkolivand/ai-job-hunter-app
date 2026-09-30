//! `build_ollama_embed_body` — no internal length cap (see its own doc for
//! the silent-truncation regression this guards).

use super::super::embed::build_ollama_embed_body;

#[test]
fn embed_body_sends_a_grown_chunk_in_full_never_silently_truncated() {
    // Regression guard for the reintroduced silent-truncation defect:
    // `bounded_split_cap`'s growth path (`embed.rs`) deliberately sizes a
    // chunk PAST the nominal 8000-char cap for a document that would
    // otherwise need more than `MAX_CHUNKS_PER_DOCUMENT` chunks — a real
    // example is a 300,000-char document / 32 chunks = 9,375 chars each.
    // `embed_with` used to `chars().take(8000)` here and silently drop
    // the last ~1,375 chars of every such chunk with no error. The body
    // builder must send whatever it's given, in full, regardless of
    // length — total chars sent must equal the input length.
    let text = "a".repeat(9_375);
    let body = build_ollama_embed_body("nomic-embed-text", &text);
    assert_eq!(
        body["prompt"].as_str().unwrap().chars().count(),
        9_375,
        "the full grown chunk must reach the provider, never silently truncated"
    );
}
