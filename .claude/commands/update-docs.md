---
description: Sync docs/knowledge with current code via project-steward (+ graphify update)
argument-hint: [area or files — defaults to current git diff]
---

Update documentation for: **$ARGUMENTS** (or the current `git diff`).

1. Load `docs-standards` + `token-efficiency`.
2. Spawn the `project-steward` subagent (Task). It maps changed code → affected docs (IPC → `docs/API.md`; domain/module → `docs/knowledge/` + `docs/ARCHITECTURE.md`; export/template → `docs/EXPORT_TEMPLATES.md`; scraping/provider → `docs/knowledge/automation-domain.md`; architecture/IPC/registry → also the landing diagrams `landing/architecture-map.html` + `landing/how-it-works.html`, verified with `pnpm check:landing-drift`).
3. Edit docs **minimally**, keep the **thin-pointer/no-drift rule** (describe shape/contracts, point at source symbols, never copy weights/counts).
4. Persist any proposed lessons to `.claude/memory/lessons.jsonl` (dedupe; graduate architecture decisions to ADRs and remove them from the log).
5. Run `graphify update .` so the graph stays current.
