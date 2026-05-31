---
description: Export pipeline review with resume-export-expert (PDF/DOCX rendering, contract, golden parity)
argument-hint: [files or PR# — defaults to current git diff]
---

Run an **export pipeline** review (PDF/DOCX rendering, the ExportRequest/ExportResult contract, fonts, pagination, golden parity, ATS-safe output, the validate gate).

1. Load the `token-efficiency` skill; read `docs/knowledge/resume-domain.md` (export contract) + `docs/EXPORT_TEMPLATES.md`.
2. Scope with graphify; **stop at ~90% confidence**.
3. Target = `$ARGUMENTS` if given, else the current `git diff` under `export/`, `layout/`, `measure/`.
4. Spawn **only** the `resume-export-expert` subagent (Task) as Primary Owner (review). `pdf-docx-generator` owns rendering _implementation_; `tauri-security-reviewer` joins as Secondary only if export touches file/temp/egress paths.
5. If golden snapshots changed, route a test pass via `test-author` → `testing-reviewer`.
6. Report severity-tagged findings; **HIGH/CRITICAL block**.
