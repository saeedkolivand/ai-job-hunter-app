---
name: rust-standards
description: Rust/Tauri backend standards — L0–L3 layers, centralized platform/net/error ownership, Rust-first business logic, registries, error handling. Load for changes under apps/tauri/src-tauri/src/**.
---

# Rust standards (CI-enforced via `cargo test --test architecture`)

Authoritative source: `docs/architecture-rules.md` + `docs/knowledge/architecture.md`.

## Hard rules (HIGH if violated — they already fail CI)

- **env access** only in `platform/` — `std::env::var` is banned elsewhere. Config/paths via `platform/config.rs` (`data_dir()`).
- **HTTP clients** only in `net/` — `reqwest::Client` is banned elsewhere. Use `net/http.rs` (`shared()` / `build_client()`).
- **typed errors** — untyped `Result<_, String>` is banned outside `error/`. Use `AppError`/`AppResult` from `error.rs`.

## Rust-first

Business logic, processing pipelines, ATS analysis, and document generation live in **Rust**. The TS renderer stays presentation-only. Flag any business logic drifting into the frontend.

## Layering (L0–L3)

Respect the layer model in `docs/architecture-rules.md`; new cross-layer coupling is HIGH. L0 platform/net/error → L1 domain → L2 services/commands → L3 entrypoints.

## Registries

New board scraper → `scraping/boards/mod.rs` (`SCRAPERS`, implement `Scraper`). New applier → `applying/registry/mod.rs` (`APPLIERS`, implement `Applier`). Register, don't special-case.

## Data

Migrations forward-safe and reversible-or-guarded; `*Store` writes go through the data layer, not ad-hoc SQL in commands. SQLite work off the async runtime (`spawn_blocking`).

## Observability

Use `observability.rs` (`Span`) for tracing; don't invent ad-hoc logging.
