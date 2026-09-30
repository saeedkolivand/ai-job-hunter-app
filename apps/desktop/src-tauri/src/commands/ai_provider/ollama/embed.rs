//! Ollama `/api/embeddings` transport. Split out of `ollama.rs` (R8 LOC cap)
//! — a pure move.

use serde_json::{json, Value};

use crate::error::AppResult;

use super::super::timeouts;
use super::host;

/// Deliberately sends `text` AS GIVEN — no internal length cap. This used to
/// `chars().take(8000)` here "defensively", reasoning that the shared
/// `embed_text` path already caps to `max_embedding_input_chars` (8000 for
/// Ollama) so it would be a no-op. That stopped being true once
/// `bounded_split_cap` (`embed.rs`) started deliberately GROWING a chunk past
/// 8000 for a document needing more than `MAX_CHUNKS_PER_DOCUMENT` chunks at
/// the nominal size — a grown ~9,375-char chunk arrived here and this cap
/// silently dropped its last ~1,375 chars, no error, every time: the exact
/// silent-truncation defect this whole adaptive-embedding feature exists to
/// fix, reintroduced one layer down. `embed_with` has exactly one caller
/// (`OllamaClient::embed`, reached only through `embed_adaptive`'s chunking),
/// so there is no direct caller to protect — length policy belongs entirely
/// to `embed_adaptive`'s chunk-and-halve ladder now: if Ollama's real request
/// limit is smaller than what's sent, it rejects with a real error
/// (`is_context_length_error` already recognizes Ollama's wording), which
/// `embed_chunk_adaptive` retries at a smaller size — the ladder doing
/// exactly what it was built for, instead of this cap quietly mutating the
/// caller's input first.
/// Build the `/api/embeddings` request body. Pure + unit-tested — the ONLY
/// place `prompt` is set, so a regression that reintroduces a length cap
/// here (see `embed_with`'s doc comment) is caught at the body-construction
/// level, the same way `gemini::build_embed_body`/`openai`'s chat body
/// builders are — no HTTP mock needed to prove `text` reaches the wire whole.
pub(super) fn build_ollama_embed_body(model: &str, text: &str) -> Value {
    json!({ "model": model, "prompt": text, "keep_alive": crate::performance::ollama_keep_alive() })
}

pub async fn embed_with(model: &str, text: &str) -> AppResult<Vec<f64>> {
    // See `local_chat`'s module doc: wait briefly (bounded) for an
    // in-flight local chat to clear the daemon before dispatching, so the
    // request below gets a genuinely full timeout window instead of one
    // already half-spent queueing behind chat.
    super::local_chat::wait_for_quiet(timeouts::OLLAMA_EMBED_QUIET_WAIT).await;
    // Read AFTER the wait, right before dispatching: whether chat is STILL
    // running now is what actually explains a timeout that follows.
    let was_busy = super::local_chat::is_chat_in_flight();

    let body = build_ollama_embed_body(model, text);
    let endpoint = format!("{}/api/embeddings", host());
    // `send_embed_with_retry`, not `send_with_retry`: the per-attempt bound and
    // the sequence budget are different values here, so a first attempt that
    // times out while Ollama COLD-LOADS the embedding model still gets a second
    // one (the first embed of an indexing run is exactly that case).
    let resp = super::super::retry::send_embed_with_retry(
        || crate::net::http::shared().post(&endpoint).json(&body),
        timeouts::OLLAMA_EMBED,
    )
    .await
    // A real `OLLAMA_EMBED` timeout (Ollama up but busy — a cold model load is
    // the ordinary case) used to be reported identically to a genuine
    // connection failure ("Ollama unreachable"), which sent two separate
    // investigations to the wrong root cause. Distinguish them — and, when
    // `was_busy`, distinguish "the daemon is busy with a local chat" too.
    .map_err(|e| {
        super::local_chat::map_embed_transport_error(e, timeouts::OLLAMA_EMBED, was_busy)
    })?;
    let status = resp.status();
    if !status.is_success() {
        let body_text =
            crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
        return Err(crate::error::AppError::Provider(format!(
            "Ollama {status}: {body_text}"
        )));
    }
    let data: Value =
        crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
            .await
            .map_err(|e| format!("Ollama parse: {e}"))?;
    let arr = data
        .get("embedding")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "Ollama: missing embedding in response".to_string())?;
    Ok(arr.iter().filter_map(|v| v.as_f64()).collect())
}
