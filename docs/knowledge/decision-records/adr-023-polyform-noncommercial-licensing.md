# ADR-023: PolyForm Noncommercial 1.0.0 licensing

Last updated: 2026-06-14

**Status:** Accepted

## Context

The AI Job Hunter project was initially released under the MIT License, permitting unrestricted use, modification, and redistribution — including commercial repackaging.

As the project matured and gained visibility, the risk of direct commercial repackaging emerged: third parties could fork or reuse the codebase to build competing products (same feature set, minimal attribution) without consent, violating the author's intent.

## Decision

**Relicense the codebase from MIT to PolyForm Noncommercial License 1.0.0** (SPDX: `PolyForm-Noncommercial-1.0.0`), effective **2026-06-14**, with the following terms:

- **Source-available** (not open source): free for noncommercial use (personal projects, internal tools, research, education, community contribution).
- **Commercial repackaging prohibited:** sales, SaaS hosting, or competing products derived from this codebase require a separate commercial license.
- **Commercial license:** available by contacting `contact@aijobhunter.app`.
- **Sole copyright holder:** Saeed Kolivand.

## Rationale

1. **Author consent:** noncommercial licensing respects the author's intent: support the job-hunting community without subsidizing competitors.
2. **Protects differentiator:** AI Job Hunter's value (9+ templates, semantic matching, multi-provider AI, scraping intelligence) is defensible only if competitors cannot wholesale-fork the codebase.
3. **Sustainability:** commercial inquiries (licensing, custom builds, dedicated support) can fund continued development.
4. **Community-friendly:** noncommercial use (contributions, forks, education) remains free and encouraged.
5. **Standards-based:** PolyForm is a widely recognized, lawyer-reviewed standard; straightforward to explain to enterprises.

## Consequences

- **MIT-licensed deployments in the wild:** remain under MIT (no retroactive relicensing). Future versions are PolyForm-only.
- **Contributions:** new PRs are implicitly contributed under PolyForm. Existing (MIT-era) contributors retain authorship; the project's licensing does not strip prior rights.
- **Ecosystem clarity:** `package.json` and `LICENSE` file reflect the new license; GitHub badge and README document the change and contact path.
- **No immediate revenue:** licensing is offered reactively; no hard enforcement (PolyForm relies on legal compliance, not technical measures). The project remains free for most users.

## Implementation

- `LICENSE` file: replaced with full PolyForm NC 1.0.0 text + copyright notice.
- `package.json`: `"license": "PolyForm-Noncommercial-1.0.0"`.
- `README.md`: license section expanded with SPDX id, source-available status, effective date, reason, and commercial contact.
- `docs/` and ADRs: thin pointer to LICENSE; no duplicate text.

## Related

- `LICENSE` — full license text and copyright.
- `README.md` § "License" — user-facing summary.
- `SECURITY.md` — security reporting (unchanged).
