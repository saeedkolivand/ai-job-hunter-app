# Architecture Rules — Rust/Tauri Core

Last updated: 2026-06-01

> **Status:** enforceable rules (Phase 2), derived from
> [`architecture-analysis.md`](architecture-analysis.md) — the **actual** structure of
> `apps/tauri/src-tauri/`, not a generic Clean-Architecture template. These rules are
> machine-enforced by `apps/tauri/src-tauri/tests/architecture.rs`
> (`cargo test --test architecture`) plus `cargo fmt`/`clippy`/`deny`/`audit`/`machete`
> in CI. The prose principles live in `docs/PATTERNS.md` §13; this file is the formal,
> testable contract.

## The layer model

The crate has four layers. **Dependencies flow downward only** (a higher layer may use a
lower layer; a lower layer may never use a higher one). Layer = the first path segment of
a module under `src/`.

```
L3  Shell / IPC        commands, ipc_contracts, main, updater, tray, deeplink
L2  Application        pipeline, cover_letter, autopilot, autopilot_scheduler,
                       autopilot_helpers, recommend
L1  Domain             scraping, extraction, export, documents, jobs, postings,
                       conversations, credentials, job_preferences, ai_generations,
                       profile_import, model, layout, measure, validate, locale, theme
L0  Shared infra       error, observability, db, data_store, net, platform, browser
```

> This list is the single source of truth and is duplicated verbatim as the `LAYER`
> table at the top of `tests/architecture.rs`. Adding a top-level module **requires**
> adding it to both, or the arch test fails with "unclassified module".

---

## Per-layer contract

### L0 — Shared infrastructure (`error`, `observability`, `db`, `data_store`, `net`, `platform`, `browser`)

- **Allowed deps:** other L0 modules only.
- **Forbidden deps:** L1, L2, L3. **No** `crate::commands`, `crate::scraping`, etc.
- **Public API:** each module's `mod.rs`/file root. Use the named owner functions:
  `platform::config::data_dir()`, `net::http::shared()` / `build_client()`,
  `error::{AppError, AppResult}`, `observability::Span`, `db::open()`,
  `data_store::DataStore`.
- **Internal-only:** submodule helpers stay `pub(crate)` or private. `net::http`'s client
  builder internals are not re-exported.
- **Ownership:** these modules are the **sole owners** of their concern (see table below).
- **Documented exception (W-9):** `error` depends on `crate::extraction` for its
  `From<DomainError>` impl. Allowlisted; the alternative (moving the `From` impl into the
  domain module) is deferred. No new L0→L1 edges permitted.

### L1 — Domain (`scraping`, `extraction`, `export`, `documents`, …, `model`, `layout`, `measure`, `validate`, `locale`, `theme`)

- **Allowed deps:** L0 + sibling L1 modules (via their public `mod.rs` surface).
- **Forbidden deps:** L2, L3. **No Tauri** — no `tauri::`, `tauri_plugin_*`, `AppHandle`,
  `tauri::State`, `Manager`, `.emit(`, `#[tauri::command]`.
- **Public API:** the module root (`mod.rs`). Registry modules expose their list +
  trait: `scraping::boards::SCRAPERS` (`Scraper`). The rendering cluster (`export`/`model`/`layout`/`measure`/`theme`/
  `validate`/`locale`) is mutually cohesive (intra-L1; permitted).
- **Internal-only:** board/provider/parser impls are `pub(crate)`; only the registry +
  trait are public across modules.
- **DB access:** a domain module that persists state owns its [SQLite][sqlite] store **in its own
  `mod.rs`** via [rusqlite][rusqlite] + `db::open()`. Logic/helper files must not touch SQL.
- **Documented exceptions (W-1, W-3):** see the allowlist tables below — provider
  reach-ups and Tauri `emit`/`AppHandle` usage that exist today are grandfathered with
  `TODO(arch)`; **no new instances allowed**.

### L2 — Application / orchestration (`pipeline`, `cover_letter`, `autopilot*`, `recommend`)

