# ADR-047 — Relicense from PolyForm Noncommercial to Apache-2.0

**Status:** Accepted (supersedes [ADR-023](adr-023-polyform-noncommercial-licensing.md))

**Date:** 2026-09-07

**Deciders:** owner (chose the license), main session (implementation)

## Context

The Microsoft Store refuses an unsigned EXE/MSI, which is what started this: shipping the desktop app there needs a code-signing certificate. The two ways to get Windows Authenticode signing without a recurring bill are open-source programmes — **SignPath Foundation** (free for OSS projects) and **Certum's** OSS programme (the paid-but-cheaper alternative) — and both gate on the project carrying an **OSI-approved license**.

[ADR-023](adr-023-polyform-noncommercial-licensing.md) had put the repo on PolyForm Noncommercial 1.0.0, which is deliberately source-available and **not** OSI-approved, so it disqualified the project from both. Azure Trusted Signing would have been the third route, but its individual tier is available only in the US and Canada and the owner is in Germany.

The license was therefore the binding constraint on signed Windows builds, not a legal preference: the anti-repackaging protection ADR-023 bought was blocking distribution on the platform that most needs signing.

## Decision

**Relicense the repo from PolyForm Noncommercial 1.0.0 to the Apache License 2.0** (SPDX: `Apache-2.0`), effective **2026-09-07**. The owner chose Apache-2.0 over MIT for its explicit patent grant; no `NOTICE` file is added, so redistributors carry `LICENSE` and the existing attribution notices and nothing further. The change is **not retroactive** — releases published before this date stay under the terms they shipped with, and earlier contributors keep the rights they had. `LICENSE` is authoritative: this record explains why the license changed and does not restate what it grants.

## Alternatives considered

1. **MIT.** Equally OSI-approved and equally acceptable to the signing programmes, so it cleared the actual constraint. Rejected by the owner because it carries no express patent grant, which Apache-2.0 §3 does — at no cost beyond a longer license file.
2. **Stay on PolyForm Noncommercial and buy a commercial OV certificate** (~$116/yr and up at the time, plus identity validation). Rejected on cost: it turns a one-time license change into a recurring bill for a project with no revenue.
3. **Azure Trusted Signing.** Rejected as unavailable, not as unsuitable — the individual tier does not cover Germany.
4. **Ship unsigned.** Not viable for the Microsoft Store, and on direct downloads it is the same problem one step removed (SmartScreen).

## Consequences

### Positive

- **Commercial use, forking, repackaging and resale are now permitted.** The noncommercial boundary ADR-023 existed to enforce is gone by choice; internal use at a for-profit company, previously "ask for a license", now needs no permission and no contact.
- **The free/cheap signing programmes become applicable**, which is the whole point — an OSI-approved license is their eligibility gate.
- **Contribution terms stay as simple as they were:** still no CLA and no copyright assignment (unchanged from ADR-023); contributions are now licensed under Apache-2.0 (`LICENSE` §5).

### Tradeoffs

- **The vendored ATS company datasets are a documented carve-out.** They are third-party data under CC BY-NC 4.0 that the Apache-2.0 grant does not reach, so a commercial redistributor must drop them or obtain the dataset author's permission. Provenance, attribution and the carve-out wording live in `apps/desktop/src-tauri/ats-slugs/README.md`; the loader repeats the pointer at the code (`apps/desktop/src-tauri/src/discovered/vendored.rs`).
- **Open follow-up, deliberately not decided here:** SignPath Foundation's terms ask for an OSI-approved license **for all components**, which the carve-out above arguably fails. If the application is questioned on it, the carve-out has to be resolved — replace the datasets with a permissively-licensed or self-harvested equivalent, or fetch them at runtime instead of vendoring them. Doing that pre-emptively is not part of this decision.
- **This direction is one-way.** Copies distributed under Apache-2.0 stay usable under it, so a later re-tightening could not reach them; the protection ADR-023 bought is not recoverable by changing the license again.

## References

- `LICENSE` — the authoritative text; every summary elsewhere is subordinate to it.
- `README.md` § License · `CONTRIBUTING.md` § Licensing and Contributions — the user-facing summaries kept in sync with this change.
- `apps/desktop/src-tauri/ats-slugs/README.md` — the CC BY-NC 4.0 dataset carve-out, its provenance and required attribution.
- [ADR-023](adr-023-polyform-noncommercial-licensing.md) — the decision this supersedes (MIT → PolyForm, effective 2026-06-14).
- The mechanical rollout is the relicense commit itself (`chore: relicense from polyform noncommercial to apache-2.0`); git history is that record, not this page.
