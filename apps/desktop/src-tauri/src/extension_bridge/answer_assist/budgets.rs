//! Size/token budgets for `answer.assist` + the compile-time guard pinning
//! the relationship between the two token caps.

/// Byte cap on the incoming question (page/user-derived, untrusted) — roomier
/// than `answers_suggest::MAX_QUESTION_BYTES` (a scanned form LABEL): a
/// pasted/picked application question is a full sentence of prose.
pub(super) const MAX_QUESTION_BYTES: usize = 2_000;

/// Byte cap on the DRAFT-mode free-text instruction (user-typed, untrusted) —
/// the wire carries a REGENERATE click's typed box (and the matched chip text
/// on a rewrite; rewrite's own cap is `answer_rewrite::INSTRUCTION_CAP`). 500
/// mirrors that sibling cap exactly — the only other free-text instruction
/// surface on the bridge — and comfortably holds a single typed sentence,
/// which is the whole product surface of the box. Clamped at the resolve
/// boundary with `clamp_bytes` (UTF-8-safe) and fenced with the same number
/// as a char cap (the identical double-bound pattern `question` uses), so the
/// bound is a hard limit on what is carried, not just on what the fence shows.
pub(in crate::extension_bridge) const MAX_INSTRUCTION_BYTES: usize = 500;

/// Char cap on the fenced company-brief block — the same value the
/// now-deleted `agent::tools`'s own `BRIEF_CAP` used (not exported there
/// either; duplicated here as a tiny local constant rather than widening that
/// module's visibility for one more caller).
pub(super) const BRIEF_CAP: usize = 2_000;

/// Char cap on the fenced opt-in web-search-notes block.
pub(super) const WEB_NOTES_CAP: usize = 2_000;

/// Char cap on the fenced salary-context block (a short "min-max CUR" line).
pub(super) const SALARY_CONTEXT_CAP: usize = 200;

/// Char cap on the produced draft — a coarse guard so a runaway response can't
/// bloat the wire reply; clamped char-boundary safe like every other cap here.
/// Enforced LIVE during streaming (see [`super::super::stream::forward_chunk`]), not
/// just clamped on the terminal string.
pub(in crate::extension_bridge) const DRAFT_CAP: usize = 4_000;

/// Explicit `max_tokens` for the streaming compose call — a cost/latency
/// bound on the provider's own generation, for both draft and rewrite.
///
/// **Not** the wire cap on the visible answer: that is [`DRAFT_CAP`] CHARS,
/// enforced LIVE mid-stream by [`super::super::stream::forward_chunk`] and
/// again on the terminal string by `clamp_chars`. This number only has to be
/// large enough that the model can reach that char cap.
///
/// **Why it is no longer `DRAFT_CAP / 4`** (the old chars≈tokens×4 mirror of
/// the char cap): that silently assumed the whole budget is spent on ANSWER
/// tokens. On a reasoning model it is not — thinking tokens are billed
/// against the same `max_tokens`, and exhausting it ends the stream with
/// `finish_reason: length` and NO answer text. Measured on Ollama Cloud
/// `gpt-oss:20b`: rewrites carrying a length instruction thought 2218–3369
/// chars, exhausting the old budget on 4 of 4 attempts, while short-thinking
/// rewrites always passed — success was a function of how long the model
/// happened to think.
///
/// **Why exactly this number.** Two bounds meet: from below, it must
/// comfortably cover one answer plus normal reasoning (the worst SUCCESSFUL
/// call measured on that replay spent 928 output tokens; this doubles the
/// old budget). From above, `commands::ai_provider::anthropic::
/// build_chat_stream_body` turns classic extended thinking ON for a classic
/// Anthropic model once `max_tokens` crosses its own threshold (and forces
/// `temperature` to 1.0) — the opposite of what this path wants. This value
/// stays under that threshold
/// ([`crate::commands::ai_provider::anthropic::classic_thinking_engages`]),
/// asserted (not just documented) by
/// `tests::the_compose_budget_stays_under_anthropics_classic_thinking_gate`.
///
/// Reasoning-effort helps too — a cheap tier when the provider has one
/// ([`crate::pipeline::Completer::low_effort`]) — and
/// [`super::compose::compose_with_length_retry`] retries once at
/// [`ANSWER_ASSIST_RETRY_MAX_TOKENS`] when a model still thinks past it.
pub(crate) const ANSWER_ASSIST_MAX_TOKENS: u32 = 2_000;

/// The budget the ONE retry in [`super::compose::compose_with_length_retry`]
/// runs at, after a model spent all of [`ANSWER_ASSIST_MAX_TOKENS`] thinking
/// and produced no answer text.
///
/// `DRAFT_CAP` as tokens: ~4x the visible answer's own char cap (the
/// chars≈tokens×4 heuristic) — room for a full-length answer plus a long
/// reasoning pass. It is deliberately allowed to cross the Anthropic
/// classic-thinking threshold [`ANSWER_ASSIST_MAX_TOKENS`] stays under: this
/// attempt exists precisely because the model needs more room to think AND
/// answer. Each attempt gets its own live `DRAFT_CAP` char window (based at
/// its own start), and exactly one retry ever runs, so a request forwards at
/// most 2 × `DRAFT_CAP` chars total.
///
/// That crossing is asserted, not assumed: the same test that pins the first
/// attempt BELOW
/// [`crate::commands::ai_provider::anthropic::classic_thinking_engages`]
/// pins this one ABOVE it, so shrinking this back under the gate fails a
/// test too.
pub(crate) const ANSWER_ASSIST_RETRY_MAX_TOKENS: u32 = DRAFT_CAP as u32;

/// Compile-time guard on the two budgets above — both are relationships
/// BETWEEN constants, so they are checked where they can never drift rather
/// than in a test that has to be remembered:
///
/// * the first attempt must exceed a cap-length answer's OWN token cost
///   (`DRAFT_CAP / 4`, on the chars≈tokens×4 heuristic), or reasoning has
///   nothing left to spend and the empty length cut is back;
/// * the retry must be strictly larger than the attempt it is retrying, or it
///   is not a retry at all.
const _: () = {
    assert!(ANSWER_ASSIST_MAX_TOKENS > (DRAFT_CAP / 4) as u32);
    assert!(ANSWER_ASSIST_RETRY_MAX_TOKENS > ANSWER_ASSIST_MAX_TOKENS);
};
