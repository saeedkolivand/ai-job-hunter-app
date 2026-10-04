use super::*;

/// A translator that rewrites the JD (German → English, the real cross-language
/// case) and an embedder that records exactly which bytes it was asked to embed.
pub(super) struct FakeScoreIo {
    /// `raw job text → translated job text`. A miss returns the text unchanged.
    translations: std::collections::HashMap<String, String>,
    /// Every text an ACTUAL round-trip was made for, in order.
    embedded: Mutex<Vec<String>>,
    space: crate::commands::ai_provider::EmbeddingSpace,
    /// Models an offline provider / a failed round-trip: every `embed_one`
    /// call is still recorded (a real attempt was made) but returns `None` —
    /// the same signal a genuine provider failure gives `score_one`.
    embed_fails: bool,
}

impl FakeScoreIo {
    pub(super) fn new(store: &DocumentStore, translations: &[(&str, &str)]) -> Self {
        let active = store.embedding_config();
        Self {
            translations: translations
                .iter()
                .map(|(from, to)| ((*from).to_string(), (*to).to_string()))
                .collect(),
            embedded: Mutex::new(Vec::new()),
            space: crate::commands::ai_provider::EmbeddingSpace {
                provider: active.provider,
                model: active.model,
                dim: 3,
                version: EMBEDDING_VECTOR_VERSION,
            },
            embed_fails: false,
        }
    }
    /// Every embed round-trip this fake is asked for fails (`None`) — the
    /// degrade path: semantic scoring was requested but no embedding was
    /// available for the pair.
    pub(super) fn failing(mut self) -> Self {
        self.embed_fails = true;
        self
    }
    pub(super) fn embedded(&self) -> Vec<String> {
        self.embedded.lock().clone()
    }
    pub(super) fn vector(&self) -> EmbeddingVector {
        EmbeddingVector {
            values: vec![0.1, 0.2, 0.3],
            space: self.space.clone(),
        }
    }
}

#[async_trait::async_trait]
impl Embedder for FakeScoreIo {
    async fn embed_one(&self, text: &str) -> Option<EmbeddingVector> {
        self.embedded.lock().push(text.to_string());
        if self.embed_fails {
            return None;
        }
        Some(self.vector())
    }
}

#[async_trait::async_trait]
impl ScoreIo for FakeScoreIo {
    async fn translate(&self, _job_id: &str, text: String, _target_lang: &str) -> String {
        self.translations.get(&text).cloned().unwrap_or(text)
    }
}

/// Counts charges against the shared daily ceiling. `affordable: false` models
/// the ceiling already being reached (every charge refused).
pub(super) struct CountingBudget {
    charges: std::sync::atomic::AtomicUsize,
    affordable: bool,
}

