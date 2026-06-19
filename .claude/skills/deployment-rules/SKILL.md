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

## External standards & best-practices (verified 2026-06-19)

- **Conventional Commits 1.0.0** — `type(scope)!: description`; `feat`→MINOR, `fix`/`perf`→PATCH, `!`/`BREAKING CHANGE:`→MAJOR. https://www.conventionalcommits.org/en/v1.0.0/
- **SemVer 2.0.0** — `MAJOR.MINOR.PATCH`; pre-release `-rc.1` / build `+meta`. https://semver.org/spec/v2.0.0.html
- **semantic-release (v24+)** — derives bump + notes + tag from commit types; never hand-tag/hand-bump; sync version files via the tool, not by hand. https://github.com/semantic-release/semantic-release
- **Keep a Changelog 2.0.0** (released 2026-06-07 — format unchanged from 1.1.0: newest-first, `YYYY-MM-DD`, `Unreleased`+`[YANKED]`, six types Added/Changed/Deprecated/Removed/Fixed/Security; only guidance restructured). https://keepachangelog.com/

**Common mistakes:** dumping raw `git log` into the changelog (it's curated user-facing notes, not commit history); capitalized subjects or acronyms (`URL`/`API`) → commitlint `subject-case` failure (lowercase the subject).
