---
name: rust-standards
description: Rust/Tauri backend standards — L0–L3 layers, centralized platform/net/error ownership, Rust-first business logic, registries, error handling. Load for changes under apps/desktop/src-tauri/src/**.
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

New board scraper → `scraping/boards/mod.rs` (`SCRAPERS`, implement `Scraper`). Register, don't special-case. (No applier registry — the apply engine was removed.)

## Data

Migrations forward-safe and reversible-or-guarded; `*Store` writes go through the data layer, not ad-hoc SQL in commands. SQLite work off the async runtime (`spawn_blocking`).

## Observability

Use `observability.rs` (`Span`) for tracing; don't invent ad-hoc logging.

## Platform-gated code & external processes (#486 lessons)

- **`#[cfg(target_os = …)]` is not compiled on other hosts** — a Windows/macOS dev build silently excludes the Linux module + its tests, so a same-host `cargo test`/`clippy` can't catch a Linux-only unused import, an attribute/`use` not in scope (a `use` is **not** inherited into a `mod` submodule), or a `pub(super)` visibility error. Cross-target-check gated code (`cargo check --target x86_64-unknown-linux-gnu`, or a dep-light standalone-crate check) before relying on a green build.
- **Global env/process mutations: scope + roll back** — a `set_var`/`LD_PRELOAD`/re-exec must be scoped to the exact case that needs it (not every launch) and reverted on the failure path, so the still-running process and its children don't inherit bogus state. Env access still lives only in `platform/`.
- **External processes must be bounded** — a `Command` probe must enforce a real timeout (`wait_timeout` + kill on expiry) when it claims to; a missing-binary or slow external tool must not block detection/startup.

## External standards & best-practices (verified 2026-06-19)

> Latest stable **Rust 1.95**; **Edition 2024** default since 1.85 (2025-02-20). Set `edition = "2024"` + pin MSRV (`rust-version`). Re-verify version-pinned items periodically.

- **API Guidelines** — conversion naming (`as_`/`to_`/`into_`), `iter`/`iter_mut`/`into_iter`, eager `Debug, Clone, Eq, Hash`, `#[non_exhaustive]` on extensible public enums/structs, no private types in public signatures. https://rust-lang.github.io/api-guidelines/checklist.html
- **Errors** — libraries: `thiserror` typed enums, preserve the chain (`#[source]`/`#[from]`, `#[error(transparent)]`); apps: `anyhow`/`eyre` + `.context(...)`. Never expose `anyhow::Error` in a library's public API. https://docs.rs/thiserror
- **async/tokio** — never block an async worker; offload blocking/CPU work via `spawn_blocking` (prefer over `block_in_place`); `tokio::spawn` futures need `Send + 'static`; bound parallel `spawn_blocking` with a `Semaphore`. https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
- **Cancel-safety** — every `.await` is a cancellation point; only hold across-await invariants that survive a drop; in `select!` use only cancel-safe futures; graceful shutdown via `CancellationToken` + cleanup. https://tokio.rs/tokio/topics/shutdown
- **Clippy** — enforce `clippy::all` + `clippy::pedantic` (`-D warnings`), allow-list rejected pedantic lints, set `msrv` in `clippy.toml`. https://doc.rust-lang.org/clippy/
- **unsafe** — FFI / validated raw-pointer-or-layout / proven hot-path only; every block carries a `// SAFETY:` note; keep minimal.
- **Supply chain** — `cargo audit`/`cargo deny` in CI vs https://rustsec.org/ . 2026 advisory **RUSTSEC-2026-0098** (`rustls-webpki < 0.103.12`, URI name-constraint bypass) — bump transitively if any TLS path pulls it.

**Common mistakes:** blocking the runtime (sync I/O, `std::thread::sleep`, heavy CPU) in async; `.unwrap()`/`.expect()` on fallible non-test paths; stringly errors dropping the source chain; `anyhow` in a public library API; holding a `Mutex`/`RefCell` guard across `.await`; assuming a `select!` branch completed (may be cancelled); unbounded `spawn_blocking` fan-out; `unsafe` without `SAFETY`.
