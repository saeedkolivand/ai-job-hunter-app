# ADR-009: Full factory reset via a `Resettable` registry

Last updated: 2026-07-16

**Status:** Accepted (supersedes the original "explicit lockstep list" form of this ADR)

## Context

A "factory reset" must wipe **every** persistent user-data store. The original implementation used an explicit hand-maintained list of `clear_all()` calls inside `privacy_reset_app`, guarded only by a comment asking developers to keep it in sync. That list "silently rots": adding a new persistent store and forgetting to add a clear line leaves that store's data behind on a reset — precisely the data-retention/GDPR bug the reset exists to prevent. We want the guarantee to be structural, not a comment.

## Decision

Reset is **registry-driven**:

- A pure `Resettable { fn reset(&self); }` trait lives in the shared-infra layer (`apps/desktop/src-tauri/src/data_store.rs`) — a superset of `DataStore` that also covers things excluded from backups (secrets, ephemeral caches, the job log). It is Tauri-free so it stays below the shell layer (architecture rule R2).
- Each managed store **wrapper type** implements `Resettable` in `commands/privacy.rs` (e.g. `Mutex<PostingsCache>`, `Arc<Mutex<AutopilotStore>>`, bare `DocumentStore`/`KvCache`), dispatching to that store's own clear method.
- A `ResetRegistry` (managed Tauri state) holds type-erased reset closures. Stores are wired through `manage_resettable(app, &mut registry, label, store)` in `main.rs::setup`, which **manages the store and registers its reset in one call** — so the `.manage()` site is the single place a store is wired for reset coverage.
- `privacy_reset_app` resolves the `ResetRegistry` and calls `reset_all(&app)`; it contains **no per-store code**.

## Consequences

- **Adding a persistent store is covered by reset automatically** — managing it via `manage_resettable` registers its wipe; `privacy_reset_app` never changes. Registration is type-checked: `register::<T>()` requires `T: Resettable`, so a registered store provably clears.
- The reset command is a trivial loop over the registry (no growing hand-list to review on every new store).
- **Backups remain a separate concern.** `commands/data.rs::build_bundle` (the `DataStore` export set) is still an explicit list; this ADR governs _reset_, not backups. Reviewers still verify a new persistent store is added to `build_bundle` if it should be backup-able.
- **Residual gap (Tauri constraint):** Tauri state cannot be enumerated generically, so there is no compile-time check that _every managed store_ was registered — only that _registered_ stores are `Resettable`. The mitigation is the `manage_resettable` convention (use it instead of bare `app.manage` for persistent stores) plus tests covering the registry mechanism and representative `Resettable` impls (`commands/privacy.rs` tests).
- A new top-level module was **not** introduced: the trait is in `data_store.rs` (L0) and the registry/impls in `commands/privacy.rs` (L3), keeping Tauri coupling in the shell layer (R2) with no upward imports (R7).
