# ADR-001: Rust-first business logic

**Status:** Accepted · See also [`docs/DESIGN_DECISIONS.md`](../../DESIGN_DECISIONS.md)

## Context

Local-first desktop app (Tauri). Business logic could live in the TS renderer or the Rust core. Renderer logic is harder to test deterministically, can't access native capabilities directly, and risks duplicating rules across IPC.

## Decision

Business logic, processing pipelines, ATS analysis, and document generation live in **Rust** (`apps/tauri/src-tauri/src/`). The React renderer is **presentation-only** and reaches the shell exclusively via the `AppClient` IPC contract.

## Consequences

- Single source of truth for rules; deterministic `cargo test`; native access (fs, keychain, browser automation).
- The renderer stays thin; new capability = the IPC 5-file flow (`tauri-standards`).
- Reviewers enforce this: business logic drifting into the renderer is a HIGH finding (`rust-backend-architect` / `frontend-reviewer`).
