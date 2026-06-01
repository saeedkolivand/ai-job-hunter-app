# ADR-009: Full factory reset via explicit lockstep store list in `privacy_reset_app`

**Status:** Accepted

## Context

A "factory reset" must wipe every persistent user-data store atomically. Two approaches were considered: a `Resettable` trait registry (stores self-register; reset iterates the registry) or an explicit maintained list. A registry requires each store to implement the trait and register at startup; it is automatic but adds indirection and requires discipline to keep registrations complete.

## Decision

`apps/tauri/src-tauri/src/commands/privacy.rs: privacy_reset_app` uses an **explicit list** of `clear_all()` calls covering every persistent store (PostingsCache, InteractionStore, JobTracker, DocumentStore, AiGenerationStore, ConversationDb, AutopilotStore, JobPreferencesStore, ContactProfileStore, KvCache, CredentialStore). A comment in the function body mandates keeping this list in lockstep with `commands/data.rs::build_bundle` (the backup set) plus stores intentionally excluded from backups. Adding a new persistent store means handling it in **both** places.

## Consequences

- The list is explicit and visible in one place (`privacy_reset_app`); no runtime registration machinery.
- Adding a new persistent store requires a deliberate edit to `privacy_reset_app` and `data.rs::build_bundle` — the lockstep comment is the enforcement mechanism.
- Reviewers (`rust-backend-architect`, `tauri-security-reviewer`) must check both files when a new store is added. Missing a store is a data-retention/GDPR risk (HIGH finding).
- A future refactor to a `Resettable` registry is not blocked by this decision but requires updating the ADR.
