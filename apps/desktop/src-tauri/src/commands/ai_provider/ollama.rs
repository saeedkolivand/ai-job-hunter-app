//! Ollama (local) provider.
//!
//! This is the ONLY module allowed to reference the Ollama host or its `/api/*`
//! endpoints. Everything Ollama-specific — chat, model list, pull, embeddings,
//! health — lives here so no hidden Ollama assumptions leak into the rest of the
//! codebase.

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};

use super::structured;
use super::timeouts;
use super::{
    map_completion_transport_error, resolve_intent, AgentTurn, AiGenerateRequest, AiProvider,
    ChatMsg, Intent, ModelCapabilities, ProviderId, SamplingProfile, TokenParam, ToolSpec, Usage,
    DETERMINISTIC_TEMPERATURE, PROSE_GROUNDED_TEMPERATURE, PROSE_REPEAT_PENALTY, PROSE_TEMPERATURE,
    PROSE_TOP_P,
};

mod chat;
mod embed;
mod inspect;
mod local_chat;
mod models;
mod search;
mod tools;
mod wire;

// The parent's public surface stays identical for outside callers (`commands/ai/`,
// `commands/system/`, `commands/translation.rs`,
// `ollama_cloud.rs`) — re-exported here rather than editing those call sites.
pub use embed::embed_with;
pub use inspect::{pull, show_model};
pub use models::{list_tag_models, reachable_model};
pub use search::{ollama_web_search, OllamaSearcher};

const EMBED_MODEL: &str = "nomic-embed-text";
/// Ollama's first-party Web Search API (cloud) — authenticated with the Ollama
/// account key (`ai:ollama-cloud`), independent of the local daemon host.
const WEB_SEARCH_URL: &str = "https://ollama.com/api/web_search";
/// Credential slot for the Ollama account key shared by Ollama Cloud chat and
/// Ollama Web Search. Local Ollama has no chat key but still needs this to search.
pub const ACCOUNT_KEY: &str = "ollama-cloud";

/// Resolve the Ollama host (env override or localhost default).
pub fn host() -> String {
    crate::platform::config::ollama_host()
}

/// Whether a local Ollama model advertises tool-calling. Ollama's `/api/chat`
/// only honors a `tools` field on models trained for it; a model without support
/// silently ignores tools (no error, but also no calls), so gate on a conservative
/// allowlist of the known tool-calling families. Unknown names default to `false`,
/// so an agent turn degrades to a single-shot answer instead of a call-less stall.
fn ollama_supports_tools(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("llama3.1")
        || m.contains("llama3.2")
        || m.contains("llama3.3")
        || m.contains("qwen2.5")
        || m.contains("qwen3")
        || m.contains("mistral")
        || m.contains("mixtral")
        || m.contains("command-r")
        || m.contains("firefunction")
        || m.contains("hermes")
        || m.contains("granite")
}

/// Ollama's family of thinking-capable models — shared by local Ollama's
/// native `/api/chat` `think` field (this file, `chat::build_chat_stream_body`)
/// and Ollama Cloud's OpenAI-compatible `reasoning_effort` field
/// (`openai.rs`, gated on `ProviderId::OllamaCloud`): both wire shapes gate
/// on the SAME model catalog, so the classifier lives once here rather than
/// drifting into two copies. Per Ollama's docs
/// (`docs/capabilities/thinking.mdx`, fetched 2026-08-03): Qwen 3, GPT-OSS,
/// DeepSeek-v3.1, and DeepSeek R1 are the currently-documented thinking
/// families. Unknown models default to `false` (a graceful miss — the
/// `think`/`reasoning_effort` field 400s on a non-thinking model, so
/// guessing wrong is never safe).
///
/// The `qwen3-coder` exclusion is scoped to the `qwen3` branch specifically
/// (not a blanket "any model containing `coder`" check) — `qwen3-coder`/
/// `qwen3-coder-plus` are separate, non-thinking models despite matching the
/// `qwen3` substring, but a hypothetical future thinking-capable coder model
/// in the gpt-oss/deepseek families must not be swept out by an unrelated
/// name collision.
pub(super) fn ollama_family_supports_thinking(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    if m.contains("qwen3") {
        return !m.contains("coder");
    }
    m.contains("gpt-oss") || m.contains("deepseek-r1") || m.contains("deepseek-v3.1")
}

