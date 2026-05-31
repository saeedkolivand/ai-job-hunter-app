---
description: Resume domain review with resume-export-expert (Primary Owner)
argument-hint: [files or PR# — defaults to current git diff]
---

Run a **resume domain** review (architecture, sections, customization, country/industry standards, ATS-safe structure).

1. Load the `token-efficiency` skill; read `docs/knowledge/resume-domain.md` + `domain-model.md`.
2. Scope with graphify; **stop at ~90% confidence**. No repo-wide scan.
3. Target = `$ARGUMENTS` if given, else the current `git diff`.
4. Spawn **only** the `resume-export-expert` subagent (Task) as Primary Owner.
5. Report severity-tagged findings; **HIGH/CRITICAL block**.
