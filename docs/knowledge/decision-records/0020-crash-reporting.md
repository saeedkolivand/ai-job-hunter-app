---
status: accepted
---

# Crash reporting via Sentry, default ON, desktop only

## Context

Until now the app was blind in production. A panic hook appends to `crashes.log`
(`apps/desktop/src-tauri/src/lib.rs`) and a redacted diagnostics zip
([ADR 0027](../../knowledge/decision-records/adr-027-diagnostics-bundle-privacy-boundary.md))
lets a user export it — but both require the user to notice a failure, find the
export, and attach it to a GitHub issue by hand. Nobody reports a crash they did
not see. The renderer was worse: React errors caught by a boundary never reached
`window`, so they were invisible even in principle.

This directly contradicted published product copy. The privacy policy named
Sentry, by name, as absent, in three places, and [ADR 0005](0005-network-egress-privacy-boundary.md)
defined the local-first guarantee as "no telemetry and no app-operated backend".
Adding remote crash reporting is therefore a **product decision that reverses a
published promise**, not a technical detail — recorded here so the reversal is
explicit and dated rather than discovered later as drift.

## Decision

Adopt **Sentry** (SaaS, free Developer plan: 5k errors/month, 30-day retention)
for the **desktop app only**, via `tauri-plugin-sentry`.

**Default ON**, revocable in the first-run wizard and in Settings → Privacy.
This departs from ADR 0005's rule 6 ("opt-in, default OFF"), which governs
_enrichment_ egress. The departure is deliberate: opt-in crash reporting on a
small user base collects nothing, and a crash reporter that reports no crashes
is just complexity. The owner made this call knowingly.

Four things make the departure survivable, and all four are load-bearing:

1. **Consent gate before transmission.** The persisted state is
   `{ enabled, consentShown }` and the only predicate that permits sending is
   `enabled && consentShown`. The default is enabled but _not shown_, so nothing
   is transmitted until the wizard has actually put the choice in front of the
   user. A default nobody saw is not consent, and the gap between a consent UI
   and what the code does is exactly where privacy claims break.
2. **Redaction of every outgoing event.** Events are serialized, every string
   value passed through the same `redact_token` pipeline the diagnostics bundle
   uses (paths, URLs, hosts, credentials, e-mails), then deserialized.
   Whole-event rather than field-by-field, because a field list is a denylist
   that rots as the SDK adds fields. `send_default_pii` is off and `server_name`
   is overridden — the SDK otherwise sends the machine hostname, which on a
   personal device is frequently the owner's real name.
3. **No DSN outside release builds.** `build.rs` bakes the DSN from an
   environment variable set only in the signed release job, read via
   `option_env!`. Local builds, contributor clones and every CI check compile to
   `None` and are physically incapable of transmitting.
4. **The extension is excluded.** `apps/extension/src/manifest.ts` declares
   `data_collection_permissions: { required: ['none'] }` to Firefox AMO.
   Including the extension would make a filed store declaration false.

The landing site is also excluded: `CspMeta.tsx` carries a hard origin invariant
banning third-party JavaScript, because any foreign script on that origin could
read the mission-control PAT from `localStorage`.

## Consequences

- ADR 0005 gains an eighth permitted egress class, and its guarantee no longer
  says "no telemetry". README, SECURITY.md, ARCHITECTURE.md, CONTEXT.md and the
  landing privacy copy were rewritten in the same change.

  **Corrected later — the rewrite was incomplete and nothing caught it.** Only
  the _prose_ moved. Three _summary_ claims kept saying "zero telemetry":
  README's Privacy bullet (~70 lines below the paragraph that correctly names
  Sentry, in the same file), the landing home page's Privacy-first card, and the
  marketing asset copy in `branding/marketing/build.mjs` — the last of which
  pairs it with "run 100% offline", so the two claims compounded. This entry
  also credited `check-parity.mjs` and `check-landing-drift.mjs` with forcing
  code and copy to move together; neither mentioned Sentry or telemetry.
  `check-parity.mjs` is a copy-_preservation_ gate for the Next port — it
  asserts phrases survive, it cannot tell a surviving phrase from a true one.
  `check-landing-drift.mjs` now carries the actual guard (check 6): a denylist
  for `zero/no telemetry` over README, SECURITY.md, `branding/` and the landing
  source, allowlisting only the extension-scoped claim, which stays true.

  The general lesson is that a reversal has to be swept across every surface
  that repeats the promise, headline claims included — and that a claim with no
  mechanical guard will drift back regardless of what an ADR says about it.

- `/privacy` is the privacy-policy URL filed with **both** the Chrome Web Store
  and Firefox AMO. It now has to state desktop collection and extension
  non-collection as clearly separate sections, or a reviewer reads one as the
  other while the manifest still declares `['none']`.
- **A failed symbol upload is made loud, never fatal — and that asymmetry is a
  decision, not an oversight.** A failing symbol upload must not be able to skip
  the jobs that publish `latest.json`; losing symbols costs readable stack
  traces, whereas losing the updater manifest breaks auto-update for every
  existing user while the installers already sit on the release. Making the
  downstream jobs tolerant instead was rejected for the mirror-image reason: it
  would publish an updater manifest even when a platform's build genuinely
  failed. "Not configured" must also stay distinguishable from "configured
  wrong", so absent secrets skip while a real failure is surfaced.
  The exact ordering and failure plumbing live in
  [`.github/workflows/release.yml`](../../../.github/workflows/release.yml), which
  is authoritative — do not restate them here.
- The release profile keeps symbols (`debug = "line-tables-only"`,
  `split-debuginfo = "packed"`, `strip = "debuginfo"`) and CI uploads the
  sidecar debug files to Sentry. Without them every frame is a bare address.
  Symbols are never attached to the GitHub release — full DWARF for this
  dependency tree is very large.
- **The consent file is not in the OS app-data directory.** It lives where
  `platform::config::data_dir()` resolves before Tauri starts (`$AJH_DATA_DIR`,
  else `$HOME/.ajh`), because `sentry::init` runs before any `AppHandle` exists
  and the minidump supervisor re-executes everything above its own call in the
  forked child, so the init cannot be moved later. It holds two booleans, no
  personal data, and the factory reset clears it — but deleting the app-data
  directory by hand will not.
- Turning reporting off unbinds the client immediately, but the minidump
  supervisor is a separate process forked at launch and only stops at restart.
  The settings copy says so rather than implying a clean instant stop.
- **5k errors/month is a quota on errors, not users.** One crash loop in one
  release can exhaust it in a day and blind us exactly when data matters most.
  Set a client `sample_rate` below 1.0 and per-issue server-side rate limiting
  before the user base grows.
- Any new field added to a transmitted payload must be assumed to carry PII
  until proven otherwise; extending the redactor remains a security change
  requiring a test (ADR 0027).
