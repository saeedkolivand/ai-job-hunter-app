//! Model capabilities + the sampling-intent vocabulary (renderer owns intent,
//! adapter owns numbers). Split out of `mod.rs` (R8 line-budget split).
//!
//! The renderer states WHAT a generation step is — exact/deterministic or
//! creative prose — never a raw sampling number. Each provider adapter maps
//! `(model, intent)` to its OWN numbers via [`AiProvider::sampling_profile`].
//!
//! THE RULE: preserve this app's pre-fix EFFECTIVE sampling wherever the
//! provider accepts it; omit ONLY where sending is forbidden or
//! documented-harmful. This fix's whole purpose is to stop sending values
//! where they break things (Claude 4.7+/5 400s on ANY non-default sampling
//! param; OpenAI's reasoning models reject `temperature`; Gemini 3.x is
//! documented to loop/degrade below its 1.0 default) — it is NOT license to
//! change register everywhere else. "Omit by default" is a category error:
//! omitting a wire field does not hand control to a safe general default, it
//! hands control to whatever the endpoint does with an absent field, which is
//! frequently NOT what this app shipped before. Confirmed empirically against
//! a live local Ollama install (not vendor docs): `qwen3.6:27b-q4_K_M`'s own
//! Modelfile defaults to `temperature: 1, presence_penalty: 1.5, top_p: 0.95`;
//! `gemma4:31b-it-q4_K_M` defaults to `temperature: 1`. Omitting on Ollama
//! therefore does not mean "sane default" — it means "whatever this
//! particular model's Modelfile says", which can be a materially WORSE
//! determinism/fabrication risk than anything this app used to send. See
//! [`DETERMINISTIC_TEMPERATURE`] and `OllamaClient::sampling_profile`.
//!
//! So every adapter's `sampling_profile` declares REAL values reproducing
//! this app's pre-fix shipped numbers for every intent, EXCEPT the four
//! documented-unsafe cases: Anthropic adaptive/frontier models, OpenAI
//! reasoning models, Gemini 3.x, and an unknown/unclassifiable model on any
//! provider (the fail-safe — an id this app cannot positively classify
//! defaults to neutral, the direction that can never 400 or trigger a
//! documented degradation).
//!
//! See `AiGenerateRequestSchema.intent` (`packages/shared/src/schemas/index.ts`)
//! for the wire contract this mirrors.

use super::AiGenerateRequest;

// ── Model capabilities ─────────────────────────────────────────────────────────

/// Which token-limit field a model's API expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenParam {
    MaxTokens,
    MaxCompletionTokens,
    NumPredict,
    MaxOutputTokens,
}

/// Per-model feature matrix. All provider/model-specific behavior lives here so
/// the request builders never special-case providers inline. Some flags are
/// declared ahead of their consumers (tools / JSON mode / embeddings) to keep
/// adding capability-gated features cheap.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct ModelCapabilities {
    pub supports_temperature: bool,
    pub supports_system_role: bool,
    pub supports_streaming: bool,
    pub supports_reasoning: bool,
    pub supports_tools: bool,
    pub supports_json_mode: bool,
    pub supports_embeddings: bool,
    /// Whether this provider/model can attempt a `research*` web search at
    /// all — a static, network-free check distinct from whether a search
    /// actually succeeds (which also depends on a configured account key,
    /// checked at call time). Lets callers that fan out a research call per
    /// item (e.g. `ai_research_answer`, one call per selected question) skip
    /// the daily-budget charge entirely for a provider that can never search,
    /// instead of charging N times for N guaranteed-empty results.
    pub supports_web_search: bool,
    pub token_param: TokenParam,
}

// ── Sampling intent (renderer owns intent, adapter owns numbers) ───────────────

