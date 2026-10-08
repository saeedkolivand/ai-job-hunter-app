//! The request one structured (JSON) provider call sends, and its output cap.

use crate::commands::ai_provider::AiGenerateRequest;

use super::completer::Completer;
use super::completion::text_request;
use super::resume::stages::letter_ahead::{one_request_at_a_time, Route};

/// Output cap for one structured (JSON) call, in tokens (#1391).
///
/// Applied ONLY where the runaway was observed and its cost is local: a server
/// that works one request at a time (Ollama, a local OpenAI-compatible server —
/// the notion `letter_ahead::one_request_at_a_time` already owns). There a
/// thinking model streamed 18k+ tokens (a small one 144k) until the stream
/// ceiling, and every other call in the app queued behind it. Cloud providers
/// are left as before: OpenAI reasoning models and Gemini 2.5 count their
/// reasoning tokens against the cap, so a cap there could cut a healthy answer,
/// and a runaway costs a bill rather than a blocked machine. Anthropic already
/// bounds itself (`adaptive_max_tokens`); CLI agents expose no such lever.
///
/// The legitimate outputs are small: the analysis and strategy schemas cap their
/// lists at a handful of short items (~1-2k tokens of JSON). qwen3-style models
/// count THINKING tokens against `num_predict` too, so 8192 leaves several times
/// that for a reasoning pass, yet bounds a runaway to minutes.
pub(crate) const STRUCTURED_MAX_TOKENS: u32 = 8192;

/// The request one structured call sends. Temperature is deliberately absent
/// (the structured path resolves it from the provider's sampling profile); the
/// configured context window is not — a structured call reads the same
/// oversized artifacts every other stage does. `effort` rides along so the
/// provider's per-call deadline (`ollama_completion_deadline`) scales by it,
/// like a streamed stage's.
pub(crate) fn structured_request(
    route: Route<'_>,
    model: &str,
    system: &str,
    user: &str,
    context_window: Option<u32>,
    effort: Option<&str>,
) -> AiGenerateRequest {
    text_request(
        model,
        system,
        user,
        None,
        one_request_at_a_time(route).then_some(STRUCTURED_MAX_TOKENS),
        context_window,
        effort,
    )
}

impl Completer {
    /// [`structured_request`] for this completer's own route and window.
    pub(super) fn structured_req(
        &self,
        system: &str,
        user: &str,
        effort: Option<&str>,
    ) -> AiGenerateRequest {
        let route = Route {
            provider: self.provider.id(),
            base_url: self.base_url.as_deref(),
        };
        structured_request(
            route,
            &self.model,
            system,
            user,
            self.context_window,
            effort,
        )
    }
}