- **Allowed deps:** L0 + L1 + sibling L2.
- **Forbidden deps:** L3 (`crate::commands`, etc.). **No `#[tauri::command]`** (handlers
  belong in L3). Direct SQL and `reqwest::Client` construction are forbidden (compose
  domain stores + `net::http`).
- **Public API:** `pipeline` exposes `Stage`/`Pipeline`; orchestrators expose their
  run/schedule entry points.
- **Documented exceptions:** `pipeline`, `documents`-adjacent flows, and `cover_letter`
  use `AppHandle.emit()` for **streaming progress** to the renderer;
  `autopilot_scheduler` calls `crate::commands::autopilot::autopilot_run`;
  `pipeline`/`documents`/`postings`/`autopilot_helpers` use
  `crate::commands::ai_provider` for embeddings/provider routing. All allowlisted with
  `TODO(arch)` (target: inject an emitter port + relocate `ai_provider`).

### L3 — Shell / IPC (`commands`, `ipc_contracts`, `main`, `updater`, `tray`, `deeplink`)

- **Allowed deps:** anything below (L0/L1/L2).
- **Forbidden deps:** none structurally — but L3 must stay **thin**: command handlers
  route to domain/application code and own no business logic.
- **Public API:** `#[tauri::command]` functions registered in `main.rs`'s
  `invoke_handler!`. `ipc_contracts` holds the serde DTOs.
- **Sole authority for Tauri:** **only L3 may define `#[tauri::command]`** and freely use
  `AppHandle`/`State`/`Manager`/`emit`. Command-defining locations are limited to
  `commands/**`, `export/commands/**`, and `updater/mod.rs` (cohesive command surfaces).
- **Ownership:** `commands/data.rs` owns the backup/restore bundle; `main.rs` owns the
  builder, menu, tray, and store registration.

---

## Module-ownership table (sole owners)

Extends `docs/PATTERNS.md` §13. No other module may reconstruct these:

| Concern                          | Sole owner                                                        | Use instead of rolling your own                                                 |
| -------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| env vars, data dir, FS paths     | `platform::config` (OS-process env: `platform::process`/`chrome`) | `platform::config::data_dir()`; never read app env elsewhere                    |
| HTTP client construction         | `net::http`                                                       | `net::http::shared()` / `build_client()` — never `reqwest::Client::new/builder` |
| timed trace spans                | `observability::Span`                                             | `Span::begin(..)` + `end`/`end_with`                                            |
| error types                      | `error::AppError`                                                 | return `AppResult<T>`; never `Result<_, String>` internally                     |
| SQLite handle + access           | `db` + per-domain `*/mod.rs` stores                               | `db::open()` inside the domain store; no `rusqlite::` in logic/helper files     |
| AI provider routing + embeddings | `ai_provider` _(relocating out of `commands/`)_                   | `resolve(ProviderId)`, `embed_text`, `cosine`, `compare`                        |
| job board scrapers               | `scraping::boards`                                                | register in `SCRAPERS`                                                          |
| workflow orchestration           | `pipeline`                                                        | compose `Stage`/`Pipeline`                                                      |
| Tauri command surface            | `commands/**` (+ `export/commands`, `updater`)                    | put new commands in `commands/`, not in a domain module                         |

---

## Enforced rules (each is a `#[test]` or CI gate)

