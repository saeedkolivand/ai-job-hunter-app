# Dependency map (pointer)

Last updated: 2026-06-01

For the live graph use **graphify** — `graphify query "what depends on <X>"`, `graphify path "<A>" "<B>"`, or `graphify-out/GRAPH_REPORT.md` for hubs/communities. This file only fixes the **boundary rules** and where manifests live.

## Package boundaries (must not be violated)

- `packages/shared` → depended on by everything; itself depends on **nothing app-specific** (no [React][react], no Node, no `window`).
- `packages/ui` → no [Zustand][zustand], no IPC, no routing, no app logic.
- `packages/prompts` → pure [TypeScript][typescript], **zero deps**, no UI, no `window`.
- `packages/translations` → i18next singleton + adapters; **no app-specific or IPC imports** inside the package (renderer couples via thin shim `@/i18n` that owns the listener logic).
- `apps/tauri` renderer → backend **only** via `AppClient`/service hooks (never `window.api.*` in features/routes/components).

## Layer boundaries (Rust, CI-enforced)

`std::env::var` only in `platform/`; `reqwest::Client` only in `net/`; untyped `Result<_,String>` only in `error/`. Cross-layer coupling beyond L0→L1→L2→L3 is a HIGH finding. Source: `docs/architecture-rules.md` + the `cargo test --test architecture` guard.

## Manifests (where to look)

- Rust: `apps/tauri/src-tauri/Cargo.toml` / `Cargo.lock`; workspace `Cargo.toml`. Supply-chain policy: `deny.toml` (`cargo deny check`, `cargo audit`).
- JS: root `package.json` + `pnpm-lock.yaml`; per-package `package.json` (`packages/*`, `apps/tauri`). Audit: `pnpm audit` + dependency-review (CI).
- Tauri: `apps/tauri/src-tauri/tauri.conf.json`, `apps/tauri/src-tauri/capabilities/default.json`.

## Registries (the extension points)

- Scrapers — `scraping/boards/mod.rs` (`SCRAPERS`, `Scraper` trait).
- AI providers — `commands/ai_provider/` (one file per provider behind a shared interface; adding a provider = new adapter only).

> No applier registry: the auto-apply engine was removed (the app is an apply **assistant**).

> Drift note: dependency counts/versions live in the manifests — never copy them here; point at the manifest.

[react]: https://react.dev
[zustand]: https://github.com/pmndrs/zustand
[typescript]: https://www.typescriptlang.org
