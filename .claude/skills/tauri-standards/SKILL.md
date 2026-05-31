---
name: tauri-standards
description: Tauri shell standards — the IPC 5-step capability flow, command implementation pattern, and capability/permission wiring. Load for new IPC capabilities and changes to commands.rs / tauri-client.ts / capabilities/.
---

# Tauri / IPC standards

## New IPC capability — 5 files, in order

1. `packages/shared/src/ipc/contracts.ts` — add the typed signature (Zod schema).
2. `apps/tauri/src-tauri/src/commands.rs` — implement the Tauri command (typed `AppResult`, no `Result<_,String>`).
3. `apps/tauri/src/tauri-client.ts` — wire the `invoke` call.
4. `apps/tauri/src/renderer/services/` — add the React Query service hook (no `window.api` in UI).
5. `services/query-client.ts` — add the query key.

Missing any step = an incomplete capability (HIGH). The contract in `packages/shared` is the single source of truth.

## Capabilities & permissions

- New commands must be allowed in `capabilities/default.json` — an exposed-but-unlisted command, or an over-broad capability, is a security finding (defer the security lens to `tauri-security-reviewer`).
- Principle of least privilege for filesystem/shell/network scopes.

## Renderer ↔ shell

Renderer talks to the shell only via the `AppClient` context (`createTauriInvokeClient()` in `apps/tauri/src/tauri-client.ts`). No direct invoke in features/routes/components.

## Boundaries

- `packages/shared` — no React, no Node APIs.
- `packages/ui` — no Zustand, no IPC, no routing.
- `packages/prompts` — no UI, no `window`.
