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
tracked job data, and credentials — lives in a local database on the device and is never
collected by telemetry or an app-operated backend. It is **not** a claim that this data
never leaves the machine: the app sends data out to services the user configures or invokes —
notably the **AI provider**, which receives the résumé and job text it is asked to generate
from — plus job-board scraping, opt-in web search, the updater check, user-typed location
autocomplete, and opt-in enrichment. See [ADR 0005](adr/0005-network-egress-privacy-boundary.md).
_Avoid_: "the only outbound calls are…" (an over-absolute phrasing — the boundary is about
storage/telemetry, not call count), "no network" / "fully offline" (untrue; scraping is core),
"personal data never leaves the device" (untrue — the AI provider receives the résumé + job text)

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

**Autofill** (a.k.a. assisted autofill):
The inverse of **Extension import**: the extension writes the user's **own** Contact
Profile fields (fullName, email, phone, location, linkedin, github, website) _out_ onto
the application form on the current page, instead of reading a job _in_. **User-initiated**
(a click, via `activeTab` + `executeScript` — no broad host access), fills **empty** fields
only via a generic label/`autocomplete`/`name` matcher, **never submits**, and is
**transparent** (shows what it filled). PII is **fetched fresh** from the desktop over the
same authenticated bridge (`profile.get` → `profile.result`) at fill time and **never
persisted** in `chrome.storage`. **Opt-in, default OFF, enforced desktop-side** — the app
refuses `profile.get` when the toggle is off. See [ADR 0009](adr/0009-assisted-autofill.md).
_Avoid_: auto-apply / auto-submit (autofill never submits — the human does), scrape
(that's the inbound import path), "fills every field" (empty + unambiguous fields only;
no file upload; complex ATS partial)

**Agency posting**:
A job posting whose company name matches one in the built-in or user-extended recruitment-agency list
(personnel staffing, HR consulting, temp placement agencies). Filterable via a renderer session flag
(`hideAgency` on `JobsSlice`). Distinct from a **normal posting** sourced directly from an employer or
ATS board.
_Avoid_: "staffing job", "recruiter posting" (agency posting is the precise domain term)

**Canonical member**:
The representative posting selected from a cluster to display as the single row. Chosen by a stable
precedence rule (has-description > direct board > aggregator > newest > key ascending). Never persisted;
recomputed at every ingest. The cluster's other members surface in the detail pane as "All sources" links.
_Avoid_: "cluster representative", "primary posting" (canonical member is the standard term)

**Cluster**:
A recomputed grouping of postings judged to be the same real-world job across multiple boards. Membership
is transient (never persisted, only displayed); clustering is deterministic and runs at every ingest after
scrape or autopilot refresh. User "not a duplicate" verdicts persist as pair tombstones. Clusters never
block results — a below-threshold cluster still displays all members.
_Avoid_: "duplicate group", "posting group" (cluster is the standard domain term); persisting clusters
in the database (they are recomputed on-demand)

