# ADR-043 — Help chatbot over the shipped corpus: renderer-supplied entries, shipped retrieval module, dense arm behind the semantic-scoring opt-in

**Status:** Accepted

**Date:** 2026-09-02

**Deciders:** owner (re-opened the ADR-041 deferral), main session

## Context

[ADR-041](adr-041-searchable-help-page-over-compiled-in-entries.md) made the support page searchable over compiled-in, localized entries and decided explicitly against a chatbot: no model, no embeddings, no Rust command. It left one open as a possible later layer over the same entries.

The owner re-opened that deferral and, asked to choose, picked the hybrid design over the main session's keyword-only recommendation.

The design chosen — the parked August plan — rested on premises that had since drifted. The corpus no longer lives where that plan assumed: it sits in the translation bundles, in the shape ADR-041 shipped. The retrieval primitives it proposed to build already exist, as the module [ADR-039](adr-039-hybrid-postings-search-lexical-dense-rerank.md) shipped. And the classification rules for a command that can charge a paid provider are settled. So the question this record answers is not whether to build a chatbot, but what its contract is with the corpus, the retrieval layer and the spend rules.

## Decision

**1. The renderer supplies the corpus with every question.** The entries stay in the translation bundles under the support keys ADR-041 defined, and stay owned by the support page: Rust never reads a bundle and there is no build step or extraction. The active locale's entries travel with each question over the `help_search` contract. Every cap that contract states is re-checked in Rust, because a Tauri command is reachable from the agent CLI and the extension bridge, neither of which ever sees a schema. The caps themselves live on `HelpSearchRequestSchema` (`packages/shared/src/schemas/index.ts`) and are mirrored as named constants in `commands/help.rs`, where one pure `validate` makes each of them a unit test rather than a claim.

**2. Retrieval is the module ADR-039 shipped, not a second copy of it.** Three functions do all of the ranking: `retrieval::lexical::LexicalIndex` (entries adapted to its `LexicalDoc`), `retrieval::dense::rank_by_similarity`, and `retrieval::fusion::reciprocal_rank_fusion`. No keyword, similarity or fusion code is written for help search anywhere. The support page keeps its own client-side filter from ADR-041; that is a different surface, not a second ranker.

**3. The dense arm is gated on the same `semantic_scoring` preference that gates hybrid postings search**, read through one named predicate with exactly one call site (`semantic_on` in `commands/help.rs`), with missing state reading as off — the failure direction that spends nothing. Off means keyword-only and an honest `skipped` status; any embedding failure means keyword-only and `unavailable`, never an error surfaced to the user. The reply's mode is derived from the dense arm's status in one place, so the UI can never claim more ran than did. Reason: a search surface must not spend against a paid provider with no opt-in.

**4. One embedding-config snapshot per request feeds BOTH the charge and the embed** — the rule `documents/embedding.rs` already enforces for postings — reusing the provider daily-ceiling charge shape `hybrid_search::embed_or_cancel` builds.

**5. Entry vectors are cached by the text hash `sha256_hex` produces, never by entry id or locale.** The table is `help_vectors`, created by the migration that is the last entry of `DocumentStore::MIGRATIONS` (that list is position-indexed: append, never insert). Hashing the answer body makes the cache locale-agnostic and self-invalidating — an edited answer is a natural miss. The embedding-space check lives inside `get_help_vector`, against the caller's snapshot via `EmbeddingConfig::matches`, so a vector from another space or in an older vector format is a miss rather than a cross-space comparison. The store methods live in `documents/help_vectors.rs` rather than `documents/mod.rs` because the parent module sits near the R8 cap; they run on the same `DocumentStore` connection. The table is bounded three ways, because the entries are supplied by the REQUEST and an agent-tier caller could send arbitrary bodies: cache-miss embeds are capped per request by the constant beside the command, the table joins the same TTL and row-cap sweep as the posting vectors in `prune_caches`, and it is cleared whole by factory reset and by the space-changed branch of `ai_set_embedding_config` (next to posting vectors and match scores). The query vector is embedded per question and not cached. Two concurrent cold calls may each embed an entry once before the other's row lands (there is no in-flight registry); the cache converges and spend stays under the daily ceiling and the per-request cap.

**6. The command is an Irreversible agent-CLI policy row**, carrying the same spend proof source as the postings hybrid-search row, for the same reason: it can charge a provider. Not `Read`.

