# Audit Triage — Develop / Modify Worklist

Decisions from the grill-with-docs session working through `AUDIT_REPORT.md` §6.
Each entry: the decision, the concrete work, and the affected surface. Implementation is
delegated separately (Opus, xhigh) — this file is the agreed scope, not the code.

Status legend: **DECIDED** (direction locked) · **BUILD** (needs new code) · **MODIFY**
(change existing) · **DOC** (docs only) · **DROP** (delete/close).

---

## Q1 — Privacy-guarantee accuracy · DECIDED · DOC

**Decision:** The local-first guarantee is about _personal data_ (résumés, generations,
applications, job data, credentials), not zero network calls. No behavior changes — every
current outbound call already complies (scraping = core & disclosed; geocoding =
user-typed typeahead; updater = version check; Clearbit = opt-in/default-OFF). Codified in
[ADR 0005](adr/0005-network-egress-privacy-boundary.md) + CONTEXT.md.

**Work (DOC):**

- Rewrite the privacy claim in `README.md` ("What It Does") and `SECURITY.md` ("Security
  posture") to (a) keep the personal-data guarantee and (b) enumerate the six egress
  classes instead of claiming "the only outbound calls are the AI provider + web search."
- No code changes.

**Rejected:** master airplane-mode switch; gating geocoding/updater. Revisit airplane-mode
only if users ask for a hard offline mode.

---

## Q2 — Semantic-matching default + Cmd+K removal · DECIDED

### Q2a · MODIFY — flip omitted-flag semantic default ON→OFF

UI path already defaults keyword-only (correct). Only the agent `match_resume` tool omits
the flag and silently gets semantic ON (extra cost + scores that disagree with the UI).
**Work:** `apps/desktop/src-tauri/src/commands/match_resume.rs` — `semantic_enabled_bit(None)`
`1`→`0`; update the pinning test (~L572) to `None → 0`. Omitted flag = documented default
(OFF). Glossary: CONTEXT.md "Semantic scoring".

### Q2b · DROP — delete the Cmd+K command-palette / hybrid-search plumbing (gone for good)

Hybrid-search UI was removed; command-palette event plumbed end-to-end with a stale
"intentionally not a command palette" comment.
**Work (crosses the IPC seam — trace + remove all):** `shortcut:command-palette` emitter
(Rust), `packages/shared/src/events/shortcuts.ts` + `events/index.ts`, contract
`packages/shared/src/ipc/contracts/shortcuts.ts` (`onCommandPalette`), tauri-client
`namespaces/shortcuts/shortcuts.ts`, `useCommandPaletteShortcut` in
`renderer/services/use-support/use-support.ts`, mock-client `onCommandPalette`, and the
stale comment in `renderer/hooks/use-keyboard-shortcuts.ts:40`. Run `gen:ipc:check` after.

---

## Q3 — `match_resume_batch` ghost · DECIDED · DROP

On-demand per-job scoring is the final design; batch has zero consumers (dead IPC
capability end-to-end). Superseded precompute-all-postings (REQ-13028).
**Work (remove across the seam):** Rust `match_resume_batch` fn + `MATCH_BATCH_MAX` +
batch tests in `commands/match_resume.rs`; registration `lib.rs:828`; contract `resumeBatch`

- `match:resume_batch` channel in `packages/shared/src/ipc/contracts/match.ts`; client
  `tauri-client/namespaces/match/match.ts`; mock `resumeBatch: noop`;
  `MatchResumeBatchRequestSchema` + type + boundary tests in `packages/shared/src/schemas`.
  Run `gen:ipc:check`. No rank-all-postings feature planned.

---

## Q4 — Extension auth-handshake timeout · DECIDED · DOC (keep behavior)

Wrong token is already surfaced correctly (explicit error reply → `bad_token`). A timeout is
ambiguous; treating it as transport-failure → `searching`/reconnect is correct and
self-correcting. `bad_token`-on-timeout would falsely accuse a good token; an `auth_timeout`
phase adds a 5th failure view for a transient condition. No behavior change.
**Work (DOC):** document the handshake phase model (`app_not_running / searching /
not_paired / bad_token / connected`; wrong-token=explicit reply, timeout=transient→retry) in
the extension-bridge module docs / docs; correct the misstated ledger note. Glossary:
CONTEXT.md "Connection phase".

---

## Q5 — Semver policy · DECIDED · DOC (stay 0.x)

`.releaserc.json` (breaking→minor, the 0.x guard) is authoritative and has been the real
behavior all along (at v0.123 — breaking→major would be past v40). Docs drifted.
**Work (DOC):** update CLAUDE.md release table + release docs (DEPLOYMENT.md) to state
`BREAKING CHANGE → minor while 0.x` (not major). No config change. 1.0 trigger left open
(revisit when declaring a stable API).

---

## Q6 — `/documents` canonical · DECIDED · DOC

`/documents` is the only route; zero `/resumes` references anywhere (Tauri = no external
bookmarks). No redirect needed.
**Work (DOC):** mark the REQ-16240 "keep /resumes" clause superseded in its doc/ledger note.
No code change.

---

## Q7 — `structuredOutputFor` scaffolding · DECIDED · DROP

Unconsumed helper; native structured output is cloud-only and only _augments_ the existing
free/local `validateAndRepair`/`validateMetadata` path (can't replace it). Modest ROI (repair
is a free local salvage, not a costly re-prompt); dual-path + per-provider wiring not
justified until real evidence of parse failures.
**Work (DROP):** delete `structuredOutputFor` in `packages/prompts/src/provider/index.ts`
(+ the `structuredOutputFor` describe block in `provider.test.ts`) and the README row that
advertises it. Keep `resolveProfile`/`structuredOutput` boolean (used elsewhere) — remove
only the unwired schema-spec helper. Revisit if analysis/metadata `null`-repair rate hurts.

---

## Q8 — PDF/UA tagged accessibility · DECIDED · DOC + tiny BUILD

Full PDF/UA-1 is a multi-week project (all-or-nothing standard: tag tree, per-template alt
text, nested headings, veraPDF validation) on a still-maturing Typst feature, for a narrow
screen-reader audience — ATS get ZERO benefit from tags. Never implemented; aspirational claim.
**Work:**

- (DOC) Remove the PDF/UA / tagged-output promise from resume-export-standards + docs; restate
  the true posture (single-column, selectable-text, logical order, ATS-safe); note full PDF/UA-1
  as a future goal pending Typst maturity.
- (tiny BUILD, optional) Set PDF document **language + title** metadata in the Typst engine
  (`export/typst_engine/engine.rs`, via `set document(...)`) — cheap screen-reader win, NOT the
  PdfStandard enforcement. Keep `PdfOptions::default()` otherwise.
- Park real PDF/UA-1 behind its own ADR if/when committed.

---

## Q9 — Splash app_ready / theme-mirror follow-ups · DECIDED · CLOSE (obsolete)

Native splash + `app_ready` IPC + theme-mirror fully removed (#475); React `AppSplash` is the
current splash (gates readiness client-side). Zero remnants. Follow-ups target a deleted
architecture.
**Work:** none — mark REQ-16637 (add app_ready to SystemContract) and REQ-16638 (splash in L3
doc list) closed-obsolete in the ledger. No code/docs.

---

## Q10 — Deferred UX/roadmap cluster · DECIDED (per item)

1. Jobs LinkedIn redesign (16642) · CLOSE — shipped via #499; ledger boxes stale.
2. Scraped-salary badge (16659) · CLOSE (YAGNI) — data already feeds salary answers; visible
   badge is cosmetic, not built.
3. Catalog-derived AUTH_BOARDS (16630) · DEFER — not built; keep on roadmap, low urgency.
4. Adzuna.de depth monitoring (16681) · CLOSE — ops task, not a code item.
5. Empty-release-notes guard (16623) · SCHEDULE (small BUILD) — real recurring bug
   (conventionalcommits v10 empties notes, broke v0.119.0); add an empty-notes-body guard to
   `.github/workflows/release.yml` alongside the existing `-z "$VERSION"` guards.

---

## Q11 — Dead Support diagnostics subtree · DECIDED · DROP + tiny BUILD → ADR 0006

Support is intentionally FAQ-only. The 22-component diagnostics/health/recovery/KB dashboard
is abandoned (unreachable since day one, backend never built, false-success on destructive
resets). Delete it; keep FAQ; salvage export-diagnostics into Settings.
**Work:**

- (DROP) Delete 22 unrendered components under `features/support/components/` (keep only
  `SupportPage` + `support-data.ts` FAQ parts); the diagnostic parts of `support-data.ts`; the
  unregistered `support`/recovery command contracts + `tauri-client/namespaces/support` methods
  they use; and the ~45 orphaned support/diagnostics i18n keys (en + de).
- (BUILD, small) Move the export-diagnostics "submit bug report" action into Settings (a single
  action); `export_diagnostics` command already registered — just needs a reachable host.
- Run `gen:ipc:check` + `i18n:extract` after; resolves findings renderer-feat-3-001,
  p2-b1-ipc-chain-001/002, and a big share of the missing/orphaned i18n-key findings.
- Security: the deleted recovery/reset commands were never registered (no security exposure);
  no new destructive command is added.

---

## Q12 — Factory reset leaves browser sessions · DECIDED · MODIFY (privacy fix)

Factory reset promises "fresh-install state" + wipe all stores, but `disconnect()` only writes
status=false + removes the cookies snapshot — it never deletes `browser-state/<board>/profile/`
(the Chromium user-data-dir with live sessions). Contradicts the reset's promise + ADR 0005.
**Work (MODIFY, Rust):** in `commands/privacy.rs::privacy_reset_app`, after the per-board
`disconnect()` calls, best-effort `remove_dir_all` the `browser-state/` tree, tolerating
locked-file failures (log what remains). Leave mid-session `board_login::disconnect()` as-is
(files may be locked while Chromium runs; scraper already honors the status flag).

---

## Q13 — OS-encryption detection + type drift · DECIDED · MODIFY (2 parts)

Doubly-dead warning: Rust returns `{available:bool}` (object) vs contract/mock/consumer bare
`bool`, AND `is_available()` hardcoded `true`. "OS keychain" is a headline SECURITY.md claim;
don't simplify away a security warning.
**Work (MODIFY):**

- Fix drift: `commands/credentials.rs::credentials_available` returns a bare `bool`
  (`json!(guard.is_available())` / typed `bool`) — aligns Rust with `contracts/credentials.ts`
  (`Promise<boolean>`), mock, and the `AccountsSettingsTab` `=== false` gate.
- Real detection: make `credentials/mod.rs::is_available()` a real keyring probe (set/get/delete
  a sentinel once; false on failure) so the "no OS encryption" warning fires on platforms
  without secure storage (e.g. Linux w/o Secret Service). Keep true on Win/macOS.

---

## Q14 — Prior analysis/audit asks · DECIDED · CLOSE (superseded)

This full-history audit is a strict superset of the truncated prior fleet-audits (which cite
stale pre-rename `apps/tauri/**` paths) and the analysis-only asks. Nothing actionable remains
beyond §4.
**Work:** none — close REQ-10036/10037 (fleet-audit tails), REQ-08038 (cache/storage), REQ-13037/
15008 (MCP liveness), REQ-14005 (/security-review gate) as superseded. Keep REQ-13006
(import/export end-to-end) as an OPTIONAL low-priority `verify`/E2E task (test-coverage gap, not
an audit gap).

---

# Unambiguous findings — no decision needed, just fix

These §4 findings don't need a grill decision; they're clear fixes. Included so the worklist is
complete.

- **[P1] Five missing i18n keys** (findings i18n-001, p2-m-i18n-001, p2-b5-translations-seam-001
  — audit top #1–#3): live, mounted components render raw key strings. Add the 5 keys to en + de:
  `ResumePreferences` (index.tsx:71/73), onboarding `BrowserErrorState` (index.tsx:32/34),
  `CloudProviderConfig` (index.tsx:109). Distinct from the ~45 orphaned Support keys (Q11). BUILD.
- **[P2] Autopilot scrape progress dropped** (finding p2-b7-events-deeplink-002, top #8): Rust emits
  `scrape:progress`/`scrape:item` but no renderer listener subscribes (autopilot's only delivery
  channel). Wire a listener or remove the emit — confirm intended UX first. MODIFY.
- **[P3] 15-minute date filter never shipped** (finding scraping-b-002, top #10): finest posted-date
  preset is 30m in both the TS schema and the Rust mirror though a 15m preset was requested. Add
  15m to `ipc_contracts/date_filters.rs` + the TS schema, or drop the ask. Small MODIFY.
- **[P3] Doc drift batch** (largest finding class, 84 findings): board counts (README 16 vs registry
  23), bundled fonts (docs say Noto Sans; engine ships Carlito/Inter/Source Serif/Manrope), storage
  (docs mention deleted app.db + LanceDB vs per-domain SQLite), accent color (docs violet vs app
  teal), release trigger (a skill says "on push to main" vs manual-dispatch). DOC sweep.

---

# Consolidated Worklist (prioritized)

**P1 — user-facing / security / privacy (do first)**

1. Q11 · DROP+BUILD · delete the 22-component Support diagnostics subtree + its contracts + ~45
   orphaned i18n keys; salvage export-diagnostics into Settings (ADR 0006). Clears the biggest
   finding cluster.
2. i18n · BUILD · add the 5 missing keys to en+de (top findings #1–#3).
3. Q13 · MODIFY · fix `credentials_available` type drift (bare bool) + real keyring probe.
4. Q12 · MODIFY · factory reset best-effort deletes `browser-state/<board>/` profiles (privacy).
5. Q2a · MODIFY · flip `semantic_enabled_bit(None)` 1→0 (stop agent tool silently running embeds).

**P2 — coherence / cleanup** 6. Q3 · DROP · delete `match_resume_batch` capability (IPC seam). 7. Q2b · DROP · delete Cmd+K command-palette plumbing (IPC seam). 8. Q7 · DROP · delete `structuredOutputFor` helper + tests + README row. 9. Q10#5 · BUILD · empty-release-notes guard in `release.yml`. 10. Q8 · BUILD(tiny) · PDF document language+title metadata. 11. p2-b7 · MODIFY · autopilot scrape-progress listener (confirm UX).

**P3 — docs / ledger (batch)** 12. Q1 · DOC · rewrite privacy claim (README, SECURITY) per ADR 0005. 13. Q5 · DOC · semver docs → breaking→minor while 0.x (CLAUDE.md, DEPLOYMENT). 14. Q8 · DOC · drop PDF/UA claim, restate export a11y posture. 15. Q4 · DOC · document extension handshake phase model. 16. Q6 · DOC · mark /resumes clause superseded. 17. Doc-drift sweep (board counts, fonts, storage, accent color, release trigger). 18. scraping-b-002 · MODIFY · add 15m date-filter preset (or drop). 19. Ledger closes: Q9, Q10 (#1/#2/#4 close, #3 defer), Q14 (prior audits superseded).

**ADRs written this session:** 0005 (network egress / privacy boundary), 0006 (Support FAQ-only,
diagnostics removed). **Glossary terms added:** Local-first privacy boundary, Enrichment egress,
Match score, Semantic scoring, Connection phase.

**Deferred (not scheduled):** Q10#3 AUTH_BOARDS catalog; Q14 import/export E2E verify; full PDF/UA-1.
