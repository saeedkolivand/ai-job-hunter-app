//! Compose, with one retry for the reasoning-ate-the-budget failure —
//! [`DraftComposer`]/[`compose_with_length_retry`]/[`BridgeComposeRound`].

use crate::error::AppResult;

use super::errors::to_draft_failed;

/// Charge the daily provider budget for the compose call. Never touches the
/// registry itself — `handle_answer_assist` is the SOLE unregister owner, so
/// a rejected charge here is just another `Err` that caller cleans up once,
/// at its single return point. Takes a plain `&Limiter` (no `AppHandle`), so
/// this is directly unit-testable.
pub(super) fn charge_compose_budget(
    limiter: &crate::limits::Limiter,
    provider_id: &str,
) -> AppResult<()> {
    limiter
        .charge_provider_daily(provider_id, crate::limits::PROVIDER_DAILY_MAX)
        .map_err(|e| to_draft_failed("daily budget exceeded before compose", e))
}

/// ONE billable compose round-trip — the charge and the stream, as a pair,
/// because [`compose_with_length_retry`] may make TWO of them and each one
/// must pay the daily ceiling.
///
/// A trait (rather than the concrete [`BridgeComposeRound`] below) purely so
/// the retry decision is unit-testable against a fake round: the real one
/// bottoms out in `stream::compose_draft_stream`, which needs a live
/// `AppHandle` + Tauri event loop this crate has no mock-app harness for.
pub(super) trait DraftComposer {
    /// Charge the per-provider daily ceiling. Called once per attempt,
    /// BEFORE it — never once per request.
    fn charge(&self) -> AppResult<()>;

    /// Whether this request's registry entry is still held by this request —
    /// checked between the two attempts, BEFORE the second charge, so a
    /// cancelled/disconnected request never pays for a retry nobody reads.
    /// See [`super::super::stream::ComposeStream::still_registered`].
    fn still_wanted(&self) -> bool;

    /// The answer text forwarded to the client for this REQUEST so far,
    /// across every attempt ([`super::super::stream::ComposeStream::forwarded`]).
    /// Append-only, which is what lets [`compose_attempts`] snapshot a length
    /// before an attempt and read back exactly that attempt's own text after.
    fn drafted(&self) -> &str;

    /// Stream one compose attempt, appending its visible text to
    /// [`Self::drafted`] and forwarding it live under a `DRAFT_CAP` window
    /// based at `cap_base` — [`compose_attempts`]'s pre-call snapshot of
    /// [`Self::drafted`]`.len()`, so each attempt is capped on ITS OWN text
    /// and [`attempt_text`] can slice the result back out at that offset.
    /// Returns no text of its own — only [`compose_attempts`] knows which
    /// attempt SUCCEEDED. The error is the provider's own (unmapped).
    async fn compose(
        &mut self,
        max_tokens: u32,
        effort: Option<&str>,
        cap_base: usize,
    ) -> AppResult<()>;

    /// Emit the ONE terminal `assist.done` frame this request owes its
    /// client, exactly once, at [`compose_with_length_retry`]'s single exit.
    async fn finish(&mut self);
}

/// Compose once; on EXACTLY the empty-answer length cut
/// ([`crate::commands::ai_provider::stream::is_empty_answer_length_cut`] — the
/// model spent its whole output budget reasoning and the provider ended the
/// stream with `finish_reason: length` and no answer text), compose a SECOND
/// time at `retry_max_tokens`, at the same already-cheapest effort tier.
/// Every other failure surfaces immediately: a retry is real, billable spend.
///
/// Three things stay per REQUEST rather than per attempt, all via this
/// function's single-exit shape: the terminal `assist.done` (emitted once,
/// here, on both outcomes — the popup deletes its `assist.chunk` listener on
/// that frame, so one per attempt would silently discard the retry's chunks);
/// the registry entry (each attempt rebinds the SAME entry/generation rather
/// than minting a new one, so `handle_answer_assist`'s single `unregister_gen`
/// still frees it); and the draft buffer
/// ([`super::super::stream::ComposeStream::forwarded`], appended to by both
/// attempts so [`attempt_text`] slices back only the SUCCEEDED attempt's own
/// tail — each attempt's `DRAFT_CAP` window is based at its own start offset,
/// so a retry is never clamped by what a failed attempt already spent; the
/// wire stays bounded at at most 2× `DRAFT_CAP`).
///
/// Spend discipline: the first charge is taken OUTSIDE the attempt block, so
/// a request the daily ceiling refuses outright never emits a terminal frame
/// for a stream that never ran. The retry pays through the SAME charge, only
/// after [`DraftComposer::still_wanted`] confirms the client is still there.
///
/// Both attempts are logged at WARN naming the retry, so the desktop log
/// tells a retried failure apart from a first-try one (the wire only ever
/// carries the fixed `DRAFT_FAILED_MESSAGE`).
pub(super) async fn compose_with_length_retry<C: DraftComposer>(
    round: &mut C,
    max_tokens: u32,
    retry_max_tokens: u32,
    effort: Option<&str>,
) -> AppResult<String> {
    round.charge()?;
    let outcome = compose_attempts(round, max_tokens, retry_max_tokens, effort).await;
    round.finish().await;
    outcome
}

