# AI Job Hunter — Context Glossary

Shared vocabulary for the AI Job Hunter desktop app, resolved during design/grilling
sessions. This is a **glossary only** — definitions of project-specific terms, not a
spec and not implementation notes. Pick one word per concept; list rejected synonyms
under _Avoid_. Grow it lazily as terms are resolved.

## UI controls (`@ajh/ui` design system)

**Switch**:
The binary on/off control rendering ARIA `role="switch"` (a sliding track + thumb). The
single `@ajh/ui` primitive for boolean toggles; supports a `size` and an optional
inline label.
_Avoid_: Toggle, ToggleSwitch, checkbox (when a switch is meant)

**SegmentedControl**:
The control for choosing exactly one option from a small fixed set of mutually-exclusive
segments. Distinct from a Switch, which is boolean.
_Avoid_: Tab bar, radio group (when the segmented control is meant)

## Domain — Privacy & network

**Local-first privacy boundary**:
The guarantee that the user's **personal data** — résumés, generations, applications,
tracked job data, and credentials — never leaves the device (no telemetry, no
app-operated backend). It is **not** a claim that the app makes no network calls: permitted
egress is enumerated by class (AI provider, job-board scraping, opt-in web search, updater
check, user-typed location autocomplete, and opt-in enrichment). See [ADR 0005](adr/0005-network-egress-privacy-boundary.md).
_Avoid_: "the only outbound calls are…" (an over-absolute phrasing — the boundary is about
personal data, not call count), "no network" / "fully offline" (untrue; scraping is core)

**Enrichment egress**:
An outbound call that trades a **public identifier or a user-typed query** for optional
presentation data (e.g. a company name → Clearbit logo, a typed place → OpenStreetMap
city suggestions). Must be **opt-in, default OFF, CSP-scoped** to the minimum hosts, and
send no personal data. Distinct from **core egress** (AI provider, scraping) which is
required for the feature to function.
_Avoid_: tracking, telemetry (enrichment is user-triggered and sends no behavioral data)

## Domain — Matching

**Match score**:
The 0–100 fit signal between one résumé and one job posting: an **ATS/keyword** coverage
component always, plus an optional **semantic** (embedding-similarity) component. Computed
**on demand, per job**, when the user opens a posting — not batch-precomputed.
_Avoid_: "relevance", "rank" (the score is per-pair fit, not a cross-posting ordering)

**Semantic scoring** (a.k.a. hybrid matching):
The optional embedding-similarity half of a Match score. **Default OFF** (keyword-only);
opt-in via the Embeddings setting. When a caller omits the flag, the default is the
documented app-wide default — **OFF**. Turning it on costs an embedding call and yields a
blended score (semantic + ATS).
_Avoid_: defaulting semantic ON for any caller (including agent tools); "hybrid search"
(the removed Cmd+K surface — gone, not this)

## Domain — Applications

**Application**:
A user's tracked pursuit of one job. The shared spine that the three application
origins (the AI Generate page, an Autopilot run, the Jobs/postings page) all write to,
and that the Documents page reads. It is distinct from the docs produced for it: one
Application can have zero or many tailored resumes/cover letters. Its lifecycle status is
user-mutable (it is _not_ the derived "applied" badge, which only asks "does a generation
exist for this URL?").
_Avoid_: Job (a posting is not an application), Generation/SavedDocument (the produced
artifact is not the pursuit), Interaction (a one-off logged event, not the mutable state)

**Application status**:
The user-mutable lifecycle stage of an Application. Canonical set (registry-backed,
any→any transitions — truth-tracking, not an enforced workflow):
`saved → applied → screening → interviewing → offer → accepted`, plus the off-funnel
outcomes `rejected`, `ghosted`, `withdrawn`. Terminal: `accepted`/`rejected`/`withdrawn`;
`ghosted` is soft-terminal (reopenable). `saved` is the only pre-apply stage.
_Avoid_: JobStatus (already taken — the async-execution lifecycle of a background job:
queued/running/streaming/…); "stage" and "status" are used interchangeably for this.

**Generation** (a.k.a. Document):
A produced artifact for an Application — a tailored résumé and/or cover letter, plus its
mode/languages/answers/brief. Formerly the `ai_generations` row doubled as "the
application aggregate"; that role moves to the **Application** table. A Generation is now a
**child** of an Application (`application_id`); an Application has zero or one child Generation
today—the save path merges by normalized URL and the single row carries both the résumé and
cover-letter columns. The FK permits many, reserved for a future flow that splits them.
_Avoid_: Application (the pursuit ≠ the artifact); calling the generation row "the
application aggregate" (historical comment — superseded)

**Status event**:
One immutable record of an Application status transition (`from → to` at a timestamp, with
optional note). The append-only history behind the timeline/funnel; current status is the
Application's own field (not derived) for cheap querying.
_Avoid_: Interaction (the viewed/opened/applied event log on postings is a separate concept)

**Application origin**:
Where an Application was first created. Canonical triggers: **Save** (a Jobs-page posting
or browser-extension import → `saved`), **Generate** / **Apply** (AI Generate or Autopilot
produce/submit docs → `applied`), **Manual** ("Track a job" — e.g. applied outside the
app), and **Backfill** (legacy row migration). Dedup on create by normalized `jobUrl`; no
URL ⇒ a new Application. The browser extension is a **Save** origin (Feature 2).
_Avoid_: source (already means the job-board id on a JobPosting, e.g. greenhouse)

**Extension import**:
Importing a job from the browser via the local WebSocket bridge (Feature 2). The
extension sends `{ url }` (URL mode, app resolves) or `{ url, html }` (Scan mode, app
parses the authenticated DOM). The browser is a **Save** Application origin. The bridge
server listens on `127.0.0.1` loopback, validates the Origin header and per-frame pairing
token, and guards the import URL against SSRF (DNS-rebinding-safe via IP pinning).
_Avoid_: scrape (the headless board path), Apply (extension imports are Saves)

**Pairing token**:
The per-install secret the browser extension sends on every bridge frame (`token` field).
Generated on app first-run, persisted to the app data dir, and rotatable via the
`extension_bridge_regenerate_token` IPC command. Shown in Settings for the user to
copy/paste into the extension. A mismatch closes the socket; the pair is 256-bit random,
lowercase hex.
_Avoid_: API key, credential (it authenticates only to the local app, never remote)

**Connection phase**:
The bridge/extension link state the popup renders. Canonical set: `app_not_running`
(desktop app unreachable), `searching` (probing/reconnecting — also the state a transient
auth **timeout** folds into: transient → retry), `not_paired` (no token stored), `bad_token`
(a wrong token, surfaced only via the server's **explicit error reply** before it closes the
socket — never inferred from a timeout), and `connected` (auth handshake succeeded). A
handshake timeout is a transport failure (→ `searching`), not a token failure.
_Avoid_: an `auth_timeout` phase (a timeout is transient, not a distinct terminal state);
treating a timeout as `bad_token` (falsely accuses a good token)

**Scan mode** vs **URL mode**:
Two import paths on the extension bridge. **Scan mode** sends the rendered (authenticated)
DOM (`{ url, html }`) — no network call needed; the app parses the posted HTML via the
fetch-free parser. **URL mode** sends just the link (`{ url }`); the app's backend scraper
resolves the URL (headless browser, redirects, JavaScript rendering). Scan mode is
preferred when the extension can supply the DOM (no auth wall); URL mode is the fallback
for link-only saves or when the extension can't intercept the HTML.
_Avoid_: conflating with the headless scraper (which is the impl detail of URL mode)
