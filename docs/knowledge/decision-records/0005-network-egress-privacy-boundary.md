---
status: accepted
---

# Network egress and the local-first privacy boundary

## Context

The README and SECURITY.md both state: "The only outbound calls are to the AI provider you configure and an optional web search you enable." A full-history audit (2026-07) found this literally false: the app also makes network calls for job-board scraping (the aggregator plus per-board and ATS fetches), an on-launch updater check to GitHub, user-initiated location autocomplete (OpenStreetMap/Nominatim), and optional company-logo enrichment (Clearbit).

Investigating each call showed none of them exfiltrate the data the guarantee is actually about — résumés, generations, applications, tracked job data, and credentials. Scraping is the app's core function and is disclosed two sentences earlier in the same README. Geocoding is a typeahead that sends only what the user types into a location field. The updater sends only a version check. Clearbit is opt-in, off by default, CSP-scoped to two hosts, and sends only a company name. The claim conflated two distinct promises — "your personal data is never collected by telemetry or an app-operated backend" (true) and "the app makes almost no network calls" (false) — into one over-absolute sentence.

## Decision

The local-first privacy boundary is defined in terms of **storage and telemetry**, not total network silence. The guarantee is:

> Your résumés, generations, applications, tracked job data, and credentials live in a local database on your device — there is no app-operated backend collecting them and no behavioural analytics. Data leaves the device only for services you configure or invoke (enumerated below) — notably the AI provider, which receives the résumé and job text you ask it to generate from — plus two classes of automatic app-initiated egress (updater version checks and crash reports when the app fails), which send only metadata and error information, never your documents or job data.

Network egress is **permitted** and enumerated by class, each with a gating rule:

