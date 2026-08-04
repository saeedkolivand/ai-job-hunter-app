//! Adaptive chunk-and-mean-pool embedding machinery — split out of `mod.rs`
//! (R8 line-budget split: this subsystem alone pushed `mod.rs` from 1076 to
//! 2062 LOC, well past the 1400-line hard cap in `tests/architecture.rs`).
//! Self-contained: everything here exists only to serve
//! [`embed_adaptive`] and its caller, `embed_text` (still in `mod.rs` — the
//! `resolve`/capability chokepoint belongs with the trait/registry surface,
//! not here). `AiProvider` and `Usage` are shared types that stay in the
//! parent module; only [`ProviderEmbedAttempt`] and [`embed_adaptive`] are
//! `pub(super)` so `mod.rs` can construct/call them — everything else here
//! is a private implementation detail.
//!
//! Tests live in the sibling `embed_tests.rs` (same pattern as
//! `anthropic.rs`/`anthropic_tests.rs`) so they stay excluded from R8's LOC
//! cap regardless of how large this subsystem's test suite grows.

use async_trait::async_trait;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};

use super::{AiProvider, Usage};

/// Char-boundary-safe truncation to at most `cap` chars (never splits a
/// multi-byte char). Single pass: `char_indices().nth(cap)` finds the byte
/// offset of the char *at* `cap` (i.e. the first char to drop). `Some` ⇒ the
/// input exceeds `cap` chars, so slice there (a char-boundary offset); `None`
/// ⇒ within cap, use as-is. Pure + unit-tested.
fn truncate_chars(text: &str, cap: usize) -> &str {
    match text.char_indices().nth(cap) {
        Some((byte_offset, _)) => &text[..byte_offset],
        None => text,
    }
}

/// Never truncate an embedding input below this many chars — halving a
/// context-length error down to nothing would still fail (an empty/near-empty
/// embedding is meaningless) and would just multiply retries for no benefit.
const EMBED_TRUNCATION_FLOOR_CHARS: usize = 500;

/// Halve `current` for the next adaptive-truncation retry, clamped at
/// [`EMBED_TRUNCATION_FLOOR_CHARS`]. `None` once `current` is already at or
/// below the floor — the caller gives up rather than retry forever with the
/// same (or a degenerate) length. Pure + unit-tested.
fn next_truncation_len(current: usize) -> Option<usize> {
    if current <= EMBED_TRUNCATION_FLOOR_CHARS {
        None
    } else {
        Some((current / 2).max(EMBED_TRUNCATION_FLOOR_CHARS))
    }
}

/// Whether an error MESSAGE (never a provider-specific error shape) reads as a
/// context-length/input-too-long overflow — provider-agnostic on purpose, so a
/// brand-new provider's own overflow wording is retried with zero code change
/// here. Matches Ollama's `"...exceeds the context length"`, OpenAI's
/// `"...maximum context length..."`/`"...too long..."`, Gemini's
/// `"...exceeds the maximum number of tokens..."`, and the `friendly_api_error`
/// 413 mapping (`"request too large"`). Pure + unit-tested.
///
/// Deliberately narrower than a bare `"too large"`/`"too long"` substring
/// where that would over-match a same-wording RATE-LIMIT message (e.g. a
/// "request too large for tokens-per-minute" 429 body) — `"request too
/// large"`/`"payload too large"` are scoped to the actual over-length
/// wording. This is NOT airtight (a provider could still phrase a rate-limit
/// message with that exact phrase), but it is strictly narrower than the
/// prior bare match and every known real provider wording still matches.
///
/// NOTE: deliberately does NOT attempt to fix this via a per-request
/// `num_ctx`/`options.num_ctx` — Ollama ignores that per request; the model's
/// context has to be baked into a derived model, which is out of scope here.
fn is_context_length_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("context length")
        || m.contains("context_length_exceeded")
        || m.contains("maximum context")
        || m.contains("token limit")
        || m.contains("too many tokens")
        || m.contains("input length exceeds")
        || m.contains("too long")
        || m.contains("exceeds the maximum number of tokens")
        || m.contains("request too large")
        || m.contains("payload too large")
}

/// `AppHandle`-free shape of "embed this (already-truncated) text", so the
/// adaptive-truncation retry loop (`embed_chunk_adaptive`) is unit-testable
/// without a live `tauri::test` mock app — this crate has none (see the same
/// note on `commands::ai::AnswerSearcher`). `embed_text` adapts the real
/// `AiProvider::embed_with_usage` call (which needs `&AppHandle`) to this
/// shape via `ProviderEmbedAttempt`.
#[async_trait]
pub(super) trait EmbedAttempt {
    async fn attempt(&self, text: &str) -> AppResult<(Vec<f64>, Usage)>;
}

