# ADR-019: Resolved performance profile with real backend tiers

Last updated: 2026-06-14

**Status:** Accepted

## Context

Performance mode evolved from a simple string enum (`'low-memory' | 'balanced' | 'performance'`) to a richer feature where users can customize per-element visuals independently and select from three backend scaling tiers. The underlying architecture faced two challenges: (1) string-based mode switching scattered enum variants across the renderer and backend, making it fragile when new modes were added; (2) the Ollama keep-alive knob (needed to tune inference idle time for local models) had no way to reach the embed builder, a free function with no `AppHandle` access.

## Decision

Resolve all performance modes to a single unified `PerformanceProfile` (the truth on the frontend) that feeds a concrete `PerformanceBackendConfig` IPC payload sent to the Rust shell. The backend holds a process-global, init-order-safe runtime config cell. Frontend selects a tier (or custom) → resolves to concrete numbers → sends to backend → backend routes to the subsystems that consume it.

### Architecture

**Frontend (resolver):**

- `PerformanceProfile` (TypeScript interface) models a user's selection: display tier + backend tier + custom overrides.
- `resolveBackendConfig(mode, profile)` pure function maps the profile to a concrete `PerformanceBackendConfig` struct with numeric knobs. See `apps/tauri/src/renderer/store/preferences-schema/preferences-schema.ts` lines 274–285.
- The function consults tier-mapping tables (`CONCURRENCY_BY_TIER`, `KEEP_ALIVE_SECS_BY_TIER`, `CACHE_TTL_SECS_BY_TIER`, `CACHE_MAX_ROWS_BY_TIER`) defined in the same file.

**IPC contract:**

- `PerformanceBackendConfig` (shared type in `packages/shared/src/types/index.ts`) is the struct sent over `system_set_performance_mode({ config })`.
- Fields: `mode` (informational label), `concurrency`, `keepAliveSecs`, `cacheTtlSecs`, `cacheMaxRows`.
- Never branch on the string `mode`; the numeric knobs are the source of truth for backend behavior.

**Backend (L0 runtime config):**

- `performance.rs` (new L0 module) holds a process-global `OnceLock<ArcSwap<PerformanceConfig>>` initialized to balanced defaults.
- Two functions: `current()` returns a lock-free snapshot of the live config; `set(cfg)` replaces it (called by the IPC handler).
- `ollama_keep_alive()` public function returns the wire value ("0" or "{secs}s") for the live config, called by the Ollama adapter (`commands/ai_provider/ollama.rs`).
- Keep-alive and cache bounds are read-side opt-in: any subsystem needing them calls `performance::current()` directly (no AppHandle required).

**Subsystems:**

- **Scraper concurrency:** `ScraperEngine::set_concurrency(n)` receives the clamp'd value from the IPC handler.
- **Ollama keep-alive:** The embed builder (free function, no AppHandle) calls `performance::ollama_keep_alive()` to read the live value and write it as a top-level request field.
- **Cache bounding:** Two SQLite tables (`match_scores`, `posting_vectors` in `documents.db`) query `performance::current()` to enforce TTL + row-cap eviction. See `documents/mod.rs` cache eviction sites.

### The 4th mode: custom

Users may opt into "custom" mode to tune each backend tier independently (not just visual preferences). The tier-mapping tables have a fallback entry for any custom values. The `resolveBackendConfig` function is the ONLY place where custom values override the tier defaults.

### Tier definitions

Tiers are defined by their numeric targets (concurrency, keep-alive, cache TTL, cache row cap). As of v0.102.0, the tiers are:

| Tier        | Concurrency | Keep-alive (s) | Cache TTL (d) | Cache rows |
| ----------- | ----------- | -------------- | ------------- | ---------- |
| low-memory  | 2           | 30             | 1             | 250        |
| balanced    | 4           | 300            | 7             | 2000       |
| high (perf) | 8           | 1800           | ∞ (unbounded) | ∞          |

To find the exact numbers and update them, see the tier-mapping tables in `apps/tauri/src/renderer/store/preferences-schema/preferences-schema.ts` (lines 257–268).

## Consequences

### Clarity and maintainability

- The `PerformanceProfile` abstraction decouples user choices (tier name, visual preference, custom overrides) from backend implementation.
- The IPC contract is numeric and never branches on string mode.
- Backend subsystems read `performance::current()` independently; no fragile mode-switch dispatcher.

