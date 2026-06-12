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

**Scan mode** vs **URL mode**:
Two import paths on the extension bridge. **Scan mode** sends the rendered (authenticated)
DOM (`{ url, html }`) — no network call needed; the app parses the posted HTML via the
fetch-free parser. **URL mode** sends just the link (`{ url }`); the app's backend scraper
resolves the URL (headless browser, redirects, JavaScript rendering). Scan mode is
preferred when the extension can supply the DOM (no auth wall); URL mode is the fallback
for link-only saves or when the extension can't intercept the HTML.
_Avoid_: conflating with the headless scraper (which is the impl detail of URL mode)
