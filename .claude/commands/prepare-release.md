---
description: Release readiness review with project-steward (commits, version sync, changelog, updater)
argument-hint: [target version or "next"]
---

Prepare release: **$ARGUMENTS**

1. Load `deployment-rules` + `token-efficiency`.
2. **Bug-catch pre-flight** over the release range (`git diff <last-release-tag>..HEAD`) — no new engine, reuse existing pieces; **HIGH/CRITICAL block the release**, route each fix to the owning **author** → re-audit:
   - **Green gate** — `pnpm typecheck` + `pnpm test` + `cargo clippy`/`cargo test` (`apps/desktop/src-tauri`) all pass.
   - **Bug-focused critic pass** — spawn the touched-domain critics over the range diff with a correctness/bug lens (not just the arch rubric).
   - **Impact sanity** — `codegraph impact <changed-symbol>` for changed public symbols → flag callers not updated in the range.
   - **Over-engineering** — `ponytail:ponytail-review` over the range.
   - **Correctness** — the built-in `/code-review` (`code-review ultra` is a user-triggered option for high-stakes releases).
3. Spawn the `project-steward` subagent (Agent) to verify release readiness:
   - Conventional commits since last release are well-formed (commitlint) and the implied bump (`feat`→minor, `fix`/`perf`→patch, `BREAKING CHANGE`→minor while 0.x) is correct.
   - Version files are in sync (`scripts/sync-tauri-version.cjs`) — a mismatch is CRITICAL.
   - Changelog/notes accurate; updater manifest (`latest.json`) + signing integrity (defer the security lens to `tauri-security-reviewer`).
4. **Do NOT** manually tag or bump — releases are **manually dispatched** (Actions → "🚀 Release"); semantic-release then derives the bump from the commit types. Nothing runs automatically on push to `main`. Report blockers; fix commit/version issues via a PR.
