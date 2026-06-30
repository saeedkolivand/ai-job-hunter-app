# ADR-001: Rust-first business logic

Last updated: 2026-06-01

**Status:** Accepted · See also [`docs/DESIGN_DECISIONS.md`](../../DESIGN_DECISIONS.md)

## Context

Local-first desktop app ([Tauri][tauri]). Business logic could live in the [TypeScript][typescript] renderer or the [Rust][rust] core. Renderer logic is harder to test deterministically, can't access native capabilities directly, and risks duplicating rules across IPC.

## Decision

Business logic, processing pipelines, ATS analysis, and document generation live in **[Rust][rust]** (`apps/desktop/src-tauri/src/`). The [React][react] renderer is **presentation-only** and reaches the shell exclusively via the `AppClient` IPC contract.

## Consequences

- Single source of truth for rules; deterministic `cargo test`; native access (fs, keychain, browser automation).
- The renderer stays thin; new capability = the IPC 5-file flow (`tauri-standards`).
- Reviewers enforce this: business logic drifting into the renderer is a HIGH finding (`rust-backend-architect` / `frontend-reviewer`).

[tauri]: https://tauri.app
[rust]: https://www.rust-lang.org
[typescript]: https://www.typescriptlang.org
[react]: https://react.dev
