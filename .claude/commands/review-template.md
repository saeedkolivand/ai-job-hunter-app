---
description: Resume template review with resume-export-expert (rendering impl by pdf-docx-generator)
argument-hint: [files or PR# — defaults to current git diff]
---

Run a **resume template** review (design, ATS-safe layout, visual consistency, country/industry standards, rendering predictability).

1. Load the `token-efficiency` skill; read `docs/knowledge/resume-domain.md` + `docs/EXPORT_TEMPLATES.md`.
2. Scope with graphify (MCP `query_graph "resume templates"`, else `graphify explain "resume templates"`); **stop at ~90% confidence**.
3. Target = `$ARGUMENTS` if given, else the current `git diff` under `export/templates/`, `theme/`, `locale/`.
4. Spawn **only** the `resume-export-expert` subagent (Task) as Primary Owner. Rendering _implementation_ concerns belong to `pdf-docx-generator` (not a reviewer).
5. Report severity-tagged findings; **HIGH/CRITICAL block**.