/// [`compose_with_length_retry`]'s attempt sequence, split out so that
/// function has ONE exit to emit the terminal frame at — every `?` and early
/// return in here still runs it. The first charge is the caller's (see its
/// doc); the retry's is taken here, because only a retry that actually
/// happens may cost anything.
pub(super) async fn compose_attempts<C: DraftComposer>(
    round: &mut C,
    max_tokens: u32,
    retry_max_tokens: u32,
    effort: Option<&str>,
) -> AppResult<String> {
    let before_first = round.drafted().len();
    let first = match round.compose(max_tokens, effort, before_first).await {
        Ok(()) => return Ok(attempt_text(round.drafted(), before_first)),
        Err(e) => e,
    };
    if !crate::commands::ai_provider::stream::is_empty_answer_length_cut(&first) {
        return Err(to_draft_failed("compose failed", first));
    }
    // The client can give up in the window between the two attempts — an
    // `assist.cancel`, or the whole connection dropping (`cancel_all`). Both
    // take this request's registry entry away, and starting a second billable
    // generation for an answer nobody will read is exactly the spend this
    // check exists to refuse.
    if !round.still_wanted() {
        return Err(to_draft_failed(
            "compose failed and the request was cancelled before the retry",
            first,
        ));
    }

    tracing::warn!("answer_assist: retrying after an empty length cut");
    round.charge()?;
    // This snapshot is the retry's cap window AND its result slice: attempt
    // 1's forwarded prose is already spent on the wire, but it was NOT this
    // answer, so it may neither shrink it nor ride back with it.
    let before_retry = round.drafted().len();
    round
        .compose(retry_max_tokens, effort, before_retry)
        .await
        .map_err(|e| to_draft_failed("compose failed on the retry after an empty length cut", e))?;
    Ok(attempt_text(round.drafted(), before_retry))
}

/// The text ONE attempt appended to the request-wide draft buffer: everything
/// in `drafted` past the length it had before that attempt ran.
///
/// Why the retry shares a buffer but not a RESULT: a first attempt can
/// forward visible text and STILL end as the empty length cut that triggers
/// the retry — a local model that spells its reasoning as ordinary inline
/// `<think>` prose emits it as non-thinking deltas, so `stream::forward_chunk`
/// forwards it while the provider's own answer accumulator strips it back to
/// empty. Returning the whole buffer would hand the popup that discarded
/// reasoning CONCATENATED with the retry's answer, and "Accept" pastes it
/// into a real form field — so only the successful attempt's own tail is
/// returned (`start` is also that attempt's live cap window, for the same
/// reason: text that is not part of the answer must neither ride along with
/// it nor eat its budget).
///
/// `start` came from `drafted().len()` on the same append-only buffer, so it
/// is always a char boundary; `unwrap_or_default` is the safe direction if
/// that ever stops being true (an empty draft, never someone else's text).
pub(super) fn attempt_text(drafted: &str, start: usize) -> String {
    drafted.get(start..).unwrap_or_default().to_string()
}

/// The production [`DraftComposer`]: the real daily-ceiling charge and
/// streaming compose, over one already-resolved request's inputs
/// ([`super::super::stream::ComposeStream`], which owns everything the two
/// attempts share).
pub(super) struct BridgeComposeRound<'a> {
    pub(super) stream: super::super::stream::ComposeStream<'a>,
    pub(super) limiter: &'a crate::limits::Limiter,
    pub(super) provider_id: &'a str,
}

impl DraftComposer for BridgeComposeRound<'_> {
    fn charge(&self) -> AppResult<()> {
        charge_compose_budget(self.limiter, self.provider_id)
    }

    fn still_wanted(&self) -> bool {
        self.stream.still_registered()
    }

    fn drafted(&self) -> &str {
        &self.stream.forwarded
    }

    async fn compose(
        &mut self,
        max_tokens: u32,
        effort: Option<&str>,
        cap_base: usize,
    ) -> AppResult<()> {
        super::super::stream::compose_draft_stream(&mut self.stream, max_tokens, effort, cap_base)
            .await
    }

    async fn finish(&mut self) {
        self.stream.send_done().await;
    }
}