impl CountingBudget {
    pub(super) fn new() -> Self {
        Self {
            charges: std::sync::atomic::AtomicUsize::new(0),
            affordable: true,
        }
    }
    pub(super) fn exhausted() -> Self {
        Self {
            affordable: false,
            ..Self::new()
        }
    }
    pub(super) fn charges(&self) -> usize {
        self.charges.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl crate::documents::EmbedBudget for CountingBudget {
    fn charge_one_embed(&self) -> crate::error::AppResult<()> {
        self.charges
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.affordable {
            Ok(())
        } else {
            Err(crate::error::AppError::RateLimited("daily ceiling".into()))
        }
    }
}

pub(super) const GERMAN_JD: &str =
    "Wir suchen einen erfahrenen Rust-Entwickler mit Kubernetes-Erfahrung \
                         für den Aufbau verteilter Systeme in Berlin.";
pub(super) const ENGLISH_JD: &str =
    "We are looking for an experienced Rust developer with Kubernetes \
                          experience to build distributed systems in Berlin.";
pub(super) const RESUME_TEXT: &str =
    "Experienced Rust developer. Kubernetes, Postgres, distributed systems.";

pub(super) fn scoring_store() -> (tempfile::TempDir, DocumentStore) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    (temp_dir, store)
}

/// A stored résumé with every optional field at its default (no locale, unindexed,
/// not the default document, no cached keywords). A test that cares about one of
/// them overrides it at the site with struct-update syntax.
pub(super) fn resume_doc(id: &str, text: &str) -> DocumentRecord {
    DocumentRecord {
        id: id.into(),
        title: String::new(),
        name: String::new(),
        locale: None,
        text: text.into(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    }
}

/// Run the kernel for `resume` against `job_text` under the store's active
/// embedding config: no cached keyword list and no budget (the interactive and
/// the plain keyword-only shape).
pub(super) async fn score(
    io: &FakeScoreIo,
    store: &DocumentStore,
    resume: &DocumentRecord,
    job_id: &str,
    job_text: &str,
    semantic_enabled: i64,
    surface: MatchSurface,
) -> Value {
    let active = store.embedding_config();
    score_one(
        io,
        store,
        resume,
        None,
        &active,
        job_id,
        Some(job_text.to_string()),
        semantic_enabled,
        surface,
        None,
    )
    .await
}

/// Run the Autopilot surface's kernel exactly as `score_autopilot_semantic`
/// does, with the two provider seams faked.
pub(super) async fn score_autopilot(
    io: &FakeScoreIo,
    store: &DocumentStore,
    budget: &CountingBudget,
    job_id: &str,
    raw_job_text: &str,
) -> Value {
    let resume = autopilot_resume_record(RESUME_TEXT);
    let active = store.embedding_config();
    score_one(
        io,
        store,
        &resume,
        None,
        &active,
        job_id,
        Some(raw_job_text.to_string()),
        1,
        MatchSurface::Autopilot,
        Some(budget),
    )
    .await
}

pub(super) fn seed_posting_vector(
    store: &DocumentStore,
    io: &FakeScoreIo,
    job_id: &str,
    text: &str,
) {
    store
        .upsert_posting_vector(job_id, &sha256_hex(text), &io.vector())
        .unwrap();
}

/// A vector in the ACTIVE provider/model/version but a DIFFERENT dimensionality
/// than [`FakeScoreIo::vector`] — the shape an OpenAI-compatible `base_url`
/// switch leaves behind. `EmbeddingConfig::matches` compares provider + model +
/// version and never `dim`, so such a row is a cache HIT; `EmbeddingSpace`'s
/// `PartialEq` DOES include `dim`, so the pair is incomparable at `compare()`.
pub(super) fn stale_space_vector(store: &DocumentStore, dim: usize) -> EmbeddingVector {
    let active = store.embedding_config();
    EmbeddingVector {
        values: vec![0.5; dim],
        space: crate::commands::ai_provider::EmbeddingSpace {
            provider: active.provider,
            model: active.model,
            dim,
            version: EMBEDDING_VECTOR_VERSION,
        },
    }
}

/// A vector in the ACTIVE embedding space with caller-chosen values, so a test
/// can seed a pair whose cosine — and therefore the kernel's `semantic` number
/// — is known in advance instead of inheriting [`FakeScoreIo::vector`]'s
/// identical-on-both-sides 1.0.
pub(super) fn vector_of(store: &DocumentStore, values: [f64; 3]) -> EmbeddingVector {
    let active = store.embedding_config();
    EmbeddingVector {
        values: values.to_vec(),
        space: crate::commands::ai_provider::EmbeddingSpace {
            provider: active.provider,
            model: active.model,
            dim: 3,
            version: EMBEDDING_VECTOR_VERSION,
        },
    }
}

/// The semantic cache key of one autopilot job, as the kernel writes it.
pub(super) fn semantic_key<'a>(
    resume_id: &'a str,
    job_id: &'a str,
    active: &'a EmbeddingConfig,
    job_text_hash: &'a str,
) -> MatchScoreKey<'a> {
    MatchScoreKey {
        resume_id,
        job_id,
        provider: &active.provider,
        model: &active.model,
        semantic_enabled: 1,
        formula_version: MATCH_FORMULA_VERSION,
        vector_version: EMBEDDING_VECTOR_VERSION,
        job_text_hash,
    }
}
