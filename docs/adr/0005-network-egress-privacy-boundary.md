---
status: accepted
---

# Network egress and the local-first privacy boundary

## Context

The README and SECURITY.md both state: "The only outbound calls are to the AI provider you configure and an optional web search you enable." A full-history audit (2026-07) found this literally false: the app also makes network calls for job-board scraping (the aggregator plus per-board and ATS fetches), an on-launch updater check to GitHub, user-initiated location autocomplete (OpenStreetMap/Nominatim), and optional company-logo enrichment (Clearbit).

Investigating each call showed none of them exfiltrate the data the guarantee is actually about — résumés, generations, applications, tracked job data, and credentials. Scraping is the app's core function and is disclosed two sentences earlier in the same README. Geocoding is a typeahead that sends only what the user types into a location field. The updater sends only a version check. Clearbit is opt-in, off by default, CSP-scoped to two hosts, and sends only a company name. The claim conflated two distinct promises — "your personal data is never collected by telemetry or an app-operated backend" (true) and "the app makes almost no network calls" (false) — into one over-absolute sentence.

## Decision

The local-first privacy boundary is defined in terms of **storage and telemetry**, not total network silence. The guarantee is:

> Your résumés, generations, applications, tracked job data, and credentials live in a local database on your device — there is no telemetry and no app-operated backend collecting them. Data leaves the device only for services you configure or invoke (enumerated below) — notably the AI provider, which receives the résumé and job text you ask it to generate from — never for a first-party analytics or collection backend.

Network egress is **permitted** and enumerated by class, each with a gating rule:

1. **AI provider** (user-configured) — required for the core generate/match/answer features; the user chooses the provider and supplies the key.
2. **Job boards and aggregators** (scraping) — the app's core function; disclosed. Sends only search parameters and fetches public postings.
3. **Web search** — optional, opt-in, off until the user enables it.
4. **Updater** (GitHub releases) — an on-launch version check; sends no user data.
5. **Location autocomplete** (OpenStreetMap/Nominatim) — user-initiated typeahead; sends only the location text the user types, returns city/country only.
6. **Optional enrichment** (e.g. Clearbit logos) — must be **opt-in and default OFF**, CSP-scoped to the minimum hosts, and send no more than a public identifier (a company name). With the setting off, nothing leaves the device.

**Rule for new egress:** any new outbound call that would send data derived from the user's documents, credentials, or tracked applications is prohibited by default; anything sending a public identifier or user-typed query for enrichment must follow rule 6 (opt-in, default OFF, CSP-scoped). The README/SECURITY wording must keep the personal-data guarantee and accurately enumerate the endpoint classes rather than claim a call count.

No runtime behavior changes: every current call already complies. The fix is to correct the documentation.

## Considered options

1. **Redefine the guarantee around personal data + enumerate egress classes (chosen).** Honest, keeps the strong true promise, and gives a durable rule for future features. Cost: the marketing line loses its punchy "only two calls" absolutism.
2. **Add a master "airplane mode" that disables all non-AI egress.** Rejected for now: it would degrade core features (scraping, location typeahead) for a guarantee that is already satisfied for personal data; revisit only if users ask for a hard offline mode.
3. **Gate geocoding and the updater behind settings to make the original sentence true.** Rejected: worsens the UX of a benign typeahead and a standard update check to preserve an over-absolute claim; treats the symptom (wording) by mutilating behavior.
4. **Leave the docs as-is.** Rejected: a materially false statement in a security-facing document misleads a privacy-conscious user and fails the audit's accuracy bar.

## Consequences

- **README.md and SECURITY.md must be rewritten** to state the personal-data guarantee and enumerate the six egress classes. (Tracked as a develop/modify item, not yet applied.)
- **A new egress endpoint now has a written test to pass:** does it send personal data (prohibited) or a public identifier/typed query (opt-in, default OFF, CSP-scoped)? This is the reference for future review.
- **The opt-in enrichment pattern (Clearbit) is now the sanctioned template** for feature-driven egress: default OFF, minimal CSP, public identifier only.
- **No behavior changes ship from this ADR.** If a future decision adds a hard offline mode, it supersedes option 2 here.

## References

- Privacy claims: `README.md` ("What It Does"), `SECURITY.md` ("Security posture").
- Egress sites: `apps/desktop/src-tauri/src/commands/geocoding.rs` (Nominatim), `apps/desktop/src-tauri/src/scraping/boards/aggregator/mod.rs` (Adzuna/JSearch), the updater wiring in `apps/desktop/src-tauri/src/lib.rs`, `apps/desktop/src/renderer/services/use-company-logo/use-company-logo.ts` (Clearbit, opt-in).
- Opt-in setting: `apps/desktop/src/renderer/store/preferences-schema/preferences-schema.ts` (company-logo preference, default OFF).
- Audit finding: `p2-contra-cross-001` (AUDIT_REPORT.md §4).
