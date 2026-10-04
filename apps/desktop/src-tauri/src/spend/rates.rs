//! The static list-price table behind [`estimate_cost`]: which calls are free,
//! which `RATES` row a model id matches, and the dollar figure that follows.
//! Best-effort ballpark rates, never a billing source (see the module docs in
//! `spend/mod.rs`).

/// Providers with no metered API — local inference (Ollama) or a CLI agent
/// authenticated via the user's own tool login (Claude Code/Codex/Gemini
/// CLI/Antigravity/Opencode/Cursor/Copilot/Qwen Code). Always $0 real cost
/// regardless of token volume — never estimated. Deliberately excludes
/// `ollama-cloud` (a paid hosted service).
pub(super) fn is_free_provider(provider: &str) -> bool {
    matches!(
        provider,
        "ollama"
            | "claude-code"
            | "codex"
            | "gemini-cli"
            | "antigravity"
            | "opencode"
            | "cursor"
            | "qwen-code"
    )
}

/// Whether a call costs nothing: [`is_free_provider`], OR an
/// `openai-compatible` request resolved against a local server (LM Studio /
/// llama.cpp / vLLM on `localhost`/`127.0.0.1`/`0.0.0.0`/`::1`) — the
/// `openai-compatible` provider id is otherwise treated as paid (OpenRouter/
/// Groq/Together/DeepSeek/Azure-style gateways are genuinely metered), so
/// without this a local dev server would show a fake `DEFAULT_RATE` dollar
/// figure. `base_url` is ignored for every other provider.
pub(super) fn is_free_call(provider: &str, base_url: Option<&str>) -> bool {
    if is_free_provider(provider) {
        return true;
    }
    provider == "openai-compatible" && base_url.is_some_and(is_localhost_url)
}

/// Crude host check — good enough for a cost gate, never used for routing or
/// security. Tolerates a scheme, a trailing path/query, and an optional `:port`.
///
/// The port is stripped structurally rather than by "split on the last colon":
/// an IPv6 host carries colons of its own, so for a bracketed host with NO port
/// (`http://[::1]/v1`) the last colon is an *internal* one and the old rule cut
/// the host down to `"[:"`, matching nothing. A bare `::1` collapsed the same
/// way. Both are loopbacks the caller means to treat as free.
fn is_localhost_url(url: &str) -> bool {
    let without_scheme = url.rsplit_once("://").map_or(url, |(_, rest)| rest);
    let authority = without_scheme.split(['/', '?']).next().unwrap_or("");
    // Strip an optional `userinfo@` prefix (`user@[::1]`, `user:pass@[::1]`) — a
    // host can't contain `@`, so the segment after the last `@` is the host. Left
    // in place, the userinfo defeated the bracket match below.
    let host_and_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, rest)| rest);
    let host = if let Some(close) = host_and_port.find(']') {
        // Bracketed IPv6: the host is everything up to and including `]`.
        &host_and_port[..=close]
    } else if host_and_port.matches(':').count() > 1 {
        // Bare IPv6 — a port cannot be appended unambiguously, so keep it whole.
        host_and_port
    } else {
        host_and_port
            .split_once(':')
            .map_or(host_and_port, |(h, _)| h)
    };
    // Literal spellings only — this is a cost/display gate, never routing or
    // security, so an IP-parsing crate would be overkill. Both the `::1`
    // shorthand and its fully-expanded (`0:0:0:0:0:0:0:1`) and IPv4-mapped
    // (`::ffff:127.0.0.1`) forms count, bracketed or bare.
    matches!(
        host,
        "localhost"
            | "127.0.0.1"
            | "0.0.0.0"
            | "::1"
            | "[::1]"
            | "0:0:0:0:0:0:0:1"
            | "[0:0:0:0:0:0:0:1]"
            | "::ffff:127.0.0.1"
            | "[::ffff:127.0.0.1]"
    )
}

