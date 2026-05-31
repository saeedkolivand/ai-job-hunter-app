---
description: AI provider review with ai-provider-expert (Primary Owner)
argument-hint: [files or PR# — defaults to current git diff]
---

Run an **AI provider** review (provider abstraction, embeddings, prompts, streaming, model routing, token/cost).

1. Load the `token-efficiency` + `automation-standards` skills; read `docs/knowledge/automation-domain.md`.
2. Scope with graphify; **stop at ~90% confidence**. No repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff` under `ai_provider/`, `commands/ai.rs`, `documents/embed`, `packages/prompts`.
4. Spawn **only** the `ai-provider-expert` subagent (Task) as Primary Owner. Enforce the **zero-change provider rule** (new provider = config + adapter only; no business-logic coupling = HIGH). Add `tauri-security-reviewer` (prompt injection / data leakage) as Secondary on risk.
5. Report severity-tagged findings; **HIGH/CRITICAL block**.