**Pair tombstone**:
A persisted user verdict that two canonical job keys are NOT duplicates. Stored unordered as key pairs in
`dedup.db` (with invariant `key_a < key_b`). Survives re-scrapes and is used to block cluster joins
across the pair. Deletion and merge-back are deferred fast-follows.
_Avoid_: "split verdict", "negative dedup" (pair tombstone is the standard architecture term);
conflating with cluster membership (tombstones block joins, they don't define membership)

**Pairing token**:
The per-install shared secret that authenticates the browser extension to the local
bridge. **In protocol v2 it is never transmitted** — it is used only as the
**HMAC-SHA256 key** in a mutual challenge-response handshake (each side proves it knows
the token over exchanged per-connection nonces; token stays on both machines). Generated
on app first-run, persisted to the app data dir (per-user file perms), rotatable via the
`extension_bridge_regenerate_token` IPC command, and shown in Settings to copy/paste into
the extension. 256-bit random, lowercase hex. See [ADR 0010](adr/0010-bridge-hmac-handshake.md).
_Avoid_: API key, credential (it authenticates only to the local app, never remote);
"sent on every frame" / a `token` field on the wire (v1 behavior — v2 never puts the
token on the wire)

**Connection phase**:
The bridge/extension link state the popup renders. Canonical set: `app_not_running`
(desktop unreachable — also where an **ambiguous silent socket-close** folds, because the
v2 desktop rejects a bad proof by closing **without a reply**, indistinguishable from a
crash, so it stays recoverable), `searching` (probing/reconnecting), `not_paired` (no token
stored), `bad_token` (a real, **unambiguous** auth failure — the peer sent an `auth.ok`
whose **`serverProof` fails verification**, i.e. a rogue/mismatched server; the extension
sends zero PII), `outdated` (a **protocol-version mismatch** — a v2 extension reached a v1
desktop or vice versa; the user must update the app/extension), and `connected` (the
**mutual** handshake verified — the only state in which `import`/`profile.get` frames are
sent). See [ADR 0010](adr/0010-bridge-hmac-handshake.md).
_Avoid_: inferring `bad_token` from a silent close/timeout (v2 auth failure closes with no
reply → ambiguous with a crash → treat as recoverable `app_not_running`, never falsely
accuse a good token); an `auth_timeout` phase

**Scan mode** vs **URL mode**:
Two import paths on the extension bridge. **Scan mode** sends the rendered (authenticated)
DOM (`{ url, html }`) — no network call needed; the app parses the posted HTML via the
fetch-free parser. **URL mode** sends just the link (`{ url }`); the app's backend scraper
resolves the URL (headless browser, redirects, JavaScript rendering). Scan mode is
preferred when the extension can supply the DOM (no auth wall); URL mode is the fallback
for link-only saves or when the extension can't intercept the HTML.
_Avoid_: conflating with the headless scraper (which is the impl detail of URL mode)

## Domain — Export & templates

**Document accent**:
The per-export document color override — an optional hex applied to **one** exported
résumé or cover letter that recolors the chosen template's accent role. It is **not
persisted** and **never reads `ThemePrefs`**; `None` (the default) leaves the template's
built-in palette untouched. Distinct from the app-UI **accent color** — the interactive-
element tint of [ADR 0004](adr/0004-single-source-user-customizable-accent-color.md), a
durable user preference. See [ADR 0007](adr/0007-document-color-is-a-knob-not-a-template.md).
_Avoid_: accent color (ambiguous — that already means the app-UI tint), theme accent,
brand color

**Letter layout**:
The arrangement/composition of a cover letter — `classic` / `refined` / `banded` —
independent of the résumé template. The palette and fonts always **inherit** from the
selected résumé template (via `style_from_template`); market conventions (date position,
subject line, recipient block) own the semantics. A layout owns arrangement only.
_Avoid_: letter template, letter style (LetterStyle is the inherited-palette carrier in
code, not the arrangement)

**Template tier**:
The honesty label on a résumé template: `ats` (single-column, parser-safe) or `design`
(photo / multi-column, visually rich). Metadata only, **no render behavior** — it groups
the gallery and picks which templates surface the ATS-mode toggle. A design-tier template
collapses to a linear single column (and drops its photo) when ATS mode is on. See
[ADR 0007](adr/0007-document-color-is-a-knob-not-a-template.md).
_Avoid_: premium tier, template category

## Domain - Landing experience

**CURRENT ARCHITECTURE** — Landing is a **Next.js 15 static-export workspace package** at
`apps/landing/`, deployed as flat files to GitHub Pages via `pages.yml`. All authored pages
are routes (`home`, `creature`, `how-it-works`, `privacy`, `download`); third-party artifacts
(dashboards, benchmarks, storybook) stay in `public/` as passthrough. Parity gate (`check:parity`)
ensures byte-shape matching with the legacy static layout. See [ADR 0018](adr/0018-landing-nextjs-static-export.md)
for full decision record.

**SUPERSEDED** — The TERMINAL VELOCITY scroll-film (ADR 0016, 0015, parts of 0014) was
abandoned mid-M4 on 2026-07-20 after three merged milestones; all WebGL infrastructure, film
concepts (playhead, scroll-film, scenes, quality governor, VAT, shader standards), and
Experience-gate machinery are **retired**. Entries below are definitions for reading the retired
ADRs only; they are not in active use.

**Authored page**:
A hand-written, user-facing page in the landing site, ported as a Next.js route under
`src/app/`. Examples: home (root), creature, how-it-works, privacy, download. Distinct from
**Passthrough artifacts** (CI-owned, served verbatim from `public/`).
_Avoid_: "component" (pages are full routes), "generated page" (they are hand-authored)

**Passthrough artifact**:
Third-party or CI-owned files served verbatim from `public/` at build time (copied unchanged
by Next.js static export). Examples: benchmarks (index.html + data.js), storybook (when present). Never built or transformed by Next, only deployed
as-is. Distinct from **Authored pages** (hand-written routes) and **Docs routes** (e.g., `/agent-system`,
`/mission-control`, `/architecture-map` — typed-data Next.js routes under the docs tier).
_Avoid_: "static assets" (too generic), "public files" (ambiguous — could mean any `public/`
content)

**Marketing tier** vs **Docs tier**:
Two visual skins on the landing site. Marketing tier (pages 1–4: home, creature, how-it-works,
privacy) preserves the original hand-authored design and brand tone. Docs tier (planned future,
PR2–PR4: `/mission-control` + `docs/` pages) uses a unified, separate visual language. They
share no design language; marketing skin is protected from future refactoring. See
[ADR 0018](adr/0018-landing-nextjs-static-export.md).
_Avoid_: "landing pages" / "public pages" / "web pages" (imprecise); using one tier's design
for the other

**Mission control**:
The full-repo dashboard at `/mission-control` (PR2, ADR-0018): a single-page app that
surfaces repo-wide metrics (releases, recent changes, DORA-lite health), accessible to signed-in users
via GitHub PAT. Shipped in PR2; replaces the scattered `ci-dashboard.html` passthrough with a unified
docs-tier interface. Supports safe-tier write actions (e.g. manual workflow dispatch). The old
ci-dashboard URL is a clean rename with no redirect stub (owner decision).
_Avoid_: "admin panel" (it is a dashboard, not an admin control), "metrics page" (it does more
than metrics)

**Semantic layer**:
The prerendered content HTML that is always in the DOM - what the visitor reads when the GL
experience does not mount. Same page, not a second destination.
_Avoid_: "fallback page" as a separate URL

**Experience gate**:
The single capability check that decides whether the GL experience mounts over the Semantic
layer. See the ADR for the exact conditions.
_Avoid_: scattered feature detection (the decision lives in one gate)

**TERMINAL VELOCITY**:
The active landing concept ([ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md)): a
realistic CG **scroll-film** (~2:40) retelling `landing/index.html`'s story as one continuous
vertical fall-then-rise, scrolled to watch. Replaces RIPBOOK. The name plays on two meanings:
the screen (terminal) and the physics of the fall (terminal velocity).
_Avoid_: RIPBOOK / notebook (retired), "the landing animation" (it is a directed film, one shot)

**Scroll-film**:
The form TERMINAL VELOCITY takes: a single continuous camera shot where **scroll is the
playhead**, fully reversible, with no page/section cuts. Distinct from the retired RIPBOOK
per-Page notebook model.
_Avoid_: slideshow, scrollytelling sections (there are no discrete DOM sections driving it - one
timeline); "video" (it is real-time GL, not a media file)

**Playhead**:
The film's 0->100% timeline position, mapped 1:1 from native scroll (~2:40 of runtime).
Reversible - scrolling up rewinds. Interactions never move it (determinism is load-bearing).
_Avoid_: "scroll progress" used loosely (the playhead is a time axis the whole render is a pure
function of); global `t` (that was the RIPBOOK scroll variable)

**Scroll map** (the 9 scenes):
The ordered set of nine scenes the playhead runs through: Cold open, The canyon, The surface
(splash), The deep, Blackout, The catch, The ascent, Dawn, Finale/credits. See the ADR for
each scene's range and beat. The TERMINAL VELOCITY analogue of RIPBOOK's 9 Pages, but scenes
are camera moments in one shot, not discrete rippable pages.
_Avoid_: Pages (retired), Beats (the older 0014 term), "chapters" when the scene ranges are meant

**Depth gauge**:
The film's diegetic progress indicator - meters below sea level shown in the chrome. Because the
whole film is one vertical axis, depth IS progress; paired with the timecode (00:00 -> 02:40).
Replaces RIPBOOK's Desk-pile odometer.
_Avoid_: progress bar (there is none - depth + timecode are the only progress UI), Desk pile (retired)

**Letterbox chrome**:
The cinematic frame drawn over the film: top/bottom bars carrying hand-lettered act titles and
captions, plus the always-available projector-slate menu chip (download, GitHub, privacy, the
creature, skip-to-end). The only web chrome besides the timecode + depth gauge.
_Avoid_: "the nav bar" / "the header" (it is diegetic film framing, not web UI); HUD (that names
the robot-lens in-world overlay, a different thing)

**Style frames**:
The nine approved FLUX.2 color-script frames rendered 2026-07-18 - the look-dev ground truth for
grade, lighting arc, and mood per scene. Stored outside the repo; referenced, never checked in.
_Avoid_: concept art (broader), storyboard (that is shot blocking, not the color script);
treating them as shippable assets (they are reference only)

**VAT** (Vertex Animation Texture):
A baked simulation stored as a texture and played back by sampling at time t (via `three-vat`) -
used for the splash crown (a Houdini FLIP bake). Scrubbing a VAT is deterministic and reversible
for free, which is why heavy sims are baked, not run real-time.
_Avoid_: "the water sim" (the bounded Gerstner water patch is separate and real-time); running
FLIP live (the crown is pre-baked)

**Quality governor**:
The runtime tiering system (TERMINAL VELOCITY): detect a startup tier at load, then dynamically
adjust at runtime via a frame-time loop with **hysteresis** - downgrade when performance drops
below a threshold, upgrade when it recovers above a higher threshold, with a cooldown between
changes. Turns knobs in a fixed priority order: pixel ratio, post samples, geometry density,
effect toggles. Thresholds and per-tier ladders are locked in [ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md).
_Avoid_: "auto quality" (the order and hysteresis are deliberate, not a bare fps flip-flop);
the RIPBOOK boil-tier scheme (retired)

**Journey** (superseded - [ADR 0015](adr/0015-ripbook-notebook-landing.md), then [ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md)):
Was the 8-Beat scroll-scrubbed camera ride (0014). Retired. The active landing is the TERMINAL
VELOCITY **scroll-film** - one continuous shot scrolled as a playhead through 9 **scenes**. Use
scroll-film / playhead / scene, not Journey (and not the RIPBOOK Page/Rip either).
_Avoid_: reusing "Journey" for the new model; overloading Autopilot (the app's job-application run)

**Beat** (superseded - [ADR 0015](adr/0015-ripbook-notebook-landing.md), then [ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md)):
Was one of the 8 places the Journey's camera visited (0014). Retired. The TERMINAL VELOCITY unit
is a **scene** (one of 9); the intervening RIPBOOK **Page** is also retired.
_Avoid_: reusing "Beat" for a scene or a page

**Page** (superseded by [ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md)):
Was one of the 9 RIPBOOK notebook pages. Retired with the notebook. The TERMINAL VELOCITY unit
is a **scene** in one continuous shot - see Scroll map. No pages, no per-page Exit.
_Avoid_: reusing Page for a TERMINAL VELOCITY scene; section / panel (those name the semantic-layer DOM)

**Exit** (superseded by [ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md)):
Was the trailing animation that played a RIPBOOK Page out (a Rip, hinge-open, or sign/stamp).
Retired - the scroll-film has no per-page exits, only continuous camera motion.
_Avoid_: reusing Exit for TERMINAL VELOCITY scene transitions (there are no cuts)

**Rip** (superseded by [ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md)):
Was the usual RIPBOOK Exit - a scrubbed tear/crumple/fold of a Page. Retired with the notebook;
TERMINAL VELOCITY has no rips.
_Avoid_: reusing Rip for any TERMINAL VELOCITY motion

**p-space** (superseded by [ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md)):
Was RIPBOOK's page-local progress `p` (a Page's slice of the global scroll `t`). Retired.
TERMINAL VELOCITY drives everything off one **playhead** (0->100%), not a per-page remap.
_Avoid_: reusing `p`/`t` for the playhead; per-scene progress remaps (the film is one timeline)

**Desk pile** (superseded by [ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md)):
Was RIPBOOK's stack of exited Pages serving as the progress odometer. Retired. TERMINAL VELOCITY
shows progress via the **depth gauge + timecode**.
_Avoid_: reusing Desk pile for any TERMINAL VELOCITY indicator

**Foley**:
The procedural sound effects synthesized in-app, spatialized to the cursor. Under TERMINAL
VELOCITY these are film-world sounds (paper snap, ripple, servo beep); the RIPBOOK set (rip,
crumple, scribble, stamp) is retired. Distinct from the Gibberish voice.
_Avoid_: sound assets / audio files (Foley is synthesized, not sampled)

**Gibberish voice**:
The voice synth that mutters as the protagonist "speaks" - non-lexical, silenced by the "mute the
guy" toggle. Carries over to TERMINAL VELOCITY (e.g. the poke-to-flail yelp with doppler).
_Avoid_: narration / TTS (it renders no real words)

**Passthrough files**:
The `landing/` files copied verbatim into the exported site by the postbuild
merge-passthrough script - source that ships unchanged.
_Avoid_: "static assets" (too generic; see the ADR for the merge mechanics)

**Line boil** (superseded by [ADR 0016](adr/0016-terminal-velocity-scroll-film-landing.md)):
Was the shader-driven wobble of ink strokes (`uBoil`) that gave RIPBOOK its hand-drawn look.
Retired - TERMINAL VELOCITY is a PBR realistic film, no ink boil. The only "on twos" that
carries over is character animation stepped at ~12 fps against a smooth 60 fps camera.
_Avoid_: reusing Line boil / `uBoil` for any TERMINAL VELOCITY effect