pub(super) struct ProviderEmbedAttempt<'a> {
    pub(super) app: &'a AppHandle,
    pub(super) client: &'a dyn AiProvider,
    pub(super) model: &'a str,
}

#[async_trait]
impl EmbedAttempt for ProviderEmbedAttempt<'_> {
    async fn attempt(&self, text: &str) -> AppResult<(Vec<f64>, Usage)> {
        self.client
            .embed_with_usage(self.app, self.model, text)
            .await
    }
}

/// Split `text` into consecutive, char-boundary-safe, non-overlapping chunks
/// of at most `cap` chars each, preserving the WHOLE text (a naive single
/// truncation silently drops everything past `cap` — indistinguishable from a
/// complete embedding once stored, since the space tag is only
/// `{provider, model, dim}`). Empty `text` yields exactly one empty chunk (so
/// the caller still makes its usual single provider call rather than zero).
/// Pure + unit-tested.
fn split_into_chunks(text: &str, cap: usize) -> Vec<&str> {
    if cap == 0 || text.is_empty() {
        return vec![text];
    }
    let mut chunks = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let piece = truncate_chars(rest, cap);
        chunks.push(piece);
        rest = &rest[piece.len()..];
    }
    chunks
}

/// Hard ceiling on the number of TOP-LEVEL chunks a single document is split
/// into. On its own this does NOT bound total provider calls — a document
/// needing several MAX_CHUNKS_PER_DOCUMENT-sized chunks, each independently
/// re-discovering the provider's real per-request limit via
/// `embed_chunk_adaptive`'s halving ladder, is still hundreds to thousands of
/// calls. [`MAX_TOTAL_EMBED_ATTEMPTS`] is the real backstop on call volume;
/// `embed_adaptive` also LEARNS the working cap once (across the whole
/// document, not per chunk — see its own doc comment) so the halving ladder
/// is normally paid only once, not once per chunk. There is no separate
/// spend/budget cap in `crate::spend` for this — the exposure these two
/// constants bound is call VOLUME/latency, not silent cost.
const MAX_CHUNKS_PER_DOCUMENT: usize = 32;

/// Hard ceiling on TOTAL provider calls (success + failure, across every
/// chunk) for a single document's `embed_adaptive`. `MAX_CHUNKS_PER_DOCUMENT`
/// alone bounds only the TOP-LEVEL chunk count, not the retries/sub-splits
/// within them — measured up to ~4224 sequential calls for a 2MB document
/// against a provider whose real limit was far below the nominal cap. This
/// also bounds how long a single document can block its caller:
/// `ai_reembed_all`'s cancellation check runs BETWEEN documents (in
/// `commands::ai`), not within one, so an unbounded single-document embed
/// makes Cancel unresponsive for however long that document takes. Once hit,
/// `embed_adaptive` aborts with a clear error rather than continuing
/// indefinitely — the caller's existing per-document failure handling
/// (`ai_reembed_all` counts it as `failed` and moves on) takes it from there.
const MAX_TOTAL_EMBED_ATTEMPTS: usize = 200;

/// The per-chunk char cap `split_into_chunks` should actually use for `text`:
/// `initial_cap`, UNLESS the document would need more than
/// [`MAX_CHUNKS_PER_DOCUMENT`] chunks at that size — in which case the chunk
/// size is grown just enough to stay within the cap. This NEVER drops text
/// (unlike shrinking the document itself would): a larger chunk that still
/// overflows the provider's real token window is caught and fully covered by
/// `embed_chunk_adaptive`'s per-chunk halving-with-remainder, same as any
/// other chunk — it just does a little more of that work. Pure + unit-tested.
fn bounded_split_cap(total_chars: usize, initial_cap: usize) -> usize {
    let cap = initial_cap.max(1);
    let needed = total_chars.div_ceil(cap);
    if needed <= MAX_CHUNKS_PER_DOCUMENT {
        return cap;
    }
    let grown = total_chars.div_ceil(MAX_CHUNKS_PER_DOCUMENT);
    tracing::warn!(
        "embedding a {total_chars}-char document would need {needed} chunks at {cap} chars \
         each — capping to {MAX_CHUNKS_PER_DOCUMENT} chunks of ~{grown} chars instead \
         (adaptive per-chunk halving still covers the whole document, no text is dropped)"
    );
    grown
}

