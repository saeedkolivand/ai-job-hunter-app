//! Per-request HTTP timeouts for the AI provider adapters.
//!
//! Each constant is named by the *operation* it bounds, not by its raw value, so
//! a call site reads as `.timeout(timeouts::COMPLETION)` instead of a bare magic
//! number. Two sites share a constant only when they bound the **same operation**
//! at the **same duration**; same duration + different operation gets its own
//! constant so the values can drift independently later. The one exception is
//! [`STREAM`]: every `chat_stream` call site uses [`stream_deadline`] (a
//! function of the request's reasoning effort) instead of the bare constant —
//! see its doc comment.
//!
//! Values are intentionally identical to the literals they replaced — this module
//! is a pure extraction with no behavior change (except [`stream_deadline`],
//! added later to fix a high-effort stream being killed mid-generation by the
//! flat [`STREAM`] deadline).

use std::time::Duration;

use crate::ipc_contracts::ai_timeouts::{EFFORT_TIMEOUT_MULTIPLIER, STREAM_BASELINE_SECS};

// ── Chat generation ─────────────────────────────────────────────────────────────

/// Streaming chat completion (`chat_stream`): the long-running SSE/JSON stream a
/// cloud provider or the local Ollama daemon emits while generating. This is the
/// BASELINE — every `chat_stream` call site uses [`stream_deadline`] (which
/// scales this by the request's reasoning effort), never this constant
/// directly, so a high-effort generation on a slow model isn't killed
/// mid-stream by a one-size-fits-all cap.
///
/// The value itself is generated from `packages/shared/src/ai-timeouts.ts`
/// (`STREAM_BASELINE_SECS`, via `pnpm gen:ipc` →
/// `ipc_contracts::ai_timeouts`) — the SAME source the renderer's own
/// `computeStreamTimeoutMs` (`renderer/lib/generate/stream-promise.ts`)
/// imports directly, so the two sides can no longer drift independently;
/// `pnpm gen:ipc:check` fails if this constant and the TS source disagree.
pub const STREAM: Duration = Duration::from_secs(STREAM_BASELINE_SECS);

/// Reasoning-effort → [`STREAM`] multiplier. `req.effort` (`AiGenerateRequest`)
/// is a closed, cross-provider vocabulary (see `OPENAI_EFFORT_LEVELS`,
/// `OLLAMA_EFFORT_LEVELS`, `anthropic_effort_levels`, `gemini_effort_levels`) —
/// the underlying table (generated `EFFORT_TIMEOUT_MULTIPLIER`) is the union of
/// every level any adapter currently exposes. "minimal"/"low"/unset/unrecognized
/// get 1.0 (no reason to extend the baseline); everything above that scales up,
/// since a higher reasoning budget is the actual driver of a stream legitimately
/// running long. A new provider that reuses this SAME `effort` vocabulary
/// benefits automatically; one that ever needs a genuinely new tier name already
/// requires touching its own `effort_levels()` to expose it, so extending the
/// shared table at the same time is not a departure from the
/// zero-change-per-provider rule.
///
/// TIER ORDER, because it is not alphabetical and reads wrong at a glance:
/// `minimal < low < medium < high < xhigh < max`. **`max` is the TOP tier, not
/// `xhigh`** — per Anthropic's effort docs (`low | medium | high | xhigh |
/// max`) and OpenAI's `reasoning_effort` (`none | minimal | low | medium |
/// high | xhigh | max`). An earlier version of this table gave `max` a SMALLER
/// multiplier than `xhigh`, so the highest-effort requests got the shortest
/// deadline — the exact failure this function exists to prevent, reserved for
/// the runs most likely to hit it. The generated table
/// (`packages/shared/src/ai-timeouts.ts`) keeps its entries in ascending tier
/// order for the same reason — keep the monotonicity test's array in that same
/// order too, or it will pass while pinning the inversion.
fn effort_multiplier(effort: Option<&str>) -> f64 {
    match effort {
        Some(e) => EFFORT_TIMEOUT_MULTIPLIER
            .iter()
            .find(|(tier, _)| *tier == e)
            .map_or(1.0, |(_, mult)| *mult),
        None => 1.0,
    }
}

