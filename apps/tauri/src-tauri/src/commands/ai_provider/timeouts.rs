//! Per-request HTTP timeouts for the AI provider adapters.
//!
//! Each constant is named by the *operation* it bounds, not by its raw value, so
//! a call site reads as `.timeout(timeouts::STREAM)` instead of a bare magic
//! number. Two sites share a constant only when they bound the **same operation**
//! at the **same duration**; same duration + different operation gets its own
//! constant so the values can drift independently later.
//!
//! Values are intentionally identical to the literals they replaced — this module
//! is a pure extraction with no behavior change.

use std::time::Duration;

// ── Chat generation ─────────────────────────────────────────────────────────────

/// Streaming chat completion (`chat_stream`): the long-running SSE/JSON stream a
/// cloud provider or the local Ollama daemon emits while generating.
pub const STREAM: Duration = Duration::from_secs(300);

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

// ── Model discovery & health ────────────────────────────────────────────────────

/// Listing models / validating a key (`list_models`, `test_key`): a quick GET to
/// the provider's model catalog or tags endpoint.
pub const LIST_MODELS: Duration = Duration::from_secs(10);

/// Local Ollama reachability probe (`reachable_model`): the fast health check
/// behind the system-health panel, kept short so an unreachable daemon fails fast.
pub const HEALTH: Duration = Duration::from_secs(3);

/// Inspecting a local Ollama model (`/api/show`): fetch a model's trained context
/// length and size labels.
pub const OLLAMA_SHOW: Duration = Duration::from_secs(15);

/// Pulling (downloading) a local Ollama model (`/api/pull`): a large multi-GB
/// download streamed with progress, hence the hour-long ceiling.
pub const MODEL_PULL: Duration = Duration::from_secs(3600);