/// Levels `think` accepts on every thinking-family model
/// ([`ollama_family_supports_thinking`]) — Ollama's `docs/capabilities/
/// thinking.mdx` (fetched 2026-08-03) documents one uniform `low`/`medium`/
/// `high` string enum, not a per-model-tier table like Gemini's
/// `thinkingLevel`. Still gated with `.contains(&effort)` on the send path
/// below, not just the family-membership boolean — the same protection
/// Gemini/Anthropic/OpenAI use (`effort` is stored PER PROVIDER, not per
/// model — `preferences-store.ts` — so a stale/unrecognized value must never
/// ship just because the CURRENT model happens to be in the thinking
/// family). A no-op today since every thinking-family model shares this same
/// set, but it keeps this call site correct with zero further change the day
/// a future model needs a narrower one.
const OLLAMA_EFFORT_LEVELS: [&str; 3] = ["low", "medium", "high"];

/// The extra, LOWEST tier a thinking model that can switch thinking off
/// offers: it sends `think: false`. Probed against a live daemon: qwen3-style
/// models take a boolean (`false` => no reasoning at all) and ALSO accept the
/// level strings; gpt-oss ignores `false` (it keeps reasoning) and only
/// honours the level strings, so [`ollama_think_is_level_only`] models never
/// list this tier and resolve a stale `off` to `low`, their cheapest tier.
const OLLAMA_OFF: &str = "off";

/// gpt-oss: reasoning cannot be disabled, only levelled.
pub(super) fn ollama_think_is_level_only(model: &str) -> bool {
    model.to_ascii_lowercase().contains("gpt-oss")
}

pub struct OllamaClient;

