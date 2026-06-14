# ADR-024: Consolidated atomic release commit

Last updated: 2026-06-14

**Status:** Accepted

## Context

The release pipeline previously executed version syncing as a separate CI step _after_ the release tag was created:

1. semantic-release analyzes commits → creates `v*` tag + publishes GitHub release notes
2. A separate `sync-version-files` GitHub Actions job runs, reads the tag, syncs 7 version files via `scripts/sync-tauri-version.cjs`, then commits them back to `main`

**Problems:**

1. **Flaky dispatch flag:** the action's `new_release_published` output was unreliable — it sometimes reported false or stale — so the `sync-version-files` step ran with a blanket `always()` gate + `git describe` probe, coupling release detection across two independent jobs.
2. **Tag precedes sync:** the version tag pointed to a commit that _lacked_ the synced version files (because the sync was a _commit_ after the _tag_). The build job then re-ran the sync to compensate, duplicating work and creating a divergence window.
3. **Inconsistent tag content:** the released artifact shipped files that differed from the tagged commit, violating the expectation that a `git checkout v*` should be reproducible and self-contained.

## Decision

**Consolidate version syncing into semantic-release's own release flow**, making it atomic with the tag:

1. **exec plugin (prepare phase):** runs `scripts/sync-tauri-version.cjs ${nextRelease.version}` to sync 7 version files in-process
2. **changelog plugin (prepare phase):** generates `CHANGELOG.md` at repo root
3. **github plugin (publish phase):** publishes GitHub release + assets
4. **git plugin (publish phase):** commits the synced version files + `CHANGELOG.md` to `main` with message `chore(release): <version> [skip ci]` and tags the commit `v*`

**Result:** the tag points to the commit that contains synced versions, Changelog, and all other release artifacts — fully self-consistent.

The separate `sync-version-files` GitHub Actions job is **removed**; the `release` job is updated to use `cycjimmy/semantic-release-action` with `extra_plugins` (changelog, git, exec).

**Deploy key:** the job checks out with `ssh-key: RELEASE_DEPLOY_KEY` so `@semantic-release/git` can push to the protected `main` branch (same key the other jobs use).

## Rationale

1. **Single source of truth:** semantic-release's own release decision triggers the sync iff a release is warranted; no separate gate, no `git describe` probe.
2. **Self-contained tag:** `git checkout v*` yields a fully-versioned, fully-synced, fully-consistent repository state. No re-sync needed during build.
3. **Reduced duplication:** the build job no longer needs to re-run the sync to compensate for a tag created before syncing.
4. **Aligns with model:** the documented release flow says "a release commit versions the app, drafts the notes, and syncs version files" — the implementation now matches that model exactly.
5. **CHANGELOG.md as in-repo mirror:** changelog is generated and committed atomically with the release, providing offline access while GitHub Releases remain canonical (they carry downloads + assets).

## Consequences

- **Single release commit:** all version bumps, changelog, and synced files land in one atomic operation, tagged once.
- **Simpler CI config:** no dispatch orchestration between jobs; the release logic lives entirely in semantic-release's plugin chain.
- **Build job simplification:** the build job no longer detects or re-runs version syncing; it trusts the tag's consistency.
- **SSH deploy key required:** the `release` job must check out with `ssh-key: RELEASE_DEPLOY_KEY` to push to protected `main` (this key already exists and is in use by other jobs).
- **No breaking change:** existing downstream tooling (tagname format, GitHub asset structure, auto-updater manifest) is unchanged.

## Related

- `.releaserc.json` — plugin configuration (commit-analyzer → release-notes-generator → exec → changelog → github → git)
- `.github/workflows/release.yml` — `release` job updated; `sync-version-files` job removed
- `scripts/sync-tauri-version.cjs` — invoked by semantic-release's exec plugin in `prepare` phase
- `docs/DEPLOYMENT.md` § "Release configuration" and "CI/CD Pipeline" — updated to reflect consolidated flow
