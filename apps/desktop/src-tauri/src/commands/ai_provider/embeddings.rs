//! Embeddings: the space-tagged vector types + [`embed_text`], the shared
//! chokepoint for AI-spend visibility on embedding calls. Split out of
//! `mod.rs` (R8 line-budget split).

use serde::Serialize;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};

use super::embed::{embed_adaptive, MeteredAttempt, ProviderEmbedAttempt};
use super::{record_usage, resolve, ProviderId, Usage};

/// Vector-FORMAT version — bumped whenever the ALGORITHM that produces a
/// stored vector's VALUES changes for the same `(provider, model, dim)`, even
/// though the provider/model IDENTITY is unchanged (e.g. replacing a naive
/// single truncation with chunk-and-mean-pool — same tag, semantically
/// different vector). `EmbeddingConfig::matches` checks this so a vector
/// persisted before a bump is treated as stale and re-embedded, instead of
/// being silently compared against a new-format vector under the identical
/// `(provider, model, dim)` tag.
pub const EMBEDDING_VECTOR_VERSION: i64 = 2;

/// The identity of an embedding "space": vectors are only comparable when they
/// share the same `(provider, model, dim)` AND the same [`EMBEDDING_VECTOR_VERSION`]
/// they were produced under. Stored alongside every vector so incompatible —
/// or differently-produced — vectors can never be silently mixed. `version`
/// is a storage-format detail, not part of the wire shape (`#[serde(skip)]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSpace {
    pub provider: String,
    pub model: String,
    pub dim: usize,
    #[serde(skip)]
    pub version: i64,
}

impl std::fmt::Display for EmbeddingSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}@{}", self.provider, self.model, self.dim)
    }
}

/// A vector tagged with the space it was produced in.
#[derive(Debug, Clone)]
pub struct EmbeddingVector {
    pub values: Vec<f64>,
    pub space: EmbeddingSpace,
}

/// Embed `text` with an explicit provider/model, returning a space-tagged vector.
/// Routes through the same `resolve` + capability + auth flow as chat, so there
/// are no Ollama assumptions and no silent fallback.
///
/// This is the shared chokepoint for AI-spend visibility on embedding calls —
/// every caller (`ai_embed`, `posting_vector_or_embed`'s match-score
/// resolution, `ai_reembed_all`'s batch re-index) routes through here, so
/// each records the provider's REAL reported token usage (zero when a
/// provider genuinely reports none) with no changes needed at any call site.
///
/// `charge` metes the per-provider daily budget once per ACTUAL round-trip
/// via [`MeteredAttempt`] — see its doc. `None` (every caller but `ai_embed`)
/// is unaffected, unchanged from before this parameter existed.
pub async fn embed_text(
    app: &AppHandle,
    provider: ProviderId,
    model: &str,
    base_url: Option<String>,
    text: &str,
    charge: Option<&(dyn Fn() -> AppResult<()> + Send + Sync)>,
) -> AppResult<EmbeddingVector> {
    let client = resolve(provider, base_url.clone());
    let model = if model.trim().is_empty() {
        client
            .default_embedding_model()
            .ok_or_else(|| {
                // Distinct from the capability message below on purpose: these
                // are two different problems and used to be indistinguishable.
                // This one means "we don't presume a default model for this
                // provider" (every OpenAI-compatible gateway — its catalog is
                // its own), which the user fixes by PICKING one. The other
                // means the provider has no embeddings API at all, which they
                // can't fix by choosing anything.
                AppError::Config(format!(
                    "No default embedding model for {}. Choose one in Settings → AI → Embeddings.",
                    provider.as_str()
                ))
            })?
            .to_string()
    } else {
        model.to_string()
    };
    if !client.capabilities(&model).supports_embeddings {
        return Err(AppError::Config(format!(
            "{} does not support embeddings.",
            provider.as_str()
        )));
    }
    // Cap the input to the provider's real limit, char-boundary-safe, then
    // adaptively retry on a context-length overflow — see `embed_adaptive`.
    // Applied here so every provider is consistent and a new one inherits a
    // safe default — see `AiProvider::max_embedding_input_chars`.
    let initial_cap = client.max_embedding_input_chars();
    let attempt = ProviderEmbedAttempt {
        app,
        client: client.as_ref(),
        model: &model,
    };
    // `usage` accumulates as `embed_adaptive` runs, even if it ultimately
    // errors (a multi-chunk document can bill several real provider calls
    // before failing on a later one) — record whatever was actually billed
    // BEFORE propagating the error, so a partial failure never silently
    // drops already-spent tokens from the ledger.
    let metered = MeteredAttempt {
        inner: &attempt,
        charge,
    };
    let mut usage = Usage::default();
    let result = embed_adaptive(&metered, text, initial_cap, &mut usage).await;
    record_usage(app, provider.as_str(), &model, usage, base_url.as_deref());
    let values = result?;
    if values.is_empty() {
        return Err(AppError::Provider(format!(
            "{} returned an empty embedding.",
            provider.as_str()
        )));
    }
    let dim = values.len();
    Ok(EmbeddingVector {
        values,
        space: EmbeddingSpace {
            provider: provider.as_str().to_string(),
            model,
            dim,
            version: EMBEDDING_VECTOR_VERSION,
        },
    })
}

/// Cosine similarity between two vectors that MUST share an embedding space.
/// Returns `Err` on a space mismatch — incomparable vectors are never silently
/// scored (the old behavior returned 0.0 and hid the bug).
pub fn compare(a: &EmbeddingVector, b: &EmbeddingVector) -> AppResult<f64> {
    if a.space != b.space {
        return Err(AppError::Validation(format!(
            "refusing to compare embeddings from different spaces: {} vs {}",
            a.space, b.space
        )));
    }
    Ok(super::cosine(&a.values, &b.values))
}
