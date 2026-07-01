---
name: tauri-standards
description: Tauri shell standards — the IPC 5-step capability flow, command implementation pattern, and capability/permission wiring. Load for new IPC capabilities and changes to commands.rs / tauri-client.ts / capabilities/.
---

# Tauri / IPC standards

## New IPC capability — 5 files, in order

1. `packages/shared/src/ipc/contracts.ts` — add the typed signature (Zod schema).
2. `apps/desktop/src-tauri/src/commands.rs` — implement the Tauri command (typed `AppResult`, no `Result<_,String>`).
3. `apps/desktop/src/tauri-client.ts` — wire the `invoke` call.
4. `apps/desktop/src/renderer/services/` — add the React Query service hook (no `window.api` in UI).
5. `services/query-client.ts` — add the query key.

Missing any step = an incomplete capability (HIGH). The contract in `packages/shared` is the single source of truth.

## Capabilities & permissions

- New commands must be allowed in `capabilities/default.json` — an exposed-but-unlisted command, or an over-broad capability, is a security finding (defer the security lens to `tauri-security-reviewer`).
- Principle of least privilege for filesystem/shell/network scopes.

## Renderer ↔ shell

Renderer talks to the shell only via the `AppClient` context (`createTauriInvokeClient()` in `apps/desktop/src/tauri-client.ts`). No direct invoke in features/routes/components.

## Boundaries

- `packages/shared` — no React, no Node APIs.
- `packages/ui` — no Zustand, no IPC, no routing.
- `packages/prompts` — no UI, no `window`.

## External standards & best-practices (verified 2026-06-19)

> Latest stable **Tauri 2.10.x**. v2 model = **permissions** (per-command) + **scopes** (arg validators) + **capabilities** (bind sets to windows); no command is exposed by default. https://v2.tauri.app/security/

- **Least-privilege capabilities** — grant only what each window needs in `capabilities/*.json`; scope to specific `windows`/`webviews`; prefer `allow`-scopes; never ship `*`-style `fs`/`shell`/`http`. https://v2.tauri.app/security/capabilities/
- **CSP** — strict `app.security.csp` in `tauri.conf.json`; no `unsafe-inline`/`unsafe-eval` (Tauri hashes/nonces local assets). https://v2.tauri.app/security/csp/
- **IPC surface** — treat every `#[tauri::command]` as an untrusted entry point; validate/sanitize args in Rust; keep the command set minimal; gate sensitive commands behind their own narrow capability.
- **Isolation pattern** — `app.security.pattern = "isolation"` to verify IPC in a sandboxed iframe whenever the frontend renders any remote/untrusted content. https://v2.tauri.app/concept/inter-process-communication/isolation/
- **Updater signing (mandatory)** — minisign-sign every release; ship the public key in `plugins.updater.pubkey`; private key + password offline (CI secret only); HTTPS manifests. https://v2.tauri.app/plugin/updater/
- **Dep hygiene** — `cargo audit` clean (e.g. **RUSTSEC-2026-0098** `rustls-webpki` can reach the updater/HTTP TLS stack); minimize plugins; track https://github.com/tauri-apps/tauri/security/advisories

**Common mistakes:** over-broad capabilities (one capability → every plugin permission → every window); leaving `shell`/`fs`/`withGlobalTauri` open in prod; weak/default CSP (`unsafe-inline`); trusting command args without Rust-side validation; committing the updater private key or disabling signature verification; skipping `cargo audit` so a transitive advisory ships silently.
