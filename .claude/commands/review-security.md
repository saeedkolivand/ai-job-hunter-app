---
description: Security review with tauri-security-reviewer (the security authority)
argument-hint: [files or PR# — defaults to current git diff]
---

Run a **security** review.

1. Load the `token-efficiency` + `security-checklist` skills; read `docs/knowledge/security-rules.md`.
2. Scope with graphify; **stop at ~90% confidence**. No repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff`.
4. Spawn the `tauri-security-reviewer` subagent (Task) over the target — desktop/app/backend/AI/data/abuse/supply-chain lens.
5. Report severity-tagged findings; **security/data findings round UP**; **HIGH/CRITICAL block** (LOW/MEDIUM advisory).
