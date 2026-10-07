//! Anthropic thinking-mode classification: which models get classic
//! (budget-token) vs adaptive extended thinking, and the shared model-id
//! normalization/boundary-matching helpers every version-needle gate in this
//! adapter builds on. Split out of `anthropic.rs` (R8 LOC cap) — a pure move.

/// Whether a model should be sent the classic `thinking: {type:"enabled",
/// budget_tokens}` block (extended thinking).
///
/// Anthropic's extended-thinking mode forces `temperature=1.0` and consumes extra
/// output tokens; a model that does **not** support it answers a `thinking`
/// request with a 400. Only the Claude 3.7+ / 4.x families (incl. Haiku 4.5) use
/// this classic budget-token mechanism, so gate on the model id (mirrors
/// [`gemini_supports_thinking`](super::super::gemini)). Older 3.0–3.5 models are
/// excluded (never supported it).
///
/// `claude-opus-4-7` and `claude-opus-4-8` are **deliberately excluded** even
/// though they match the `claude-opus-4` substring below: per Anthropic's
/// thinking docs they are adaptive-only models ("Extended thinking: No" in the
/// per-model table) — see [`anthropic_uses_adaptive_thinking`] instead.
///
/// The Claude 5 family (`claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`,
/// Mythos, and later adaptive families) is excluded too: it replaced classic
/// budget-token thinking with adaptive thinking, which this predicate does not
/// gate — see [`anthropic_uses_adaptive_thinking`]. Unknown future names
/// default to **off** — a graceful miss (no thinking) is always safe; a
/// wrongful `thinking` request 400s the whole generation.
pub(super) fn anthropic_supports_thinking(model: &str) -> bool {
    let m = normalize_model_id(model);
    // Opus 4.7/4.8 are adaptive-only (see doc comment) — carve them out before
    // the "claude-opus-4" substring below would otherwise catch them.
    if contains_version_needle(&m, "opus-4-7") || contains_version_needle(&m, "opus-4-8") {
        return false;
    }
    // Claude 3.7 (the first extended-thinking model) and the 4.x families.
    // Deliberately does NOT match "claude-opus-5"/"claude-sonnet-5"/
    // "claude-fable-5"/mythos — the 5 family (and later adaptive families)
    // use adaptive thinking, not this mechanism. Do not widen this to a bare
    // "claude-" match.
    contains_version_needle(&m, "claude-3-7")
        || contains_version_needle(&m, "claude-4")
        || contains_version_needle(&m, "claude-opus-4")
        || contains_version_needle(&m, "claude-sonnet-4")
        || contains_version_needle(&m, "claude-haiku-4")
}

/// Whether `max_tokens` reaches the size gate for **classic** extended
/// thinking. Necessary but not sufficient: the streaming body builder also
/// requires the request to carry an effort (classic thinking is opt-in), so a
/// request with no effort never engages it however large `max_tokens` is —
/// see its call site.
pub(crate) fn classic_thinking_engages(max_tokens: u32) -> bool {
    max_tokens >= 2048
}

/// Whether a model should be sent Anthropic's **adaptive** thinking block
/// (`thinking: {"type":"adaptive","display":"summarized"}`): `claude-opus-4-7`,
/// `claude-opus-4-8` (adaptive-only per the extended-thinking per-model table),
/// and the Claude 5 family (`claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`,
/// Mythos) and later adaptive families.
///
/// `display` defaults to `"omitted"` on every one of these models (per
/// Anthropic's thinking docs) — an empty `thinking` field, signature only. We
/// opt into `"summarized"` explicitly so the app's thinking view actually
/// receives text; without this, "sending nothing extra" silently regresses the
/// thinking view to blank on every adaptive model, even though the model is
/// still thinking (and billing for it) under the hood. Every model matched
/// here 400s on ANY non-default temperature/top_p/top_k, on every request,
/// not just while thinking (Anthropic's "Sampling parameters" note) — see
/// [`super::capabilities::anthropic_supports_temperature`], which builds on
/// this predicate.
///
/// New adaptive families need a new substring added here — this predicate IS
/// the adapter's model-classification layer (the zero-change rule protects
/// business logic/callers, not this file). Unknown names default to **off** —
/// a graceful miss (no adaptive block, `display` stays omitted) is always
/// safe; guessing wrong never 400s *here* (unlike the classic gate above),
/// since adaptive thinking is already on by default on every model in this
/// set — but see [`super::capabilities::anthropic_supports_temperature`] for
/// the fail-safe that keeps a wrong guess from 400ing on the
/// sampling-parameter side either.
pub(super) fn anthropic_uses_adaptive_thinking(model: &str) -> bool {
    let m = normalize_model_id(model);
    contains_version_needle(&m, "opus-4-7")
        || contains_version_needle(&m, "opus-4-8")
        || contains_version_needle(&m, "opus-5")
        || contains_version_needle(&m, "sonnet-5")
        || contains_version_needle(&m, "fable-5")
        // A bare family word rather than a version needle — Mythos is gated as
        // a WHOLE family here (unlike `anthropic_supports_effort`, which lists
        // only the two documented Mythos names), so `mythos-6` and any later
        // point release stay adaptive with no code change. It still goes
        // through [`contains_version_needle`]: the helper's rule is a component
        // boundary, not a version shape, so it applies unchanged to a bare
        // word, and a raw `contains` would classify `claude-notmythos-9` as
        // adaptive — the same fail-open direction the version needles closed.
        || contains_version_needle(&m, "mythos")
}