/// `(model-name-PREFIX, input $/1M tokens, output $/1M tokens)`. Matched by
/// prefix (case-insensitive) so date-suffixed snapshots (`gpt-4o-2024-08-06`)
/// still match their family. **Order matters**: a more specific prefix (e.g.
/// `gpt-4o-mini`) must precede a shorter prefix it also satisfies (`gpt-4o`),
/// since the first match wins. Approximate list prices as of 2026 — a
/// best-effort ballpark, not a billing-accurate source (see module docs).
/// Hand-curated, not fetched, same staleness risk as `provider-meta.ts`'s
/// model lists: a row for a model that gets retired should stay (so
/// already-recorded spend keeps its real rate) but a NEW/replacement model
/// needs its own row, added with the authority page + date it was checked
/// against, not carried over from a sibling by memory.
const RATES: &[(&str, f64, f64)] = &[
    // OpenAI
    ("gpt-4o-mini", 0.15, 0.60),
    ("gpt-4o", 2.50, 10.00),
    // GPT-4 Turbo's 1106 snapshot ("gpt-4-1106-preview"/"-vision-preview") must
    // precede the "gpt-4.1" rows below: post dot/dash normalization, bare
    // "gpt-4.1" becomes the prefix "gpt-4-1", which is ALSO a prefix of
    // "gpt-4-1106-preview" — without this row, that 1106 id silently matched
    // the $2/$8 gpt-4.1 rate instead of its own $10/$30 list price.
    ("gpt-4-1106", 10.00, 30.00),
    ("gpt-4.1-mini", 0.40, 1.60),
    ("gpt-4.1-nano", 0.10, 0.40),
    ("gpt-4.1", 2.00, 8.00),
    ("gpt-4-turbo", 10.00, 30.00),
    ("gpt-4", 30.00, 60.00),
    ("gpt-3.5", 0.50, 1.50),
    ("o1-mini", 1.10, 4.40),
    ("o1", 15.00, 60.00),
    ("o3-mini", 1.10, 4.40),
    ("o3", 2.00, 8.00),
    ("o4-mini", 1.10, 4.40),
    // Anthropic
    ("claude-3-5-haiku", 0.80, 4.00),
    ("claude-3-haiku", 0.25, 1.25),
    ("claude-haiku-4", 1.00, 5.00),
    ("claude-fable-5", 10.00, 50.00),
    ("claude-opus-5", 5.00, 25.00),
    ("claude-opus-4-5", 5.00, 25.00),
    ("claude-opus-4-6", 5.00, 25.00),
    ("claude-opus-4-7", 5.00, 25.00),
    ("claude-opus-4-8", 5.00, 25.00),
    ("claude-opus-4", 15.00, 75.00),
    ("claude-3-opus", 15.00, 75.00),
    ("claude-sonnet-5", 3.00, 15.00),
    ("claude-sonnet-4", 3.00, 15.00),
    ("claude-3-7-sonnet", 3.00, 15.00),
    ("claude-3-5-sonnet", 3.00, 15.00),
    ("claude-3-sonnet", 3.00, 15.00),
    // Gemini
    // `gemini-3-pro-preview` is SHUT DOWN (`ai.google.dev/gemini-api/docs/models`,
    // checked 2026-08-04) and no longer in the curated list (`provider-meta.ts`
    // now ships `gemini-3.6-flash` instead). This row is kept anyway — same
    // precedent as `text-embedding-004` elsewhere in this file — so an
    // already-recorded historical spend entry for it still gets its real rate
    // instead of silently falling through to DEFAULT_RATE.
    ("gemini-3-pro-preview", 2.00, 12.00),
    // gemini-3.6-flash — LIVE, Stable status, curated-list default (see
    // `provider-meta.ts`). Standard tier, verified live rate
    // (`ai.google.dev/gemini-api/docs/pricing`, checked 2026-08-04).
    ("gemini-3.6-flash", 1.50, 7.50),
    // gemini-3.1-pro-preview — LIVE, Preview status, curated-list "Pro" tier
    // entry (see `provider-meta.ts`). Standard tier, prompts <=200k tokens
    // (`ai.google.dev/gemini-api/docs/pricing`, checked 2026-08-04).
    ("gemini-3.1-pro-preview", 2.00, 12.00),
    // gemini-3.5-flash / -lite — both LIVE, Stable, curated-list entries (see
    // `provider-meta.ts`). Standard tier, verified live rates
    // (`ai.google.dev/gemini-api/docs/pricing`, checked 2026-08-04). `-lite`
    // must precede the bare `-flash` row — same dot/dash-normalized-prefix
    // reason as the `gemini-2.5-flash-lite`/`gemini-2.5-flash` pair below.
    ("gemini-3.5-flash-lite", 0.30, 2.50),
    ("gemini-3.5-flash", 1.50, 9.00),
    ("gemini-2.5-pro", 1.25, 10.00),
    ("gemini-2.5-flash-lite", 0.10, 0.40),
    ("gemini-2.5-flash", 0.30, 2.50),
    ("gemini-2.0-flash-lite", 0.075, 0.30),
    ("gemini-2.0-flash", 0.10, 0.40),
    ("gemini-1.5-flash", 0.075, 0.30),
    ("gemini-1.5-pro", 1.25, 5.00),
    // gpt-oss (OpenAI's open-weight model family) — the model this row exists
    // for previously fell through to DEFAULT_RATE ($3/$15), ~100x too high for
    // this tier of open model. Hosted primarily via Ollama Cloud
    // (`ollama-cloud`), whose ids use Ollama's colon-tag form
    // ("gpt-oss:120b"/"gpt-oss:20b" — NOT the dash form other gateways use),
    // and re-servable through any `openai-compatible` gateway that hosts it
    // (dash form, "gpt-oss-120b"). Ollama Cloud itself publishes NO per-token
    // rate for individual accounts — its plans are flat-fee subscriptions
    // ($0/$20 per month, Free/Pro) gated by a per-model "usage level", not
    // $/1M tokens (`ollama.com/pricing`, checked 2026-08-05; Teams overage
    // bills "at the model's token rate" but that rate itself is not published
    // anywhere on the page). These rates are OpenRouter's current live
    // published rate for the SAME open-weight model
    // (`openrouter.ai/api/v1/models`, checked 2026-08-05) — the closest
    // verifiable $/1M figure for this model family, and the SAME rows also
    // correctly price gpt-oss reached through any other openai-compatible
    // gateway that bills it per-token. The colon-tag rows must precede the
    // bare `"gpt-oss"` catch-all below (same "more specific prefix first"
    // rule as every other row in this table) — a bare `"gpt-oss"` row is a
    // prefix of both and would otherwise win first and mask the size-specific
    // rates.
    ("gpt-oss:120b", 0.037, 0.17),
    ("gpt-oss:20b", 0.03, 0.13),
    // `gpt-oss-safeguard-20b` is the one shipped variant whose rate is HIGHER
    // than either base size ($0.075/$0.30 per 1M — Groq via OpenRouter,
    // `openrouter.ai/api/v1/models/openai/gpt-oss-safeguard-20b/endpoints`,
    // checked 2026-08-06), so the catch-all below would under-cost it by ~2x.
    // Must precede the bare `"gpt-oss"` prefix row for the same reason the
    // colon-tag rows do.
    ("gpt-oss-safeguard", 0.075, 0.30),
    // Catch-all for any other gpt-oss spelling/gateway (dash form, a future
    // size) — the 120B rate, the higher of the two BASE sizes. Note this is
    // NOT a conservative ceiling for the family: `-safeguard` above already
    // prices above it, so a genuinely new variant can still be under-costed
    // until it gets its own row. Check a new variant's published rate rather
    // than assuming this row covers it.
    ("gpt-oss", 0.037, 0.17),
    // Embeddings — input-only (no completion tokens), so the output rate is
    // always 0.0 and unused (`embed_text` always records `output_tokens: 0`).
    ("text-embedding-3-small", 0.02, 0.0),
    ("text-embedding-3-large", 0.13, 0.0),
    ("text-embedding-ada-002", 0.10, 0.0),
    // text-embedding-004 was retired by Google (shutdown Jan 14, 2026); the
    // adapter now defaults to gemini-embedding-2, with gemini-embedding-001
    // still available for text-only use. Verified via the live Gemini API
    // pricing docs, not memory. The retired row stays — historical spend
    // records made against it still exist, and letting them fall through to
    // DEFAULT_RATE would silently 120x-overestimate an already-recorded call.
    ("text-embedding-004", 0.025, 0.0),
    ("gemini-embedding-2", 0.20, 0.0),
    ("gemini-embedding-001", 0.15, 0.0),
];