**7. The prompt renders entries as trusted app copy and fences everything else.** The question, the data glance and the history are untrusted input, fenced with every fence tag neutralised ([ADR-010](adr-010-untrusted-input-fencing.md)). Chat is session-only: nothing the model writes is persisted ([ADR-033](adr-033-no-model-written-agent-memory.md)).

**8. The reply carries entry ids and their fused scores only.** The renderer already holds the text it sent, so no copy of the corpus crosses back.

**9. The request types are generated from the schema; the result type is hand-written Rust.** The request side follows every other contract (`pnpm gen:ipc` → `ipc_contracts/help.rs`). The result type is written by hand in `commands/help.rs` because the generator would emit stringly-typed arm statuses; the statuses are instead the enum `commands::hybrid_search` already defines, since two enums that must serialize identically forever are drift waiting to happen.

**10. Logging is content-free by construction:** counts and enum tags only, never the question and never entry text, because the diagnostics bundle ships log files verbatim.

## Considered options

1. **Keyword-only in the renderer** — the main session's recommendation, and the cheapest thing that answers most questions. Rejected by the owner, who chose the hybrid design.

2. **Baking the bundles into the binary, or extracting them in a build step** — rejected: it ships the whole app's translations a second time, and adds build machinery for a corpus the renderer is already holding.

3. **A hand-rolled keyword or RRF implementation in Rust** — rejected: it duplicates ADR-039's tested module, and two rankers drift.

## Consequences

### Positive

- **Grounded, streamed answers over exactly the corpus the page already shows** — the model has no room to invent product behaviour.

- **The retrieval math is the one already proven on postings search**, so a fix there is a fix here.

- **The dense arm is opt-in**, and the surface degrades to keyword-only honestly rather than erroring or spending silently.

- **Nothing the model writes persists**: ADR-033 holds, and the only durable artefact is a cache of vectors the user can clear.

### Tradeoffs

- **No cancellation of an in-flight retrieval in v1**: a question is a single deliberate action with no supersede-on-keystroke shape behind it, so the command mirrors the postings path minus its cancellation token. The cost is that a first question against a cold cache runs to completion even if the user navigates away.

- **First-question latency on a cloud embedder**: entry vectors are computed on the first call and reused afterwards. A local embedder, or a warm cache, does not pay this.

- **The agent-CLI policy table module reached the R8 cap** and was split; its types now live in `extension_bridge/agent_cli/policy/types.rs`.

- **A locale change does not invalidate the cache** — the key is the answer text, so a translated answer is a different row rather than a replacement. Harmless: the cost is one re-embed per embedding space.

### What was verified, and how

- **Unit tests on the command and on the store**: each validation cap, the arm statuses and the mode derived from them, and the cache's hit, miss, space-mismatch and stale-format paths through a freshly migrated store.
- **A lexical eval over the real English bundle** (`apps/desktop/src-tauri/tests/help_retrieval.rs`): hand-written user phrasings must reach their entry in the lexical top results, and a separate test fails the eval if any case has fewer than two contenders — an uncontested case measures a grep, not a ranking.
- **Mutation checks**: swapping the column mapping in the cache read, and a cache read that ignores the embedding space, both fail their tests; so does removing the prompt's fence-tag neutralisation.
- **Not verified**: a live end-to-end answer in the running app, and the dense arm against a real embedding provider — both remain unexercised outside the keyword path.

## References

- Corpus: `packages/translations/src/locales/{en,de}/translation.json` (support keys)
- Retrieval module: `apps/desktop/src-tauri/src/retrieval/` (lexical, dense, fusion)
- Command: `apps/desktop/src-tauri/src/commands/help.rs`
- Wire contract: `HelpSearchRequestSchema` / `HelpSearchResultSchema` in `packages/shared/src/schemas/index.ts`
- Vector cache: `apps/desktop/src-tauri/src/documents/help_vectors.rs`
- Renderer generation: `apps/desktop/src/renderer/lib/generate/generation/generation.ts`
- Renderer hook: `apps/desktop/src/renderer/features/support/use-help-chat.ts`
- Component: `apps/desktop/src/renderer/features/support/components/HelpChat/index.tsx`
- Prompts: `packages/prompts/src/generate/help-chat/`
- Prior decisions: [ADR-039](adr-039-hybrid-postings-search-lexical-dense-rerank.md) (retrieval module), [ADR-041](adr-041-searchable-help-page-over-compiled-in-entries.md) (searchable page), [ADR-010](adr-010-untrusted-input-fencing.md) (fencing), [ADR-033](adr-033-no-model-written-agent-memory.md) (no persistence)