/// The renderer's declared intent for one generation step. Parsed from the
/// wire `AiGenerateRequest.intent` string via [`resolve_intent`] —
/// unrecognized/absent always fails toward [`Intent::Default`], never a
/// guess. On every adapter, `Default` resolves to the SAME numbers as
/// [`Intent::Deterministic`] — the corrected rule is "declare real values for
/// every intent" on an accepting model, and among the three registers,
/// exact/non-creative is the conservative one to fall back to for a caller
/// with no declared opinion (no creative-writing penalty knobs applied to an
/// unknown-purpose call). The FOUR documented-unsafe classification cases
/// (Anthropic adaptive/frontier, OpenAI reasoning, Gemini 3.x, an
/// unrecognized/unclassifiable model) still fail toward neutral regardless of
/// intent, `Default` included — see the module doc comment above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Intent {
    /// Analysis/résumé/job-ad-summary/GitHub-projects: exact, non-creative
    /// output. Every adapter treats [`Intent::Default`] identically to this
    /// variant (see this enum's own doc comment).
    Deterministic,
    /// Interview questions, likely questions, STAR feedback: creative,
    /// detector-resistant prose with no traceability requirement.
    Prose,
    /// Cover letter, application answers, referral messages, application
    /// email: the SAME detector-resistant register as [`Intent::Prose`], but
    /// the output makes factual claims about the candidate — a real résumé
    /// achievement, a real reason for applying — that must stay traceable to
    /// the résumé/job ad. Concretely `Prose` minus the presence-penalty knob
    /// (it pushes a model toward new topics, i.e. invented candidate facts).
    /// Never collapse this back into `Prose`.
    ProseGrounded,
    /// No declared intent — see this enum's own doc comment: resolves to the
    /// SAME numbers as [`Intent::Deterministic`] on an accepting model, never
    /// a genuinely separate "let the provider fully decide" state (that
    /// would repeat the "omission is neutral" mistake this fix corrects).
    #[default]
    Default,
}

/// Parse `req.intent` into the typed [`Intent`] every adapter's
/// `sampling_profile` consumes. Pure so it needs no `AppHandle`/mock harness.
pub fn resolve_intent(req: &AiGenerateRequest) -> Intent {
    match req.intent.as_deref() {
        Some("deterministic") => Intent::Deterministic,
        Some("prose") => Intent::Prose,
        Some("prose_grounded") => Intent::ProseGrounded,
        _ => Intent::Default,
    }
}

// ── Shared target numbers (the values being preserved, not invented) ──────────
//
// These reproduce the pre-fix renderer's hardcoded numbers — sent uniformly
// to every provider before this fix, which is exactly the defect being
// corrected (not the numbers themselves). Each adapter's `sampling_profile`
// references these directly rather than re-typing them, so the historical
// value survives as one auditable source instead of N silently-drifting
// copies. App choices inside documented "reasonable" ranges where a vendor
// publishes one — never vendor-published numbers themselves.

/// `Intent::Deterministic`'s temperature — reproduces this app's pre-fix
/// shipped default for exact/near-JSON output (analysis 0.15, résumé/
/// job-ad-summary/inline-rewrite 0.3, GitHub-projects 0.4). Determinism on
/// the strictest surface (analysis — `runAnalysis` hard-throws
/// "malformed output" on a JSON parse failure) leans primarily on the
/// analyze prompt's explicit JSON-only output contract
/// (`packages/prompts/src/analyze/system-prompt.ts`), the vendor-recommended
/// lever — but omitting this value is NOT a safe fallback on a provider that
/// accepts it: see the empirical Ollama Modelfile defaults in the module doc
/// comment above (`temperature: 1` on both currently-default local models).
pub const DETERMINISTIC_TEMPERATURE: f64 = 0.3;
/// `Intent::Prose`'s temperature (interview questions 0.5, likely questions
/// 0.5, STAR feedback 0.4 — the cover letter's 0.58/0.8 moved to
/// `Intent::ProseGrounded`, see below).
pub const PROSE_TEMPERATURE: f64 = 0.5;
/// `Intent::ProseGrounded`'s temperature (application answers 0.5, referral
/// 0.7, application email 0.7, cover letter 0.58 small-tier / 0.8
/// large-tier).
///
/// This is HIGHER than [`PROSE_TEMPERATURE`], which reads backwards until you
/// see why: grounding is enforced by withholding `presence_penalty` (the knob
/// that rewards new topics), NOT by cooling the sampler. The ordering is an
/// accident of which surfaces carry which intent — the grounded ones are
/// long-form letters and emails, the ungrounded ones are STAR feedback (0.4)
/// and interview questions (0.5). Do not read a semantic claim into it, and do
/// not "fix" it by swapping the values: both preserve the per-surface
/// temperatures this app shipped before sampling moved into the adapters.
pub const PROSE_GROUNDED_TEMPERATURE: f64 = 0.6;
/// Shared prose penalty knobs — RAID (ACL 2024) detector-resistance,
/// unchanged from the pre-fix renderer's `PROSE_SAMPLING` constant.
/// `Intent::Prose` gets the full set; `Intent::ProseGrounded` gets
/// `top_p`/`frequency_penalty` (and, on Ollama, `repeat_penalty`) but NEVER
/// `presence_penalty` — see [`PROSE_PRESENCE_PENALTY`]'s own doc.
pub const PROSE_TOP_P: f64 = 0.95;
pub const PROSE_FREQUENCY_PENALTY: f64 = 0.3;
/// `Intent::Prose`-only — pushes a model toward new topics; deliberately
/// never applied to `Intent::ProseGrounded` (see [`Intent::ProseGrounded`]'s
/// own doc for why). Declaring this app-side value is a request to NOT use
/// an aggressive one — it is not a guarantee against a provider whose own
/// server-side default disagrees when this app omits the field entirely
/// (e.g. a local Ollama Modelfile can set its own `presence_penalty`, as
/// high as `1.5` on a currently-default model — see the module doc comment).
pub const PROSE_PRESENCE_PENALTY: f64 = 0.2;
/// Ollama's own repetition knob (distinct semantics from
/// `frequency_penalty` — never a remap). Applied wherever `Intent::Prose`/
/// `Intent::ProseGrounded` apply `PROSE_TOP_P`, on Ollama native + Ollama
/// Cloud native-shaped requests only (OpenAI/Gemini have no such field).
pub const PROSE_REPEAT_PENALTY: f64 = 1.15;

