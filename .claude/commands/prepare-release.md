---
description: Release readiness review with project-steward (commits, version sync, changelog, updater)
argument-hint: [target version or "next"]
---

Prepare release: **$ARGUMENTS**

1. Load `deployment-rules` + `token-efficiency`.
2. Spawn the `project-steward` subagent (Task) to verify release readiness:
   - Conventional commits since last release are well-formed (commitlint) and the implied bump (`feat`→minor, `fix`/`perf`→patch, `BREAKING CHANGE`→major) is correct.
   - Version files are in sync (`scripts/sync-tauri-version.cjs`) — a mismatch is CRITICAL.
   - Changelog/notes accurate; updater manifest (`latest.json`) + signing integrity (defer the security lens to `tauri-security-reviewer`).
3. **Do NOT** manually tag or bump — semantic-release runs on push to `main`. Report blockers; fix commit/version issues via a PR.