1. **AI provider** (user-configured) — required for the core generate/match/answer features; the user chooses the provider and supplies the key.
2. **Job boards and aggregators** (scraping) — the app's core function; disclosed. Sends only search parameters and fetches public postings. One board (freehire) is **keyless**: it needs no credential, so unlike every other broad source it can run on a fresh install before anything is configured. It stays inside this class because it sends the same thing the keyed sources do: search keywords and the requested place. The place costs one extra call to freehire's own city typeahead to resolve the free text to their canonical facet value — same host, same class, and it sends the city segment only (never the whole `"{city}, {country}"` label). See `resolve_freehire_city` and `fetch_freehire` in `apps/desktop/src-tauri/src/scraping/boards/freehire/mod.rs`. It was the aggregator's always-on last tier, where its failures degraded to empty because nobody had opted into it; as a board the user selects, a failure is reported like any other board's.
3. **Web search** — optional, opt-in, off until the user enables it.
4. **Updater** (GitHub releases) — a version check 10 s after launch, then every 4 h; sends only app version and OS/architecture.
5. **Location autocomplete** — **offline by default**: a bundled GeoNames index (`apps/desktop/src-tauri/geodata/`, CC BY 4.0) answers virtually every query with no network call at all. Only a query the index cannot match reaches Photon (photon.komoot.io, OpenStreetMap/ODbL) as a fallback; it sends only the location text the user types and returns city/country only. Nominatim was retired here — its usage policy forbids autocomplete.
6. **Optional enrichment** (e.g. Clearbit logos) — must be **opt-in and default OFF**, CSP-scoped to the minimum hosts, and send no more than a public identifier (a company name). With the setting off, nothing leaves the device.
7. **Email-confirmation watching** (IMAP to user's own mail host) — opt-in, default OFF; credential user-supplied and OS-keychain-backed; email content never leaves the device. See [ADR 0013](0013-email-confirmation-watching.md).

8. **Crash reporting** (Sentry) — the one class that is **default ON** rather than default OFF, and the only first-party-initiated egress. Amended in by [ADR 0020](0020-crash-reporting.md); see it for why the default departs from rule 6, and for the redaction and consent-gating that make the departure survivable. Sends the error, stack trace, OS/architecture and app version only; never document, job, prompt or credential content. Desktop app only — the browser extension is excluded so its AMO `data_collection_permissions: ['none']` declaration stays true.

**Rule for new egress:** any new outbound call that would send data derived from the user's documents, credentials, or tracked applications is prohibited by default; anything sending a public identifier or user-typed query for **enrichment** must follow rule 6 (opt-in, default OFF, CSP-scoped).

**Exemption (classes 2 and 5 — user-initiated lookups):** rule 6 governs _enrichment_ — a call the app makes on its own initiative to decorate data the user did not ask about (a logo for a company they merely viewed). It does **not** govern a lookup the user directly invoked by typing into a field and waiting for its result: location autocomplete (class 5) and job search (class 2). Those are the feature, not a garnish, so an opt-in default-OFF toggle would just break them. They qualify only while all three hold: (a) **user-initiated** — no call without a deliberate action; (b) **no personal data** — only the literal text typed into that field; (c) **Rust-side** — issued from the backend through `net::http`, so no renderer CSP entry is needed and the request is auditable in one place. Location autocomplete additionally answers offline from a bundled index, so the common case makes no call at all. The README/SECURITY wording must keep the personal-data guarantee and accurately enumerate the endpoint classes rather than claim a call count.

No runtime behavior changes: every current call already complies. The fix is to correct the documentation.

## Considered options

1. **Redefine the guarantee around personal data + enumerate egress classes (chosen).** Honest, keeps the strong true promise, and gives a durable rule for future features. Cost: the marketing line loses its punchy "only two calls" absolutism.
2. **Add a master "airplane mode" that disables all non-AI egress.** Rejected for now: it would degrade core features (scraping, location typeahead) for a guarantee that is already satisfied for personal data; revisit only if users ask for a hard offline mode.
3. **Gate geocoding and the updater behind settings to make the original sentence true.** Rejected: worsens the UX of a benign typeahead and a standard update check to preserve an over-absolute claim; treats the symptom (wording) by mutilating behavior.
4. **Leave the docs as-is.** Rejected: a materially false statement in a security-facing document misleads a privacy-conscious user and fails the audit's accuracy bar.

## Consequences

- **README.md and SECURITY.md have been updated** to state the personal-data guarantee and enumerate the egress classes.
- **A new egress endpoint now has a written test to pass:** does it send personal data (prohibited) or a public identifier/typed query (opt-in, default OFF, CSP-scoped)? This is the reference for future review. The machine-readable inventory lives in `apps/desktop/src-tauri/tests/egress.rs`.
- **The opt-in enrichment pattern (Clearbit) is now the sanctioned template** for feature-driven egress: default OFF, minimal CSP, public identifier only.
- **No behavior changes ship from this ADR.** If a future decision adds a hard offline mode, it supersedes option 2 here.

## References

- Privacy claims: `README.md` ("What It Does"), `SECURITY.md` ("Security posture").
- Egress sites by class:
  - Class 2 (Job boards/aggregators): `apps/desktop/src-tauri/src/scraping/boards/aggregator/adzuna.rs` (Adzuna), `apps/desktop/src-tauri/src/scraping/boards/aggregator/providers.rs` (JSearch/RapidAPI, Jooble, Apify), `apps/desktop/src-tauri/src/scraping/boards/freehire/mod.rs` (freehire, keyless).
  - Class 3 (Web search): `apps/desktop/src-tauri/src/commands/ai_provider/search/mod.rs` (Exa).
  - Class 4 (Updater): `apps/desktop/src-tauri/tauri.conf.json` (endpoint configuration), `apps/desktop/src-tauri/src/updater/mod.rs` (polling loop), `apps/desktop/src-tauri/src/lib.rs` (setup hook).
  - Class 5 (Location autocomplete): `apps/desktop/src-tauri/src/commands/geocoding.rs` (Photon fallback; bundled index in `apps/desktop/src-tauri/geodata/`, see `apps/desktop/src-tauri/geodata/README.md` for license).
  - Class 6 (Optional enrichment): `apps/desktop/src/renderer/services/use-company-logo/use-company-logo.ts` (Clearbit), `apps/desktop/src-tauri/tauri.conf.json` (CSP enforcement).
  - Class 7 (Email-confirmation watching): `apps/desktop/src-tauri/src/email_watch/` (IMAP).
  - Class 8 (Crash reporting): Sentry integration in `apps/desktop/src-tauri/src/crash_reporting/mod.rs` (the `DSN` const and the event filter) and `crash_reporting/transport.rs`, wired from `lib.rs` via `crash_reporting::init` and `tauri_plugin_sentry::init`.
  - Profile import: `apps/desktop/src-tauri/src/profile_import/github.rs` (GitHub).
- Opt-in setting: `apps/desktop/src/renderer/store/preferences-schema/preferences-schema.ts` (company-logo preference, default OFF).
- Machine-readable inventory: `apps/desktop/src-tauri/tests/egress.rs` (EGRESS const).
- Audit finding: `p2-contra-cross-001`, §4 of the 2026-08-16 full-history audit.
  That report was a 1.5 MB one-off snapshot scoped to
  `v0.123.0`. It is not in the tree and, since the 2026-08-21 history rewrite,
  not in any commit either — read it at
  [AUDIT_REPORT.md on the `assets` branch](https://raw.githubusercontent.com/saeedkolivand/ai-job-hunter-app/assets/AUDIT_REPORT.md).
  Deliberately no longer cited by commit sha: this citation has now been broken
  by two separate rewrites, and a sha in a doc is precisely the drift-prone
  literal rule 17 forbids. The `assets` branch is parentless and force-pushed,
  so the URL is stable and the file costs no history. Its `path:line` citations
  point at the tree as of PR #1000, not this one.