/// The actual per-request deadline for `chat_stream`: [`STREAM`] scaled by
/// [`effort_multiplier`]. `reqwest::RequestBuilder::timeout` bounds the WHOLE
/// request (connect through the last streamed byte — see reqwest's own docs),
/// so this is a total deadline, not a per-chunk idle timeout; a per-chunk idle
/// timeout would be architecturally nicer (it wouldn't need effort awareness
/// at all — a stream that's still actively producing text would never be
/// killed, regardless of how long it legitimately runs) but would mean
/// loosening or removing this same total timeout at all 4 call sites AND
/// adding a second timing mechanism inside the shared `stream_response` loop
/// (`stream.rs`) — a materially bigger surface for the same bug. Scaling the
/// existing total deadline is the smaller, fully-tested fix.
pub fn stream_deadline(effort: Option<&str>) -> Duration {
    Duration::from_secs_f64(STREAM.as_secs_f64() * effort_multiplier(effort))
}

/// Non-streaming cloud completion (`complete`): a single full-response call to a
/// cloud provider (OpenAI / Anthropic / Gemini).
pub const COMPLETION: Duration = Duration::from_secs(120);

/// Non-streaming **local** Ollama completion (`complete`): the local daemon can
/// be far slower than a cloud API on first token, so it gets the longer
/// stream-class budget rather than the cloud [`COMPLETION`] bound.
pub const OLLAMA_COMPLETION: Duration = Duration::from_secs(300);

// ── Embeddings ──────────────────────────────────────────────────────────────────

/// Cloud embeddings (`embed`): a single-vector embeddings request to OpenAI or
/// Gemini.
pub const EMBED: Duration = Duration::from_secs(30);

/// Local Ollama embeddings (`/api/embeddings`): the local daemon's embeddings
/// endpoint, bounded tighter than cloud embeddings.
pub const OLLAMA_EMBED: Duration = Duration::from_secs(15);

// ── Company research (provider-native web search) ───────────────────────────────

/// Cloud native web-search research (`research`): the provider's own server-side
/// web-search tool call (OpenAI `web_search`, Anthropic `web_search_20250305`,
/// Gemini `google_search`).
pub const WEB_SEARCH: Duration = Duration::from_secs(25);

/// Ollama Web Search API (`/api/web_search` on ollama.com): the hosted search
/// call that backs the Ollama-family research path.
pub const OLLAMA_WEB_SEARCH: Duration = Duration::from_secs(15);

/// BASELINE for the OUTER bound on a whole research pass — search **plus** the
/// model's synthesis of the results — held by `CompanyResearch::enrich_with` and
/// `SalaryResearch`. Every call site uses [`research_deadline`], never this
/// constant directly, for the same reason [`STREAM`] has [`stream_deadline`]:
/// synthesis is a model call, so its cost scales with reasoning effort.
///
/// This wraps [`WEB_SEARCH`]/[`OLLAMA_WEB_SEARCH`] *and* a completion, so it must
/// exceed their sum or it becomes the binding constraint and the inner bounds
/// (which produce actionable errors) never get to fire. The previous flat 25s
/// was barely above the 25s [`WEB_SEARCH`] bound alone: a 2026-08-08 support
/// bundle shows a reasoning model taking 9.2s for synthesis at idle, and all six
/// research calls in a concurrent batch timing out — every one of those cover
/// letters shipped with no company knowledge at all.
pub const RESEARCH_BASELINE: Duration = Duration::from_secs(90);

/// The actual deadline for one research pass: [`RESEARCH_BASELINE`] scaled by
/// [`effort_multiplier`], exactly as [`stream_deadline`] scales [`STREAM`].
/// Shares the one generated multiplier table, so a new effort tier is picked up
/// here for free.
pub fn research_deadline(effort: Option<&str>) -> Duration {
    Duration::from_secs_f64(RESEARCH_BASELINE.as_secs_f64() * effort_multiplier(effort))
}

// ── Model discovery & health ────────────────────────────────────────────────────

/// Listing models / validating a key (`list_models`, `test_key`): a quick GET to
/// the provider's model catalog or tags endpoint. Applied PER REQUEST.
pub const LIST_MODELS: Duration = Duration::from_secs(10);

/// Cumulative deadline across a PAGINATED `list_models` fetch (Anthropic's
/// `after_id`/Gemini's `pageToken` cursor loop, capped at
/// `MAX_LIST_MODELS_PAGES`) — bounds the WHOLE fetch, not any single request,
/// so a misbehaving/slow provider can't chain up to `MAX_LIST_MODELS_PAGES`
/// individual [`LIST_MODELS`] timeouts into one very long invoke (50 × 10s =
/// 500s, worst case, without this).
pub const LIST_MODELS_TOTAL: Duration = Duration::from_secs(30);

/// Local Ollama reachability probe (`reachable_model`): the fast health check
/// behind the system-health panel, kept short so an unreachable daemon fails fast.
pub const HEALTH: Duration = Duration::from_secs(3);