| ID      | Rule                                                                                                                                        | Enforcement                            | Current exceptions (allowlist)                                                                                                                                     |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **R1**  | `#[tauri::command]` only under `commands/**`, `export/commands/**`, `updater/mod.rs`                                                        | arch test                              | none after W-2 fix (extraction’s command relocated)                                                                                                                |
| **R2**  | No Tauri (`tauri::`/`tauri_plugin_*`/`AppHandle`/`.emit(`) in L0/L1/L2 (shell-role files such as `export/commands/**` are exempt, not debt) | arch test                              | `platform/config`, `pipeline`, `cover_letter/{mod,leakage,research}`, `documents`, `conversations`, `autopilot_helpers`, `autopilot_scheduler` — each `TODO(arch)` |
| **R3**  | `rusqlite::` only in `db.rs`, `error.rs`, and per-domain store `mod.rs`                                                                     | arch test                              | the 8 genuine owners (documents, conversations, jobs, job_preferences, ai_generations, pipeline/cache, db, error)                                                  |
| **R4**  | `std::env::var` / `AJH_DATA_DIR` only in `platform/**`                                                                                      | arch test                              | `commands/ai_provider/{cli_agent,ollama}` (provider env: `OLLAMA_HOST`, `<AGENT>_BIN`) — `TODO(arch)`                                                              |
| **R5**  | `reqwest::Client::new/builder` only in `net/http.rs`                                                                                        | arch test                              | none (clean)                                                                                                                                                       |
| **R6**  | `Result<_, String>` forbidden outside `error.rs` (non-test)                                                                                 | arch test                              | none (clean)                                                                                                                                                       |
| **R7**  | No upward layer imports (L0→L1+, L1→L2+, L2→L3) via `crate::<mod>`                                                                          | arch test                              | the 6 edges in W-1/W-9: `error→extraction`; `{pipeline,documents,postings,autopilot_helpers,autopilot_scheduler}→commands`                                         |
| **R8**  | No source module > 1100 LOC (warn > 600)                                                                                                    | arch test (soft)                       | the 10 W-6 files, capped at current size; may not grow                                                                                                             |
| **R9**  | `cargo fmt --all -- --check` clean                                                                                                          | CI + `rustfmt.toml`                    | —                                                                                                                                                                  |
| **R10** | `cargo clippy --all-targets --all-features -- -D warnings`, no blanket crate-level `#![allow]`                                              | CI + `clippy.toml` + scoped allows     | unavoidable lints use a **scoped** `#[allow]` + reason at the site                                                                                                 |
| **R11** | `cargo deny check` (advisories, licenses, bans, sources)                                                                                    | CI + `deny.toml`                       | documented `deny.toml` entries for transitive licenses                                                                                                             |
| **R12** | `cargo audit` on every PR                                                                                                                   | CI                                     | RUSTSEC ignores listed in `deny.toml`/`audit.toml` if unfixable                                                                                                    |
| **R13** | No unused dependencies                                                                                                                      | CI (`cargo machete`)                   | `machete` ignore list if a dep is used only via macro/cfg                                                                                                          |
| **R14** | No lock held across `.await`                                                                                                                | `#![deny(clippy::await_holding_lock)]` | —                                                                                                                                                                  |

> **Allowlists are debt, not absolution.** Every `TODO(arch)` entry is a tracked exception
> that keeps the suite green **today** while making the rule block **new** violations.
> The test fails if an allowlisted file no longer needs its exception (so they can't rot).

---

## How to extend the system (stays within the rules)

- **New job board** → 1 scraper module under `scraping/boards/` + 1 line in `SCRAPERS`.
  Compose `net::http`, `error`, `observability`, `platform::config`. No new Tauri, no new
  env reads.
- **New AI provider** → 1 client module under `ai_provider/` + 1 `ProviderId` arm + 1
  `resolve` arm. (Until relocated, that is `commands/ai_provider/`.)
- **New IPC command** → a `#[tauri::command]` in `commands/<namespace>.rs` that routes to
  domain/application code, registered in `main.rs`. Never put the command in the domain
  module itself.
- **New persistent store** → a domain module with its store in `mod.rs` using `db::open()`
  - [rusqlite][rusqlite], implementing `data_store::DataStore`; register it in `commands/data.rs`.
- **New cross-cutting concern** → add a single owner under L0 and route everyone through
  it; add a guard to `tests/architecture.rs`.

## Roadmap (deferred, not in this change)

1. **Relocate `ai_provider`** from `commands/` to a top-level L1/L0 module → deletes the
   shell↔domain cycle and clears most of R7's allowlist.
2. **Inject an emitter port** so L1/L2 stream progress without `AppHandle` → clears R2's
   allowlist.
3. **Group the rendering cluster** under a `render/` parent (or document the cohesion as
   intentional) → addresses W-8.
4. **Split the god objects** (W-6), starting with `export/pdf_renderer` (1343 LOC).

[sqlite]: https://www.sqlite.org
[rusqlite]: https://github.com/rusqlite/rusqlite