#[async_trait]
impl AiProvider for OllamaClient {
    fn id(&self) -> ProviderId {
        ProviderId::Ollama
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: ollama_family_supports_thinking(model),
            // Per-model: only tool-calling families advertise it (see the allowlist);
            // unknown models stay `false` so an agent turn degrades safely.
            supports_tools: ollama_supports_tools(model),
            supports_json_mode: true,
            supports_embeddings: true,
            // Attempts research via the Ollama Web Search API (account-key
            // gated at call time, not statically known here).
            supports_web_search: true,
            token_param: TokenParam::NumPredict,
        }
    }

    fn effort_levels(&self, model: &str) -> Vec<&'static str> {
        if !ollama_family_supports_thinking(model) {
            return Vec::new();
        }
        // Lowest tier FIRST: `Completer::low_effort` takes entry 0.
        let mut levels = Vec::with_capacity(4);
        if !ollama_think_is_level_only(model) {
            levels.push(OLLAMA_OFF);
        }
        levels.extend(OLLAMA_EFFORT_LEVELS);
        levels
    }

    /// Declares real values for every intent, on every model, no gating —
    /// unlike every cloud adapter, Ollama has no "this family 400s" split to
    /// fail-safe against, so there is no unknown-model case to stay neutral
    /// for. Omitting is NOT a safe default here: `/api/chat` falls back to
    /// the model's own Modelfile, a file this app cannot see or control at
    /// request time. Confirmed empirically against this machine's live local
    /// Ollama (not vendor docs): `qwen3.6:27b-q4_K_M`'s Modelfile defaults to
    /// `temperature: 1, presence_penalty: 1.5, top_p: 0.95`;
    /// `gemma4:31b-it-q4_K_M` defaults to `temperature: 1`. Both are
    /// currently-default-tier local models — a résumé/analysis JSON call
    /// omitting `temperature` on either would run at a creative-writing
    /// temperature (and, on the first, a presence-penalty HIGHER than
    /// anything this app ever sent), not a "sane default". Reuses the SAME
    /// per-intent targets every other accepting adapter does (this app's
    /// pre-fix renderer sent identical numbers to Ollama as every cloud
    /// provider) — `repeat_penalty` is Ollama's own field (see
    /// `chat::build_chat_stream_body`'s doc comment), never a `frequency_penalty`
    /// remap, and this app has no way to forward `presence_penalty` to
    /// Ollama at all (its wire body has no such field), so
    /// `Intent::ProseGrounded`'s presence-penalty distinction from
    /// `Intent::Prose` has no Ollama equivalent to withhold, exactly like
    /// Anthropic's — but the two intents still land on different
    /// temperatures here (`PROSE_TEMPERATURE` vs `PROSE_GROUNDED_TEMPERATURE`
    /// below), pinned by this adapter's own tests.
    fn sampling_profile(&self, _model: &str, intent: Intent) -> SamplingProfile {
        match intent {
            // `Default` (no declared intent) resolves the same as
            // `Deterministic` — see `Intent`'s own doc comment
            // (`commands/ai_provider/sampling.rs`).
            Intent::Deterministic | Intent::Default => SamplingProfile {
                temperature: Some(DETERMINISTIC_TEMPERATURE),
                ..SamplingProfile::default()
            },
            Intent::Prose => SamplingProfile {
                temperature: Some(PROSE_TEMPERATURE),
                top_p: Some(PROSE_TOP_P),
                repeat_penalty: Some(PROSE_REPEAT_PENALTY),
                ..SamplingProfile::default()
            },
            Intent::ProseGrounded => SamplingProfile {
                temperature: Some(PROSE_GROUNDED_TEMPERATURE),
                top_p: Some(PROSE_TOP_P),
                repeat_penalty: Some(PROSE_REPEAT_PENALTY),
                ..SamplingProfile::default()
            },
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        let sampling = self
            .sampling_profile(&req.model, resolve_intent(req))
            .resolve(req);
        chat::stream_chat(app, job_id, req, sampling).await
    }

    async fn complete(
        &self,
        _app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String> {
        chat::complete_impl(model, system, user, temperature, None)
            .await
            .map(|(text, _)| text)
    }

    async fn complete_with_usage(
        &self,
        _app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
        chat::complete_impl(model, system, user, temperature, None).await
    }

    /// Plain text, but with the request's effort (`think`), `num_predict` and
    /// `num_ctx` — no `format`.
    async fn complete_with_effort(
        &self,
        _app: &AppHandle,
        req: &AiGenerateRequest,
    ) -> AppResult<(String, Usage)> {
        let (system, user) = structured::plain_prompt(req);
        chat::complete_impl(
            &req.model,
            &system,
            &user,
            req.temperature,
            Some(chat::StructuredCall {
                format: None,
                effort: req.effort.as_deref(),
                max_tokens: req.max_tokens,
                context_window: req.context_window,
            }),
        )
        .await
    }

    /// Native constrained decoding via Ollama's `format` field: the caller's
    /// JSON Schema verbatim when there is one, else `"json"` (valid-JSON only)
    /// — see [`structured::ollama_format`]. Applies to every local model:
    /// constrained decoding is enforced by the SERVER's sampler, not by model
    /// training, so unlike `tools` there is no per-family allowlist to gate on
    /// (`capabilities().supports_json_mode` is already `true` for all).
    ///
    /// `req.effort` rides along (gated by `chat::think_level`, the same family
    /// check `chat_stream` uses) for the same reason: this is a full
    /// [`AiGenerateRequest`], and a structured call that silently ran with
    /// thinking off was the user's setting being dropped, not honored. Same
    /// for `req.max_tokens`/`req.context_window` — a structured call is the
    /// one that carries a whole résumé plus a job ad, so dropping `num_ctx`
    /// here truncated exactly the prompts that need it most.
    async fn complete_structured(
        &self,
        _app: &AppHandle,
        req: &AiGenerateRequest,
        schema_hint: &str,
        schema: Option<&Value>,
    ) -> AppResult<(String, Usage)> {
        let (system, user) = structured::structured_prompt(req, schema_hint);
        chat::complete_impl(
            &req.model,
            &system,
            &user,
            structured::structured_temperature(self, req),
            Some(chat::StructuredCall {
                format: Some(structured::ollama_format(schema)),
                effort: req.effort.as_deref(),
                max_tokens: req.max_tokens,
                context_window: req.context_window,
            }),
        )
        .await
    }

    fn has_native_search(&self, _model: &str) -> bool {
        // Advertises web search for the family, but the MODEL never searches —
        // the hosted Web Search API does, through `native_searcher`.
        false
    }

    fn native_searcher(
        &self,
        app: &AppHandle,
        model: &str,
    ) -> Option<Box<dyn super::search::WebSearcher>> {
        OllamaSearcher::from_credentials(app, model)
            .map(|s| Box::new(s) as Box<dyn super::search::WebSearcher>)
    }

    async fn embed(&self, _app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>> {
        embed_with(model, text).await
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some(EMBED_MODEL)
    }

    async fn list_models(&self, _app: &AppHandle) -> AppResult<Vec<Value>> {
        models::fetch_tag_models().await
    }

    async fn test_key(&self, _app: &AppHandle) -> AppResult<()> {
        // Ollama needs no key — a reachable host counts as healthy.
        let client = crate::net::http::shared();
        match client
            .get(format!("{}/api/tags", host()))
            .timeout(timeouts::LIST_MODELS)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(AppError::Provider(format!(
                "Ollama returned status: {}",
                r.status()
            ))),
            Err(e) => Err(map_completion_transport_error(
                e,
                "Ollama",
                timeouts::LIST_MODELS,
            )),
        }
    }

    async fn chat_with_tools(
        &self,
        app: &AppHandle,
        model: &str,
        messages: &[ChatMsg],
        tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        self.chat_with_tools_impl(app, model, messages, tools, temperature)
            .await
    }
}

#[cfg(test)]
mod tests;