/// L2-normalize `v` in place (a no-op on an already-zero vector). Cosine
/// similarity is normalization-invariant, so this doesn't change match
/// quality — it keeps a mean-pooled vector on the same footing as a
/// provider's own (typically already-normalized) single-call embedding.
/// NOTE: this makes even a single-chunk embedding's stored VALUES
/// byte-different from what the provider returned (same direction, unit
/// length) — harmless for every current consumer, which all go through the
/// scale-invariant `vector::cosine`, but a future DOT-PRODUCT (not cosine)
/// consumer would silently mix normalized and un-normalized magnitudes if it
/// read a pre-this-change stored row. Pure + unit-tested.
fn l2_normalize(v: &mut [f64]) {
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Embed ONE chunk (already at-or-under `initial_cap` chars) in full,
/// adaptively re-sizing when the provider reports a context-length overflow
/// — the per-chunk fallback for a token-dense language where even a
/// `cap`-sized chunk overflows the model's real token window (a fixed char
/// cap is fine for English but not for German, which tokenizes far denser).
///
/// Critically, a successful length that's SMALLER than the chunk is NOT the
/// end — the unsent remainder is never discarded: once a length succeeds,
/// that same length becomes the working cap for the rest of THIS chunk too
/// (no repeating the failed larger caps), so the chunk's full text is always
/// embedded, possibly as several vectors. `initial_cap` is clamped to the
/// remaining length up front so a short remainder is never "halved" through
/// cap values it was never actually sent at, and both the retry log and the
/// give-up error name the length ACTUALLY sent, not the nominal cap. Gives up
/// with the LAST provider error once truncating further would fall at/below
/// [`EMBED_TRUNCATION_FLOOR_CHARS`], or once `attempts_used` (shared across
/// every chunk of the same document — see `embed_adaptive`) would exceed
/// [`MAX_TOTAL_EMBED_ATTEMPTS`]. Any non-context-length error returns
/// immediately (never retried), abandoning whatever of the chunk is still
/// unsent.
///
/// Returns the FINAL working cap alongside the vectors — the caller carries
/// it forward as the STARTING cap for the next chunk, so the halving ladder
/// is learned once per document, not re-paid by every chunk (see
/// `embed_adaptive`).
///
/// `learned` (the returned/carried-forward value) is DELIBERATELY a
/// SEPARATE variable from the per-attempt `cap` used to size each actual
/// send. Only a genuine provider context-length REJECTION lowers `learned`;
/// clamping `cap` down to whatever's left in a short final remainder (e.g.
/// the chunk's last 200 chars) must NEVER also shrink `learned` — that was
/// the bug: on a doc long enough to hit `bounded_split_cap`'s growth path,
/// the tiny leftover tail of chunk N would poison the cap chunk N+1 starts
/// at, permanently collapsing the "learned once" optimization into a
/// near-worst-case per-piece ladder for the REST of the document (measured:
/// a 300k-char document degraded to 1-char sends and hit
/// `MAX_TOTAL_EMBED_ATTEMPTS` with barely 3% actually embedded).
///
/// `usage` is an OUTPUT parameter, not part of the return value, because it
/// must accumulate a REAL provider-reported token count for every call that
/// actually succeeded even when a LATER attempt in this same chunk fails —
/// an early `return Err(..)` (the attempt ceiling, a non-context-length
/// error, or the length-floor give-up) must never silently discard the
/// spend already billed by the provider for the pieces that DID succeed.
async fn embed_chunk_adaptive<A: EmbedAttempt>(
    attempt: &A,
    chunk: &str,
    initial_cap: usize,
    attempts_used: &mut usize,
    usage: &mut Usage,
) -> AppResult<(Vec<Vec<f64>>, usize)> {
    let mut vectors = Vec::new();
    let mut rest = chunk;
    let mut learned = initial_cap.min(chunk.chars().count());
    // `while first_pass || !rest.is_empty()`: an empty chunk (the whole
    // document was empty) still makes exactly ONE provider call, matching
    // every other caller's single-attempt behavior for empty input — it does
    // NOT silently make zero calls just because `rest` starts empty.
    let mut first_pass = true;
    while first_pass || !rest.is_empty() {
        first_pass = false;
        // The length to actually send THIS pass — bounded by what's left in
        // `rest` and by the best-known working size (`learned`). This local
        // clamp must never feed back into `learned` itself (see doc comment).
        let mut cap = learned.min(rest.chars().count());
        loop {
            if *attempts_used >= MAX_TOTAL_EMBED_ATTEMPTS {
                return Err(AppError::Provider(format!(
                    "embedding this document needed more than {MAX_TOTAL_EMBED_ATTEMPTS} \
                     provider calls (context-length retries + chunk splits) — aborting rather \
                     than continuing indefinitely"
                )));
            }
            let truncated = truncate_chars(rest, cap);
            let sent_len = truncated.chars().count();
            *attempts_used += 1;
            match attempt.attempt(truncated).await {
                Ok((values, piece_usage)) => {
                    usage.input_tokens =
                        usage.input_tokens.saturating_add(piece_usage.input_tokens);
                    usage.output_tokens = usage
                        .output_tokens
                        .saturating_add(piece_usage.output_tokens);
                    vectors.push(values);
                    rest = &rest[truncated.len()..];
                    break;
                }
                Err(e) if is_context_length_error(&e.to_string()) => {
                    match next_truncation_len(cap) {
                        Some(smaller) => {
                            tracing::warn!(
                                "embed context-length error at {sent_len} chars — retrying at {smaller} chars"
                            );
                            cap = smaller;
                            // A genuine discovery — this is the one place
                            // `learned` may legitimately shrink.
                            learned = smaller;
                        }
                        None => {
                            return Err(AppError::Provider(format!(
                                "embedding input is too long for this model even after truncating to \
                                 {sent_len} characters: {e}"
                            )));
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }
    Ok((vectors, learned))
}

/// Embed the FULL `text` — chunk it at `initial_cap` chars
/// ([`split_into_chunks`], bounded to at most [`MAX_CHUNKS_PER_DOCUMENT`] top-
/// level chunks by [`bounded_split_cap`]), embed every chunk in full
/// ([`embed_chunk_adaptive`] never drops a chunk's unsent remainder even
/// when it has to sub-split on a context-length error), then mean-pool +
/// L2-normalize every resulting vector into one ([`l2_normalize`]). This is
/// what makes re-indexing self-tuning per document/language AND lossless: a
/// naive single truncation silently drops everything past the cap while
/// still tagging the result as a complete embedding (the space is only
/// `{provider, model, dim}` — nothing marks a truncated vector as partial),
/// so a long résumé would be indexed on roughly its first quarter with no
/// way to tell. Usage is the REAL sum across every provider call actually
/// made (chunking/sub-splitting a document legitimately costs proportionally
/// more tokens than embedding it whole).
///
/// The working cap is LEARNED ONCE across the whole document, not
/// re-discovered per chunk: `embed_chunk_adaptive` returns the cap it ended
/// up succeeding at, and that becomes the STARTING cap for the next
/// top-level chunk. Without this, a document needing `MAX_CHUNKS_PER_DOCUMENT`
/// grown chunks would pay the full halving-ladder discovery cost on EVERY
/// chunk — measured ~224 wholly wasted failed round-trips for a 2MB document
/// against a provider whose real limit was far below the nominal cap; with
/// this, only the first chunk pays that discovery cost. `attempts` is the
/// running TOTAL across every chunk, bounded by [`MAX_TOTAL_EMBED_ATTEMPTS`].
///
/// `usage` is an OUTPUT parameter (see `embed_chunk_adaptive`'s doc comment)
/// so a failure partway through a multi-chunk document — the attempt
/// ceiling, or ANY error on chunk 5 of 8 — still leaves the REAL usage
/// billed for chunks 1-4 available to the caller (`embed_text`) to record.
/// Before this, `embed_text`'s `.await?` on the old `AppResult<(Vec<f64>,
/// Usage)>` return discarded the whole `Usage` on any error, silently
/// dropping already-billed spend from the ledger — a cost-VISIBILITY
/// regression versus the pre-chunking code (which made exactly one call, so
/// a failure billed nothing to begin with).
pub(super) async fn embed_adaptive<A: EmbedAttempt>(
    attempt: &A,
    text: &str,
    initial_cap: usize,
    usage: &mut Usage,
) -> AppResult<Vec<f64>> {
    let split_cap = bounded_split_cap(text.chars().count(), initial_cap);
    let chunks = split_into_chunks(text, split_cap);
    let mut pooled: Vec<f64> = Vec::new();
    let mut expected_dim: Option<usize> = None;
    let mut piece_count: usize = 0;
    let mut attempts: usize = 0;
    let mut learned_cap = split_cap;
    for chunk in chunks {
        let (pieces, cap_used) =
            embed_chunk_adaptive(attempt, chunk, learned_cap, &mut attempts, usage).await?;
        learned_cap = cap_used;
        for values in pieces {
            piece_count += 1;
            match expected_dim {
                None => {
                    expected_dim = Some(values.len());
                    pooled = values;
                }
                Some(d) if d == values.len() => {
                    for (p, v) in pooled.iter_mut().zip(values.iter()) {
                        *p += v;
                    }
                }
                Some(_) => {
                    // Same model/provider must always yield the same
                    // dimension — a mismatch means the provider itself is
                    // inconsistent, not something averaging can paper over.
                    return Err(AppError::Provider(
                        "embedding dimension changed between chunks of the same document"
                            .to_string(),
                    ));
                }
            }
        }
    }
    // Mean-pool (a single piece is already its own mean — skip the no-op
    // divide-by-one), then L2-normalize so a multi-piece pooled vector sits
    // on the same footing as a provider's own single-call embedding.
    if piece_count > 1 {
        let n = piece_count as f64;
        for p in pooled.iter_mut() {
            *p /= n;
        }
    }
    l2_normalize(&mut pooled);
    Ok(pooled)
}

#[cfg(test)]
#[path = "embed_tests.rs"]
mod tests;
