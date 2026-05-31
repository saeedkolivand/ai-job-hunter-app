---
name: docs-standards
description: Documentation maintenance rules — which docs map to which code areas, the thin-pointer/no-drift rule, and the graphify-update step. Owned by project-steward. Load for /update-docs and the docs-sync step of the implement-workflow.
---

# Docs standards (project-steward)

## Code → docs map

- IPC contract change (`packages/shared/src/ipc/contracts.ts`) → `docs/API.md`.
- New domain/module or boundary change → `docs/knowledge/` + `docs/ARCHITECTURE.md`.
- Export/template change → `docs/EXPORT_TEMPLATES.md` + `docs/knowledge/resume-domain.md`.
- Scraping/provider change → `docs/knowledge/automation-domain.md`.
- Design-system change → `docs/DESIGN_SYSTEM.md`.
- A durable architecture decision → an ADR in `docs/knowledge/decision-records/`.

## No-drift rule (thin pointers)

Describe **shape and contracts**; **never copy drift-prone literals** (scoring weights, template/board/applier counts). Point at the owning source symbol instead (weights → `commands/match_resume.rs`; templates → `export/templates/mod.rs`; registries → `scraping/boards/mod.rs`, `applying/registry/mod.rs`). Knowledge files capped ~150 lines.

## After editing code or docs

Run `graphify update .` (AST-only, no API cost) so the graph stays current.

## Lessons graduation

When an **Architecture-decision** lesson becomes an ADR, **remove it from `lessons.jsonl`** — the ADR is then its single source.