/// A provider's own sampling NUMBERS for a `(model, intent)` pair — the
/// adapter's half of the intent/numbers split. A `None` field means "omit
/// this wire parameter entirely" — reserved for the four documented-unsafe
/// cases (see the module doc comment above): Claude 4.7+/5, OpenAI's
/// reasoning models, Gemini 3.x, and an unrecognized/unclassifiable model on
/// any provider. Everywhere else, a provider's `sampling_profile` declares
/// the real [`DETERMINISTIC_TEMPERATURE`]/[`PROSE_TEMPERATURE`]/
/// [`PROSE_GROUNDED_TEMPERATURE`] + penalty constants — [`Default`] being the
/// derived zero value is a type-system convenience for the NEUTRAL case, not
/// a claim that neutral is the general answer.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SamplingProfile {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub repeat_penalty: Option<f64>,
}

impl SamplingProfile {
    /// Merge this provider-declared profile with the request's explicit
    /// numeric fields — explicit ALWAYS wins, per field. The Ollama
    /// per-model/per-step temperature slider (`LocalModelLimits.tsx`) is the
    /// only value in the system a human actually chose; every other numeric
    /// field is currently never sent by the renderer at all, but the same
    /// override contract applies to any of them the moment one is. A field
    /// absent on both sides stays `None` — omitted from the wire body.
    ///
    /// Whether an explicit value can reach a model that would reject it
    /// depends on WHERE each adapter reads the merged result, and is NOT
    /// uniform across adapters — this method does not itself enforce a
    /// safety gate:
    ///
    /// - OpenAI (`build_chat_stream_body`) and Anthropic
    ///   (`build_chat_stream_body`) both read `.temperature`/`.top_p` from
    ///   the merged result ONLY inside a `caps.supports_temperature` /
    ///   `anthropic_supports_temperature` gate, so an explicit value
    ///   genuinely never reaches a model that 400s on it there.
    ///   `sampling_profile` ALSO already returns a neutral (`None`) profile
    ///   for those same gated models, so in practice the explicit value is
    ///   blocked twice.
    /// - Gemini's `build_chat_stream_body` gates `top_p` the same way
    ///   (unconditionally omitted on a v3+ model, even when explicit — it's
    ///   the renderer's own anti-detection knob, not a user dial), but does
    ///   **NOT** gate `temperature` at the send site: an explicit
    ///   `temperature` intentionally reaches a Gemini 3.x model even though
    ///   `sampling_profile` itself would have returned neutral (a
    ///   deliberate, pre-existing, tested choice — "a deliberate user value
    ///   must still be honored on a v3+ model" — safe because Google
    ///   documents this as a quality recommendation, not a 400).
    pub fn resolve(self, req: &AiGenerateRequest) -> SamplingProfile {
        SamplingProfile {
            temperature: req.temperature.or(self.temperature),
            top_p: req.top_p.or(self.top_p),
            frequency_penalty: req.frequency_penalty.or(self.frequency_penalty),
            presence_penalty: req.presence_penalty.or(self.presence_penalty),
            repeat_penalty: req.repeat_penalty.or(self.repeat_penalty),
        }
    }
}