/// Conservative default for a cloud model this table doesn't recognize (a
/// mid-tier price point) — so an unknown-but-paid model never silently shows
/// $0, which would look like a free/local call. New models therefore need no
/// code change to show *some* estimate; the table can be tightened later.
const DEFAULT_RATE: (f64, f64) = (3.00, 15.00);

/// True if `candidate` starts with `prefix`, treating `.` and `-` as
/// equivalent on BOTH sides — a model id spelled with a dot
/// (`claude-opus-4.7`) must still match a dash-form row (`claude-opus-4-7`).
/// Normalizing only `candidate` would corrupt the OpenAI/Gemini rows that use
/// a literal dot for their own version number (`gpt-4.1-mini`,
/// `gemini-2.5-flash`) — e.g. `"gpt-4.1-mini-2024".replace('.', "-")` no
/// longer starts with the literal `"gpt-4.1-mini"` prefix. Normalizing both
/// sides identically keeps every existing match intact — but it also *adds*
/// matches, not just the intended dot/dash equivalence: post-normalization,
/// `"gpt-4.1"` becomes the prefix `"gpt-4-1"`, which now ALSO matches ids like
/// `"gpt-4-1106-preview"` that were never meant to hit that row (see the
/// `("gpt-4-1106", ...)` row in [`RATES`], added specifically to out-rank this
/// side effect via ordering).
fn prefix_matches(candidate: &str, prefix: &str) -> bool {
    candidate
        .replace('.', "-")
        .starts_with(&prefix.replace('.', "-"))
}

