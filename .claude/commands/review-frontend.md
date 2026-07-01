---
description: Renderer/UI review with frontend-reviewer (UI only)
argument-hint: [files or PR# — defaults to current git diff]
---

Run a **frontend (renderer)** review — UI, design system, i18n, a11y only.

1. Load the `token-efficiency` + `frontend-standards` skills; read `docs/DESIGN_SYSTEM.md` and `docs/knowledge/architecture.md` (feature ownership).
2. Scope with graphify; **stop at ~90% confidence**. No repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff` under `apps/desktop/src/renderer/**`.
4. Spawn **only** the `frontend-reviewer` subagent (Task). It does NOT review backend/export/scraping/ai/ATS — defer those to their owners.
5. Report severity-tagged findings (ports-&-adapters, tokens, motion, `@ajh/ui`, i18n, a11y); **HIGH/CRITICAL block**.