/// Inspecting a local Ollama model (`/api/show`): fetch a model's trained context
/// length and size labels.
pub const OLLAMA_SHOW: Duration = Duration::from_secs(15);

/// Pulling (downloading) a local Ollama model (`/api/pull`): a large multi-GB
/// download streamed with progress, hence the hour-long ceiling.
pub const MODEL_PULL: Duration = Duration::from_secs(3600);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_deadline_matches_the_baseline_for_no_or_low_effort() {
        assert_eq!(stream_deadline(None), STREAM);
        assert_eq!(stream_deadline(Some("")), STREAM);
        assert_eq!(stream_deadline(Some("minimal")), STREAM);
        assert_eq!(stream_deadline(Some("low")), STREAM);
    }

    #[test]
    fn stream_deadline_scales_up_for_higher_effort() {
        assert_eq!(stream_deadline(Some("medium")), Duration::from_secs(450));
        assert_eq!(stream_deadline(Some("high")), Duration::from_secs(600));
        // `xhigh` then `max` — vendors' ascending order, see `effort_multiplier`.
        assert_eq!(stream_deadline(Some("xhigh")), Duration::from_secs(750));
        assert_eq!(stream_deadline(Some("max")), Duration::from_secs(900));
    }

    /// A caller must be able to trust the ordering, not just the individual
    /// values — this is what "scales with effort" actually promises.
    ///
    /// The array below must stay in the VENDORS' ascending tier order
    /// (`… high < xhigh < max`), not in the order the match arms happen to be
    /// written. A previous version listed `max` before `xhigh`, which made this
    /// test pass against a table that gave the top tier the shortest deadline.
    #[test]
    fn stream_deadline_is_monotonically_nondecreasing_by_effort_tier() {
        let tiers = [
            None,
            Some("minimal"),
            Some("low"),
            Some("medium"),
            Some("high"),
            Some("xhigh"),
            Some("max"),
        ];
        let mut prev = Duration::from_secs(0);
        for effort in tiers {
            let d = stream_deadline(effort);
            assert!(
                d >= prev,
                "stream_deadline({effort:?}) = {d:?} must be >= the previous tier's {prev:?}"
            );
            prev = d;
        }
    }

    /// An effort string outside the known vocabulary must fall back to the
    /// baseline — never explode a typo/future-provider string into an
    /// unbounded multiplier.
    #[test]
    fn stream_deadline_falls_back_to_baseline_for_an_unrecognized_effort_string() {
        assert_eq!(stream_deadline(Some("ultra-mega-think")), STREAM);
    }

    // ── research_deadline ───────────────────────────────────────────────────
    //
    // Same contract as `stream_deadline`, and for the same reason: the thing it
    // bounds ends in a model call. A flat 25s here meant every research call in
    // a reported reasoning-model session timed out, and each cover letter was
    // written with no company knowledge and no visible failure.

    #[test]
    fn research_deadline_exceeds_the_inner_search_bounds_it_wraps() {
        // It wraps a web search AND a synthesis completion. If the outer bound
        // isn't clear of the inner ones, it fires first and the actionable inner
        // error never surfaces. The old flat 25s was EQUAL to `WEB_SEARCH`.
        assert!(
            research_deadline(None) > WEB_SEARCH,
            "the outer research bound must clear the cloud web-search bound"
        );
        assert!(research_deadline(None) > OLLAMA_WEB_SEARCH);
    }

    #[test]
    fn research_deadline_is_monotonically_nondecreasing_by_effort_tier() {
        // Vendors' ascending order — `max` is the TOP tier, above `xhigh`.
        let tiers = [
            None,
            Some("minimal"),
            Some("low"),
            Some("medium"),
            Some("high"),
            Some("xhigh"),
            Some("max"),
        ];
        let mut prev = Duration::from_secs(0);
        for effort in tiers {
            let d = research_deadline(effort);
            assert!(
                d >= prev,
                "research_deadline({effort:?}) = {d:?} must be >= the previous tier's {prev:?}"
            );
            prev = d;
        }
        // Not vacuously true: the top tier must actually exceed the baseline.
        assert!(research_deadline(Some("max")) > research_deadline(None));
    }

    #[test]
    fn research_deadline_falls_back_to_baseline_for_an_unrecognized_effort_string() {
        assert_eq!(research_deadline(Some("ultra-mega-think")), RESEARCH_BASELINE);
        assert_eq!(research_deadline(None), RESEARCH_BASELINE);
    }
}