### Scale and extensibility

- A new backend knob (e.g., batch size, timeout) requires: (1) add field to `PerformanceConfig` in Rust; (2) add field to `PerformanceBackendConfig` IPC struct; (3) add tier values to the resolver tables; (4) update the subsystem that consumes it.
- Users in custom mode can immediately tune the new knob without app changes.

### Init-order safety

- The Ollama embed builder (invoked from AI generation calls, no AppHandle in scope) reads `performance::current()` via the process-global cell. The cell initializes to balanced defaults on first read (via `get_or_init`), guaranteeing a safe value even before the IPC command fires.
- Lock-free reads via `ArcSwap::load_full()` mean concurrent reads never block each other, and there is no init-order dependency between subsystems.

### Cache self-invalidation

- Both cache tables (`match_scores`, `posting_vectors`) encode mode-sensitive inputs in their primary key or TTL, so a tier change immediately creates cache misses. No explicit cache flush needed. See ADR-017 for the full caching story.

### IPC trust boundary

- The command handler (`system_set_performance_mode`) clamps concurrency to [1, 16] and coerces cache bounds non-negative at the Rust boundary, even though today's renderer only sends fixed tiers. This guards against a buggy or future-variant renderer sending extreme values.

## Trade-offs Evaluated

### String enum vs. resolved struct

**Chosen:** Resolved struct.

- String enums scatter variants across two language runtimes and require careful synchronization (enum drift is easy to miss in CI).
- A resolved struct with explicit tier tables is a single source of truth: one place to change, one place to test, one place to document.
- The renderer still has the enum for UI purposes; the backend never sees it.

### Process-global `OnceLock<ArcSwap>` vs. AppHandle-threaded config

**Chosen:** Process-global.

- Threaded config would require an `AppHandle` in every subsystem that reads performance knobs, violating L0 independence (L0 modules cannot hold Tauri state).
- The Ollama embed builder is a free function with no AppHandle access; the only way to reach it is a process-global cell.
- The `OnceLock` pattern is common in Rust libraries (e.g., log levels, tracing config) and is safe because init is idempotent (all threads converge to the same default or the explicitly-set value).

### Cache TTL + row cap vs. tiered record deletion

**Chosen:** Both.

- A single eviction strategy (TTL only, or cap only) is fragile: cache rows could live past their semantic usefulness, or accumulate indefinitely during heavy scraping.
- Composite bounding (TTL AND row cap) with a one-shot lazy-on-upsert eviction ensures both freshness and bounded size.

### Ollama keep-alive placement (top-level field vs. options)

**Chosen:** Top-level field, assigned AFTER any `body["options"]` writes.

- Ollama API spec places `keep_alive` at the request root, not nested under `options`.
- Writing it after options are set prevents downstream code from accidentally clobbering it.
- Cloud providers (OpenAI, Anthropic, Claude-via-API) do not support `keep_alive` and must not receive it; the Ollama adapter is the only place that writes it.

## References

- **Config cell:** `apps/tauri/src-tauri/src/performance.rs` — the L0 module.
- **Tier-mapping tables:** `apps/tauri/src/renderer/store/preferences-schema/preferences-schema.ts` lines 257–268.
- **Resolver function:** `apps/tauri/src/renderer/store/preferences-schema/preferences-schema.ts` lines 274–285 (`resolveBackendConfig`).
- **IPC command handler:** `apps/tauri/src-tauri/src/commands/system/mod.rs` (` system_set_performance_mode`, lines 177–199+).
- **Scraper concurrency:** `apps/tauri/src-tauri/src/scraping/engine/mod.rs` (`set_concurrency`).
- **Ollama keep-alive:** `apps/tauri/src-tauri/src/commands/ai_provider/ollama.rs` (embed builder calls `performance::ollama_keep_alive()`).
- **Cache eviction:** `apps/tauri/src-tauri/src/documents/mod.rs` (cache sites query `performance::current()`).
- **UI provider:** `apps/tauri/src/renderer/providers/PerformanceModeProvider/PerformanceModeProvider.tsx`.
- **Settings UI:** `apps/tauri/src/renderer/features/settings/components/preferences/PerformancePreferences/index.tsx`.
- **Cinematic background:** `apps/tauri/src/renderer/components/background/CinematicBackground/index.tsx` (consumes display tier to gate visual layers).
- **Related ADR:** ADR-017 (persisted self-invalidating match-score caches).