/// Shared normalization for the two thinking-mode predicates above:
/// **strip a vendor prefix** (an OpenRouter-style `anthropic/claude-...` id —
/// keep only the segment after the last `/`, so a vendor-prefixed id is
/// classified identically to its bare form on every predicate, including
/// [`super::capabilities::anthropic_supports_temperature`]'s new-family
/// fail-safe, which otherwise silently disarms on a prefixed id since it no
/// longer starts with `"claude-"`), then lowercase, then collapse dot-form
/// version separators to dashes, so a model id spelled `claude-opus-4.7` (dot
/// form) still matches the `opus-4-7` needle instead of falling through to
/// the classic `claude-opus-4` gate and 400ing (adaptive models reject the
/// classic `thinking.enabled` shape).
pub(super) fn normalize_model_id(model: &str) -> String {
    let bare = model.rsplit('/').next().unwrap_or(model);
    bare.to_ascii_lowercase().replace('.', "-")
}

/// Component-aware substring check for the version needles used by the
/// thinking-mode predicates above: `haystack` must contain `needle` sitting on
/// its own id COMPONENTS — the characters on both sides of the match must each
/// be either end-of-string or a separator (anything non-alphanumeric: the `-`
/// every Anthropic id uses, plus the `.`/`/`/`_`/`@`/`:` a gateway, Bedrock or
/// Vertex id can introduce; [`normalize_model_id`] has already folded `.` to
/// `-` and dropped a vendor prefix by the time this runs).
///
/// A raw [`str::contains`] has no boundary at all — it would let
/// `opus-4-70`/`opus-4-71`/… wrongly match the `opus-4-7` needle, and
/// `sonnet-50`/`sonnet-58`/… wrongly match `sonnet-5`, exactly the class of
/// prefix-collision bug this file already patched once with the explicit
/// opus-4-7/4-8 carve-out above `claude-opus-4`.
///
/// Checking only for a trailing DIGIT (the first fix) left the same collision
/// reachable from three sides, and every one of them fails OPEN — an
/// unrecognized id silently classified as a known family, which is the exact
/// direction these predicates' doc comments promise they never fail in:
///
/// - a glued prefix — `claude-notopus-4-5` matched the `opus-4-5` needle,
/// - a non-digit glued suffix — `claude-sonnet-4-5alpha` matched `sonnet-4-5`,
/// - and both at once.
///
/// A real id always separates its components (`claude-sonnet-4-5-20250929`,
/// `anthropic.claude-opus-4-5-v1:0`, `claude-opus-4-5@20251101`), so requiring
/// the boundary costs nothing and closes all three.
pub(super) fn contains_version_needle(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let is_boundary = |index: usize| bytes.get(index).is_none_or(|b| !b.is_ascii_alphanumeric());
    haystack
        .match_indices(needle)
        .any(|(idx, _)| (idx == 0 || is_boundary(idx - 1)) && is_boundary(idx + needle.len()))
}

/// True for Anthropic's pre-thinking-era ids — Claude 1.x, 2.x, and 3.x below
/// 3.7 (`claude-3-7` itself already matches [`anthropic_supports_thinking`],
/// so anything reaching this check with a "claude-3" marker is guaranteed to
/// be below 3.7). This is a closed, historical set — Anthropic will never
/// ship a NEW model under these version numbers — so hardcoding it is safe
/// and needs no maintenance for future releases. Its only purpose is keeping
/// [`super::capabilities::anthropic_supports_temperature`]'s new-family
/// fail-safe from misfiring on these long-shipped, well-understood models,
/// which have always accepted a normal `temperature` (they simply predate
/// thinking, unlike a genuinely unclassified NEW family).
pub(super) fn anthropic_is_legacy_pre_thinking(model: &str) -> bool {
    let m = normalize_model_id(model);
    m.contains("claude-3")
        || m.contains("claude-2")
        || m.contains("claude-1")
        || m.contains("claude-instant")
}

/// Thinking-aware `max_tokens` inflation shared by every body builder:
/// the streaming builder (whose `max_tokens` comes from the caller) and the
/// three non-streaming builders (which hardcode a fixed cap). Adaptive
/// thinking is on by default and can't be turned off on several models (Fable
/// can't disable it at all) — it still counts toward `max_tokens` even on
/// paths that never send a `thinking` key (the three non-streaming builders
/// have no thinking-view display concern; default `"omitted"` is correct
/// there and gives faster time-to-first-text per Anthropic's docs).
/// Deliberately **not** gated on a size threshold: a small caller-supplied cap
/// (e.g. the extension bridge's answer-assist calls, see
/// `extension_bridge::answer_assist::ANSWER_ASSIST_MAX_TOKENS`) or a fixed
/// small cap (1024 for web search) still gets thinking billed against it by
/// default, so the inflation must apply unconditionally.
///
/// The headroom is `max(base / 2, 1024)`, not a bare `base / 2`: a small cap
/// would otherwise add less than the ~1024-token floor a model
/// typically needs to produce a useful summarized-thinking pass, leaving too
/// little room for both thinking and the visible response and risking a
/// short/empty draft even after "inflating". `saturating_add` guards `base`
/// being an unclamped caller-supplied IPC `u32` near `u32::MAX`.
pub(super) fn adaptive_max_tokens(model: &str, base: u32) -> u32 {
    if anthropic_uses_adaptive_thinking(model) {
        base.saturating_add((base / 2).max(1024))
    } else {
        base
    }
}