/// The [`RATES`] row matched for `model` (case-insensitive prefix match), or
/// `None` if it falls through to [`DEFAULT_RATE`]. Exposes the matched
/// **prefix**, not just the resulting price — several rows share a price
/// (e.g. `claude-sonnet-5`'s rate equals `DEFAULT_RATE`), so a price-only
/// assertion can pass even when the wrong row (or no row) matched; tests use
/// this to pin the actual match. Shared by [`estimate_cost`] so the
/// normalization (strip a leading `models/`, then a vendor prefix) lives in
/// one place.
fn rate_for(model: &str) -> Option<&'static (&'static str, f64, f64)> {
    // Gemini ids can arrive prefixed (`models/gemini-2.5-flash`); strip it
    // before matching so it isn't silently mismatched to DEFAULT_RATE.
    let lower = model.to_ascii_lowercase();
    let m = lower.strip_prefix("models/").unwrap_or(&lower);
    // Vendor-prefixed ids (`anthropic/claude-fable-5`, as seen through an
    // OpenRouter-style gateway) must match on the bare model name — keep only
    // the segment after the last `/`, so the row lookup below isn't silently
    // mismatched to DEFAULT_RATE.
    let last_segment = m.rsplit('/').next().unwrap_or(m);
    RATES
        .iter()
        .find(|(prefix, _, _)| prefix_matches(last_segment, prefix))
}

/// Estimated USD cost for one call, from the static [`RATES`] table (or
/// [`DEFAULT_RATE`] for an unrecognized model). Pure — callers gate local/
/// CLI-agent providers to $0 via [`is_free_provider`] before calling this, so
/// this function never needs to know about providers at all.
pub fn estimate_cost(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    let (in_rate, out_rate) = rate_for(model)
        .map(|(_, i, o)| (*i, *o))
        .unwrap_or(DEFAULT_RATE);
    (f64::from(input_tokens) / 1_000_000.0) * in_rate
        + (f64::from(output_tokens) / 1_000_000.0) * out_rate
}

#[cfg(test)]
mod tests;
