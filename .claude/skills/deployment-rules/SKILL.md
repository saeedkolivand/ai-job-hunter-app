---
name: deployment-rules
description: Release/deployment standards — semantic-release, commitlint, version sync, the updater. Load for /prepare-release and changes to .releaserc / commitlint / workflows / version files. Owned by project-steward.
---

# Deployment / release rules

Automated via **semantic-release** on push to `main`. **Never manually tag or bump versions.**

## Release triggers (commit type → bump)

- `feat:` → minor · `fix:` / `perf:` → patch · `BREAKING CHANGE` footer → major.
- `refactor:` / `docs:` / `chore:` / `ci:` / `test:` → no release.

## Commit messages (commitlint, blocks the commit)

- Subject **lower-case**, ≤100 chars, imperative, no trailing period (acronyms like URL/API/DOCX must be lowercased or reworded).
- Body lines ≤200; blank line between subject and body.
- Type ∈ `feat|fix|perf|refactor|ui|style|test|docs|build|ci|chore|revert`.

## Version sync

Version files are synced by `scripts/sync-tauri-version.cjs` — don't hand-edit them; a mismatch breaks the release (CRITICAL).

## Updater

Updater manifest (`latest.json`) + signing key integrity — a broken/unsigned update is CRITICAL (defer the security lens to `tauri-security-reviewer`).

## Pre-push

Trust the pre-push hook; investigate failures rather than `--no-verify`.
