---
name: docs-standards
description: Documentation maintenance rules — which docs map to which code areas, the thin-pointer/no-drift rule, and the graphify-update step. Owned by project-steward. Load for /update-docs and the docs-sync step of the implement-workflow.
---

# Docs standards (project-steward)

## Code → docs map

- IPC contract change (`packages/shared/src/ipc/contracts/`) → `docs/API.md`.
- New domain/module or boundary change → `docs/knowledge/` + `docs/ARCHITECTURE.md`.
- Export/template change → `docs/EXPORT_TEMPLATES.md` + `docs/knowledge/resume-domain.md`.
- Scraping/provider change → `docs/knowledge/automation-domain.md`.
- Design-system change → `docs/DESIGN_SYSTEM.md`.
- Architecture / module / IPC-contract / registry change → also refresh the landing diagrams `landing/architecture-map.html` + `landing/how-it-works.html`, then run `pnpm check:landing-drift` (CI enforces it via the Lint & Format job).
- A durable architecture decision → an ADR in `docs/knowledge/decision-records/`.

## No-drift rule (thin pointers)

Describe **shape and contracts**; **never copy drift-prone literals** (scoring weights, template/board/applier counts). Point at the owning source symbol instead (weights → `commands/match_resume.rs`; templates → `export/templates/mod.rs`; registries → `scraping/boards/mod.rs` (SCRAPERS)). Knowledge files capped ~150 lines.

## After editing code or docs

Run `graphify update .` (AST-only, no API cost) so the graph stays current.

## Lessons graduation

When an **Architecture-decision** lesson becomes an ADR, **remove it from `lessons.jsonl`** — the ADR is then its single source.

## External standards & best-practices (verified 2026-06-19)

- **Diátaxis** — keep the four types distinct, never mixed in one page: **Tutorial** (learning), **How-to** (task), **Reference** (lookup/spec), **Explanation** (the "why"). Axes: action↔cognition, acquisition↔application. https://diataxis.fr/
- **Docs-as-code** — docs in-repo, versioned, reviewed in PRs, built/linted in CI. https://www.writethedocs.org/guide/docs-as-code/
- **Thin-pointer / no-drift** — `docs/knowledge/` points at the owning symbol/file; never copy code literals (they rot). After code changes, re-sync docs + run `graphify update .`.

**Common mistakes:** a "reference" page drifting into tutorial prose (split it — one need per page); pasting code values/signatures into docs instead of pointing at the source (guaranteed drift).
