---
description: Improve a résumé's ATS score — job-match-expert (gaps) + resume-export-expert (safe formatting)
argument-hint: <résumé id/path + target job>
---

Improve the ATS score for: **$ARGUMENTS**

1. Load `token-efficiency`; read `docs/knowledge/resume-domain.md`.
2. Spawn `job-match-expert` (Task) — compute the gap: missing keywords/skills/requirements vs the target job; produce actionable, **truthful** suggestions (never fabricate experience).
3. Spawn `resume-export-expert` (Task) — verify the proposed changes keep the document **ATS-safe** (parseable layout, section naming, no formatting that breaks extraction).
4. Present the prioritized changes + expected score impact. If the user approves edits to résumé _data_, apply them; if it's a change to the scoring/formatting _code_, route through `/implement-feature`.

≤3 agents (the two Primaries + an optional Secondary). Stop at ~90% confidence.
