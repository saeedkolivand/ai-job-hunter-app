# ADR-048 — Extension store submission is automated, terminal at "submitted", and gated on a source archive that provably rebuilds the shipped package

**Status:** Accepted

**Date:** 2026-09-07

**Deciders:** owner (chose to automate), main session (implementation)

## Context

The MV3 extension is already listed on the Chrome Web Store and Firefox AMO, and `scripts/sync-tauri-version.cjs` bumps its version with every app release — so every release owes both stores a new version. Until now that was a manual post-release chore across two dashboards, i.e. the one release step nothing enforced and nothing noticed the absence of.

Firefox makes it worse than a chore. AMO requires reviewable **source** for a bundled add-on (this one is built with Vite), and reviewers **rebuild from that source and diff the result against the uploaded package**. A mismatch delays the review, and a repeat offence gets the add-on taken down — so the artifact most likely to be assembled wrong by hand is also the one with the harshest failure mode.

Automating it introduces its own problem: the job that submits to AMO holds a credential that can publish a Mozilla-signed version of **every** add-on on the account, and the tool that uses it (`web-ext`) drags in a large dependency tree — including a linter this repo does not otherwise use — that would otherwise be installed and executed in that same job.

## Decision

**Every installer-build dispatch submits the extension to both stores automatically** — the same `workflow_dispatch` (`action: build-installers`) that builds the installers — from two independent jobs in `.github/workflows/release.yml` (`publish-chrome`, `publish-firefox`) that consume the zips `package-extension` already built, handed over as a workflow artifact rather than rebuilt or re-downloaded, so what reaches a store cannot diverge in CONTENT from what is attached to the GitHub Release. Chrome receives that zip byte-for-byte; `web-ext` re-zips the directory for AMO, so the submitted xpi is file-for-file identical rather than byte-identical, and byte-identity is claimed only where it holds. Three properties are part of the decision, not incidental to it:

1. **The terminal state is "submitted for review", never "live".** Approval is a human step at Google/Mozilla that lands hours to days later; neither job waits for it, and neither is a `needs:` of anything else, so one store failing blocks neither the other store nor the rest of the release fan-out.
2. **AMO submission is gated on reproducibility.** `apps/extension/scripts/source-archive.mjs` builds the reviewable archive as `git archive` of the release tag plus a generated build README whose tool versions are read from the toolchain that produced it; `publish-firefox` then unpacks that archive, runs the README's own build commands and diffs the result against the shipped package — failing **before** anything is uploaded when they differ.
3. **The AMO submission tool is pinned outside the pnpm workspace.** `web-ext` lives in `apps/extension/tools/amo/` under its own committed npm lockfile, installed with `--ignore-scripts` in a step that carries no credential; the single step that holds the AMO key runs nothing but the submission.

## Alternatives considered

1. **Keep submitting by hand.** Rejected: it is the release step easiest to skip, and the AMO source archive is the artifact most likely to be built wrong by hand — exactly the case where a mistake is a rejection rather than a retry.
2. **`npx web-ext@<version>` instead of a committed lockfile.** Rejected: that pins only the entry package. Every transitive dependency re-resolves at run time, without integrity hashes, inside the job holding the AMO credential.
3. **`web-ext` as a normal devDependency of `@ajh/extension`.** Rejected: it would add a once-per-release tool's whole tree to every `pnpm install --frozen-lockfile` in the monorepo — every CI job, every contributor clone, and the audit surface — for something that runs once per release. Rationale and the bump procedure live in `apps/extension/tools/amo/README.md`.
4. **Re-download the published release asset in the publish jobs.** Rejected: it opens a way for the submitted contents to differ from the released ones, which is the one property this design exists to keep.
5. **Submit the source archive without proving it rebuilds the package.** Rejected: AMO performs that rebuild regardless. The gate does not add a check, it just moves an existing one into CI, where failing costs a red job instead of a delisting.
6. **One combined publish job.** Rejected: a rejection or outage at one store would stop the other from shipping, and a single job status could not say which store failed.

## Consequences

### Positive

- **The stores track the release automatically.** Submission stops being a step a human can forget between a release and the next one.
- **The archive AMO receives is provably the one that produces the shipped package**, checked the same way the reviewer will check it.
- **No store credential is in scope while third-party code is fetched or installed.** Not while it is _executed_: the submission step necessarily runs the tool's whole dependency tree with the store key in its environment, which is an accepted residual risk stated in full in `apps/extension/tools/amo/README.md`, not something this design removes. Credential presence is also checked in a dedicated first step per job, so a misconfiguration surfaces by name rather than as an opaque API error.

### Tradeoffs

- **The reproducibility gate is expensive.** It performs a second full workspace install and build on top of the job's own. That cost is accepted deliberately; if the gate ever goes red, fix the non-determinism in the build — do **not** loosen the comparison.
- **`apps/extension/tools/amo/` is a deliberate exception** to this repo's "pnpm, not npm" rule and to its workspace layout. It therefore needs its own Dependabot entry and its own advisory triage, both recorded in that directory's README rather than inherited from the root install.
- **A green job still does not mean the version is live.** The store dashboards remain the source of truth for approval, and re-running a release for a tag cut before this feature existed fails these jobs by construction — the publishing tooling is not in that tag's tree.

## References

- `docs/DEPLOYMENT.md` § "Browser extension store publishing" — the operational page: credential setup, secret names, and the known failure modes. This record is the _why_; that page is the _how_.
- `.github/workflows/release.yml` — the two jobs, commented step by step.
- `apps/extension/scripts/source-archive.mjs` · `apps/extension/src/source-archive.test.ts` — the archive builder and its checks.
- `apps/extension/tools/amo/README.md` — why the tool is isolated, which advisories are accepted, and how to bump it.
- `docs/knowledge/extension-domain.md` § "Store policy" — the pointer from the domain doc.
- [ADR-015](adr-015-extension-bridge-websocket-save-origin.md) — the extension/desktop bridge whose extension half is what gets submitted.
