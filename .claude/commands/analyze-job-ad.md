---
description: Analyze a job ad with job-match-expert (requirements, skills, keywords, classification)
argument-hint: <job ad text, URL, or job id>
---

Analyze the job ad: **$ARGUMENTS**

1. Load `token-efficiency`; read `docs/knowledge/resume-domain.md` (ATS section) for the extraction/scoring model.
2. Spawn the `job-match-expert` subagent (Task) to extract: requirements, skills, technologies, industry classification, seniority — and how they map to the existing matching pipeline (`commands/match_resume.rs`).
3. Output a structured analysis (requirements / must-have skills / nice-to-have / keywords / classification) and, if a résumé is in context, gap analysis + actionable tailoring recommendations.

This is analysis (read-only) — no code change unless the user asks.
