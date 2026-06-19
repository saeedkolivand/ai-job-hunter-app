---
description: Browser-extension + bridge review with extension-reviewer (Primary Owner)
argument-hint: [files or PR# — defaults to current git diff]
---

Run an **extension / bridge** review (MV3 compliance, permission minimization, pairing/token auth, origin allowlist, protocol lockstep TS↔Rust, Chrome Web Store + Firefox AMO store policy).

1. Load the `token-efficiency` + `extension-standards` skills (the latter carries the Chrome Web Store + AMO store-policy checklist).
2. Scope with graphify; **stop at ~90% confidence**. No repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff` under `apps/extension/`, `apps/tauri/src-tauri/src/extension_bridge/`, `commands/extension_bridge.rs`, `packages/shared/src/ipc/extension-protocol-constants.ts`.
4. Spawn **only** the `extension-reviewer` subagent (Task) as Primary Owner. Add `tauri-security-reviewer` (auth/token/permission/data) as Secondary on risk — **≤3 reviewers**.
5. Report severity-tagged findings; **HIGH/CRITICAL block** (auth bypass, token leakage, over-broad permission / store-rejection risk, protocol drift).
