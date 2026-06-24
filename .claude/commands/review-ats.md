---
description: ATS scoring / job-match review with job-match-expert (Primary Owner)
argument-hint: [files or PR# — defaults to current git diff]
---

Run an **ATS / job-match** review.

1. Load the `token-efficiency` skill.
2. Query graphify (MCP `query_graph "ats scoring"`, else `graphify explain "ats scoring"`); then targeted-read the authoritative source (`commands/match_resume.rs`) and `docs/knowledge/resume-domain.md` (ATS section) for context. **Stop at ~90% confidence.**
3. Target = `$ARGUMENTS` if given, else the current `git diff`.
4. Spawn **only** the `job-match-expert` subagent (Task) as Primary Owner. (Secondary `ai-provider-expert` only if provider infra changed.)
5. Report severity-tagged findings (`LOW/MEDIUM/HIGH/CRITICAL · file:line · fix`); **HIGH/CRITICAL block**.
