# Premium Glass UX/UI Audit — AI Job Hunter

**Status:** Audit only — analysis + proposals. No code changed here. The owner greenlights items individually.
**Regenerated:** 2026-06-03 against `main` @ `2627f1c6` (after #231 + #232 merged) · **Branch:** `feat/ux-audit-premium-glass`
**Bar:** production-grade, premium, on par with a polished native macOS/iOS app. macOS/iOS glassmorphism is the signature.

> **This is a regeneration.** The first pass (2026-06-02) recommended building a real theme system and
> closing a set of consistency/a11y gaps. Since then **two PRs landed on `main`**: #231 (premium glass —
> token material + Light/Dark/System theme) and #232 (design-system lint reactivation + primitive/motion
> cleanup). The audit's **#1 deliverable shipped**, so this pass re-verifies every finding against current
> code, marks what's closed, and **replaces all screenshots** with fresh captures of the **new Light + Dark
> themes** (+ reduced-transparency / more-contrast modifiers).
>
> **Method.** Code review of `apps/tauri/src/renderer/**` + `packages/ui/**`, plus mock-client Playwright
> captures (`apps/tauri/e2e` harness, `ajh-theme` seeded per scheme, Chromium 1440×900 — the same engine as
> the Tauri WebView2 shell, so the CSS glass renders faithfully). Native macOS `NSVisualEffectView` vibrancy
> is a macOS-only layer the app still doesn't use; CSS `backdrop-filter` is a faithful proxy on Windows.

---

## 0. What shipped since the first audit ✅

The audit's centerpiece (the theme system) and most of the consistency lens are now done:

| First-audit finding                                                    | Status        | Shipped by                                                                                                                  |
| ---------------------------------------------------------------------- | ------------- | --------------------------------------------------------------------------------------------------------------------------- |
| **No real light theme** — only dark variants (§7, the #1 deliverable)  | ✅ **Closed** | #231 — `ThemeId = light \| dark \| system`, macOS-grade light + charcoal dark, View-Transition crossfade                    |
| **Glass material hardcoded dark / flat `.glass-card`** (§3.2)          | ✅ **Closed** | #231 — token-driven material in `tokens.css`/`utilities.css`, per-scheme surface flips                                      |
| **No `prefers-reduced-transparency` / `prefers-contrast` wiring** (§5) | ✅ **Closed** | #231 — `reduceTransparency`/`contrast` prefs + `data-reduce-transparency` (12 CSS hooks) + `data-contrast`                  |
| **~20 raw `<button>` / toggles bypassing `@ajh/ui`** (§4.2)            | ✅ **Closed** | #232 — **0** raw `<button>` left in features/routes/components; new `Button`/`Input` `unstyled` variant for custom surfaces |
| **Inline `transition={{…}}` motion drift** (§4.3)                      | ✅ **Closed** | #232 — **0** inline transition objects left; `withDelay()` + `spinSlow`/`breathe`/`ping` tokens added                       |
| **Type too small / no Text Size / no native fonts**                    | ✅ **Closed** | #231 — 16px base, 12px floor, Text Size S/M/L, SF-on-Mac font stack                                                         |
| **Design-system ESLint rules inert (dead globs)**                      | ✅ **Closed** | #232 — rules retargeted + enforced in CI; `<input>` honors documented native-type exceptions                                |

The two modes now read as **one macOS-grade system** (§1 gallery). Everything below is **what remains** —
the still-valid turnkey detail from the first audit, re-verified and re-prioritized.

---

## 1. Executive summary — remaining opportunities (priority order)

| #     | Opportunity                                                                                                                                                                                                                                                                                 | Why it's the highest leverage now                                                                                                                  | Effort  |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| **1** | **Close the last consistency residue + a real silent bug.** `bg-brand/08` (invalid Tailwind opacity → **no fill**, so selected states are invisible) survives in **2 autopilot wizard files**; no lint rule catches it.                                                                     | One concrete invisible-selection bug; cheap, and a lint guard prevents the whole class.                                                            | **S**   |
| **2** | **Finish the a11y foundation.** `:focus-visible { outline:none }` is still **global** + `.input-field:focus-visible{outline:none}` → inputs/links/custom rows can focus **invisibly** (Buttons now inject a ring, so they're OK). Add `ModalShell aria-labelledby`, switch ARIA on toggles. | WCAG 2.4.7; the reduced-transparency/contrast half already shipped, this is the other half.                                                        | **S–M** |
| **3** | **Make local-first feel instant.** Optimistic updates on delete/move/bookmark/toggle mutations via TanStack Query.                                                                                                                                                                          | The biggest perceived-speed win, still untouched.                                                                                                  | **M**   |
| **4** | **Re-aim Autopilot at "find & notify" and wire the native surface.** Remove the disabled auto-apply UI; send real OS notifications + deep-links; tray job-count/pause-all; fix missed-run catch-up.                                                                                         | The installed notification/tray plugins are still **unwired**, so the core "notify me" value isn't delivered; the wizard still implies auto-apply. | **L**   |
| **5** | **Group the 11-item flat sidebar + Documents hub.** Sectioned nav; merge Résumés + Generated + cover letters.                                                                                                                                                                               | Lowest priority, highest structural clarity; cover letters still have no first-class home.                                                         | **M**   |

**The delivered theme.** Light = cool present-gray canvas, off-white cards with gutters, two-tone sidebar,
deepened `#7C3AED` accent — the old "white-on-white flashbang" is gone. Dark = neutral charcoal (`~#1B1B22`),
mostly-opaque cards, brand violet only as accent. (Screenshots are captured locally during review and are
**not committed** — see §11.)

---

## 2. Per-route status (re-verified)

Severity: **H** high · **M** med · **L** low. "✅" = the first-audit finding for that route is now closed.

| Route            | Remaining issues (closed items struck)                                                                                                                                                                                                     | Sev | Owner                       |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --- | --------------------------- |
| **Dashboard**    | `JobPipelineOverview` empty state is a bare `<p>` (dead-end, no CTA); `AISystemStatus` has no `ErrorState` (failed health → "Checking…" forever). ✅ flat-glass tiles fixed by #231 material.                                              | M   | frontend                    |
| **Analyze**      | Error shown as plain inline `<div>`, no retry. ✅ raw segmented control → `Button unstyled` (#232); add `aria-pressed`.                                                                                                                    | M   | frontend                    |
| **AI Generate**  | Generation error has no "Try again"; some hardcoded English ("Prompt Quality", "Template", step labels). ✅ raw toggles/pickers converted (#232) — but ATS/Preview toggles still need `role="switch"`/`aria-pressed`.                      | M   | frontend                    |
| **Jobs**         | Empty state `GlassCard` with text, **no inline "Scrape jobs" CTA**; 500 un-virtualized rows (perf §10); icon copy button needs `aria-label`; ScrapeForm state lost on nav (`JobsPage:45`). ✅ raw copy/segmented buttons converted (#232). | M   | frontend · perf             |
| **Autopilot**    | Wizard still presents `auto_apply` + auto-submit (contradicts find-&-notify, §8); `· Applied {N}` permanently 0; no inline board-auth hint; loading is text not `CardSkeleton`; no optimistic delete.                                      | H   | scraping-applier · frontend |
| **Résumés**      | Misnamed (job log + "Generated" tab); Generated empty state has no "Generate" CTA + no skeleton; **delete fires with no `ConfirmModal`**; cards use raw `glass-graphite` utility not `<GlassCard>`.                                        | H/M | resume-export               |
| **Search**       | Zero-results `EmptyState` has no CTA to scrape/index first.                                                                                                                                                                                | M   | frontend                    |
| **AI Workspace** | Conceptually overlaps Analyze + Generate (IA §7). ✅ raw segmented control converted (#232).                                                                                                                                               | M   | frontend                    |
| **Monitoring**   | Two dead-end empty states (`ActiveJobsSection`, `ActivityFeedSection`) bare centered text, no `EmptyState`.                                                                                                                                | H   | frontend                    |
| **Settings**     | `SettingsSidebar` `div[role=button]` no `tabIndex` → **keyboard-unreachable**; toggles need `role="switch"`; per-tab rhythm drift (§9). ✅ Developer/provider raw buttons converted (#232); `bg-brand/08` still present (§3.1).            | M   | frontend                    |
| **Onboarding**   | Re-verify keyboard operability after a11y fixes. ✅ inline `transition` objects → tokens (#232).                                                                                                                                           | L   | frontend                    |

---

## 3. Lens — Consistency

### 3.1 `bg-brand/08` — the silent bug _(STILL OPEN — the one concrete defect)_

`08` is **not** a valid Tailwind opacity step (leading zero) → the utility is dropped at build and the
element gets **no fill**; selected states look identical to unselected. The first audit found 7; #232's
button conversions incidentally cleared most, but it survives in **2** files:

- `features/autopilot/components/wizard-steps/StepSchedule/index.tsx`
- `features/autopilot/components/wizard-steps/StepAction/index.tsx`

**Fix:** `bg-brand/08` → `bg-brand/10`. **Turnkey guard:** add an ESLint `no-restricted-syntax` rule for
`/\/0\d\b/` opacity modifiers in `className` so this can never silently ship again. **[H] frontend-reviewer.**

### 3.2 Primitive bypasses — ✅ CLOSED (#232)

0 raw `<button>` in features/routes/components. Custom surfaces (segmented controls, icon toggles, inline
links, clickable cards) route through `Button variant="unstyled"`. Raw `<input>` is limited to the
documented native types (`range\|file\|checkbox\|radio\|hidden`), enforced by the corrected `RAW_INPUT`
selector. **Residual:** the converted toggles still lack `role="switch"`/`aria-pressed` semantics (the
`unstyled` variant carries no ARIA) — fold into the §4 a11y sweep. Consider extracting a shared
`SegmentedControl` to `@ajh/ui` (the pattern still repeats ≥4×: Analyze, AI Workspace, Generate, OutputPanel).

### 3.3 Motion-token drift — ✅ CLOSED (#232)

0 inline `transition={{…}}` objects in feature files. Delayed/ambient animations use `transition.*` /
`withDelay()`. Reduced-motion is honored by the motion library defaults.

### 3.4 Component-vs-utility drift _(OPEN)_

`resumes/.../GenerationCard:113` and `InteractionRow:50` still instantiate `glass-graphite glass-highlight`
as raw classes on a `<div>` instead of `<GlassCard tone="graphite" highlight>` → tone changes won't
propagate. **[M] resume-export-expert.**

### 3.5 State coverage _(OPEN)_

`ConfirmModal` is used in only **2** feature files; `EmptyState`/`ErrorState` in **7**. Destructive actions
(delete generation/autopilot, disconnect account, remove key) should consistently confirm; every async
surface should pair a skeleton with an `EmptyState` (primary action) + `ErrorState` (retry). **[M] frontend.**

---

## 4. Lens — Accessibility & legibility

| Sev   | File:line                         | Finding → Fix                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ----- | --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **H** | `utilities.css:511`               | Global `:focus-visible{outline:none}` with no compensating ring → every non-`@ajh/ui` element (links, labels, custom rows) is keyboard-invisible. **Fix:** scope it (`.focus-ring-none:focus-visible{outline:none}` inside Button/Input) **or** set a global fallback `:focus-visible{outline:2px solid var(--color-brand);outline-offset:2px}`. ✅ `Button` (incl. `unstyled`) now injects `focus-visible:ring-2` — but that's not enough alone. |
| **H** | `Input.tsx` + `utilities.css:187` | `.input-field:focus-visible{outline:none}` + global reset → inputs have **zero** focus indicator. **Fix:** base classes `focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1`.                                                                                                                                                                                                                                           |
| **H** | `SettingsSidebar`                 | `div[role=button]` no `tabIndex` → keyboard-unreachable settings nav. **Fix:** real `<button>` / `<Button variant="ghost">`.                                                                                                                                                                                                                                                                                                                      |
| **M** | `ModalShell.tsx`                  | `role="dialog" aria-modal` but no `aria-labelledby`. **Fix:** add `aria-labelledby` wired to each modal's `<h2 id>`.                                                                                                                                                                                                                                                                                                                              |
| **M** | converted toggles                 | The #232 `unstyled` toggles (ATS mode, debug mode, Preview/Edit, target pickers) carry no `role="switch"`/`role="tab"` + `aria-checked`/`aria-pressed`. **Fix:** add ARIA in the a11y sweep.                                                                                                                                                                                                                                                      |
| **M** | i18n                              | Hardcoded English in `GenerationConfig` ("Prompt Quality","Template"), `OllamaConfig`, `OutputPanelGenerating` ("Loading model…","Step X of Y"). **Fix:** route through `t()`.                                                                                                                                                                                                                                                                    |
| ✅    | `theme.ts` + `utilities.css`      | `prefers-reduced-transparency` + `prefers-contrast` now wired (#231): `data-reduce-transparency` drives 12 solid-fallback overrides; `data-contrast="more"` set (verified in the reduced-glass / more-contrast capture passes).                                                                                                                                                                                                                   |

**Light-mode contrast — verified legible:** the light captures read cleanly; secondary text lands on the
macOS hierarchy (`#6E6E73`/`#8E8E93`), not the old invisible low-opacity grays.

---

## 5. Lens — Friction & accelerators _(OPEN)_

### 5.1 Optimistic updates — highest perceived-speed win, untouched

Add `onMutate` + rollback via TanStack Query to: `use-ai-generations.ts` delete generation; `use-autopilot.ts`
delete autopilot; `PostingRow` bookmark/applied (badge is optimistic but **counters** lag → `setQueryData` on
the interactions list); board-move / save / toggle. The data is local — the UI should never appear to "think."

### 5.2 Contextual-setup catalog — surface the prerequisite where the flow blocks

| Prerequisite missing      | Today                           | Recommend inline                                                                           |
| ------------------------- | ------------------------------- | ------------------------------------------------------------------------------------------ |
| No AI provider            | Generate/Analyze idle/fail      | Inline "Set up AI provider" card (reuse onboarding's `AISelectionStep` panel)              |
| Not connected to a board  | Autopilot/Jobs degrade silently | Reuse `AuthHint`/`AuthModeBadge` on the Autopilot wizard `StepTarget` + the Autopilot card |
| No browser (Chrome)       | "New Autopilot" → fails at run  | Guard the button: inline "Chrome required → Install" before wizard                         |
| Empty Jobs/Search/Résumés | Dead-end empty states           | `EmptyState action=` CTA on every one                                                      |

### 5.3 Keyboard shortcuts (NOT a command palette)

Small discoverable set + a "?" cheat-sheet: `⌘/Ctrl+K` focus search · `g`+`j/d/a/s` jump routes · `⌘/Ctrl+↵`
run primary action (Analyze/Generate) · `⌘/Ctrl+E` export · `Esc` closes modal/drawer (verify all honor it) ·
`⌘/Ctrl+,` Settings. Do **not** build a command palette (out of scope).

---

## 6. Lens — Navigation & IA _(OPEN — lowest priority, bold proposal)_

The sidebar is still **11 flat peers**; three (Analyze, Generate, AI Workspace) are the same AI-on-text job,
"Résumés" is really a job-log + Generated tab, and cover letters have no first-class home.

```
BEFORE (flat 11)                AFTER (grouped, ~3 sections)
 Dashboard                       Dashboard
 Resume Analyzer                 ── WORKSPACE ──
 AI Generate                      Jobs
 Jobs                             Documents ▸ Résumés / Cover Letters / Activity
 Autopilot                        Generate
 Resumes                          Analyze
 Search                           Chat (was "AI Workspace")
 AI Workspace                    ── AUTOMATION ──
 Monitoring                       Autopilot
 Help & Support                   Activity (was Monitoring)
 Settings                        ── (pinned bottom) ──
                                  Support · Settings
   Search → ⌘K in-page on Jobs (toolbar icon), not a sidebar slot.
```

**Documents hub** (rename "Résumés"): tabs Résumés / Cover Letters (both from `AiGenerationRecord`) / Activity
(the applied/viewed/bookmarked log) — gives cover letters a real home. Use `.section-label` for group headers

- hairline dividers. Recommendation only. **Owner: resume-export-expert · frontend-reviewer.**

---

## 7. Onboarding _(existing wizard; do NOT add a new one)_

Flow: Welcome → Resume → AI Selection → Research → Browser → Extension → Appearance + `SpotlightTour`. ✅ Browser prerequisite honored. ✅ Extension step added: pair the published Chrome extension, Firefox coming soon. ✅ Appearance step added: colour scheme + accent picker using the live theme engine. Research is conditional (Ollama only). ✅ inline `transition` objects → tokens (#232). **Open:** re-verify full keyboard operability after the §4 focus-ring fix (first thing a new user touches); ensure "skip" paths leave a usable contextual-setup trail (§5.2 — if AI setup is skipped, the inline Generate CTA must appear).

---

## 8. Autopilot — current vs. "find & notify" target _(OPEN)_

**Target:** find · score · dedupe · **notify** (native OS) — never auto-prepare/auto-submit; application
generation stays **on-demand** from the job view. Tray-resident; catches up missed runs; battery-aware.

### 8.1 Remove the auto-apply surface _(scraping-applier-expert)_

- **[H]** `StepAction/index.tsx` still presents `review`/`auto_apply` + a live auto-submit toggle + cover-letter
  textarea, wired end-to-end (IPC contract, Rust store). **Delete `StepAction`**, replace wizard step 3 with a
  summary/confirm; drop `action`/`autoSubmit`/`coverLetter` from `WizardState`, the IPC contract, and the Rust
  `Autopilot` store. The only persisted intent should be `schedule`. (Also clears the `bg-brand/08` here, §3.1.)
- **[H]** `AutopilotCard` shows a permanent `· Applied 0` counter + an `action` pill + dead `apply_start/apply_done`
  step icons → remove. On-demand `ApplyJobModal` already exists; relabel "Apply" → "Tailor"/"Prepare".

### 8.2 Wire native notifications + tray + deep-link _(tauri-security-reviewer · scraping-applier-expert)_

- **[H]** Plugins (`tauri-plugin-notification`, `single-instance`, tray-icon) installed but **no notification is
  ever sent**. After `record_run`, if new-job count > 0, `NotificationExt::notification(...).show()`, gated on
  permission (`Prompt`→request, `Denied`→skip). Thread the `is_new` count from `merge_found_jobs` through.
- **Deep-link:** tray/notification click → `autopilot.focus` event with `autopilot_id`; renderer navigates to that
  card's found-jobs panel. Register an `ajh://` scheme and **validate** argv in `single-instance` against a route
  allowlist before navigating (injection guard).
- **Tray menu:** "New jobs: N" (updated on run-complete) + "Pause All" + "Reopen window".

### 8.3 Missed-run catch-up + reliability _(scraping-applier-expert)_

- **[H]** `autopilot_scheduler.rs` — the first `interval.tick().await` swallows the immediate tick → catch-up is
  delayed 60s after launch (a daily job closed within 60s slips a full day). **Fix:** `tick()` once before the loop.
- **[M]** `stamp_last_run` before completion + no `run_status` → interrupted runs look "completed, 0 found." Add
  `run_status: in_progress|completed|failed` + an amber "interrupted" badge.
- **[M]** `min_match_score` configured but never gated before `record_run` → split passing/below-threshold.
- **[M]** Cancellation token registered but not threaded into `autopilot_scrape` → runs can't be cancelled.
- **[M]** Jaccard keyword score shown as a precise "%" → label "Keyword match" (or use embedding cosine).

### 8.4 Battery + launch-at-login _(tauri-security-reviewer)_

- **[M]** Scheduler ticks regardless of power source → add `sysinfo` battery check + `allow-on-battery` pref
  (default: pause heavy browser-automation on battery) + the tray "Pause All" gate.
- **[L]** `tauri-plugin-autostart` absent → add for optional launch-at-login, **default false**, scoped to `main`.

---

## 9. Settings — per-section consistency _(largely OPEN; foundation helped)_

The token unification (#231) removed the worst color drift and the new **Appearance** card
(`general-section/AppearanceCard.tsx`) is clean. #232 converted the raw
`<button>` toggles/pills (Developer, provider switchers). **What remains is structural rhythm + semantics + i18n.**

**The five systemic inconsistencies (re-verified):**

1. **Header primitive ignored.** `SettingsSection` (`GlassCard > IconBadge + SectionLabel`) is used by only ~2 of
   9 sections; the rest hand-roll the header 12+ times with drifting caps-label tracking and field-label opacity
   (`/40 /50 /55 /70` for the same role). **[H]**
2. **Two "you-are-here" languages.** Main sidebar active pill = violet gradient + glow; Settings sub-sidebar pill =
   flat `bg-white/[0.07]`, active icon `/70` not `text-brand-soft`. Every selector (Language, ActiveProvider,
   Performance, OutputTone) invents its own active state. **[H]**
3. **One concept, three row components.** `AccountRow` / `BoardSessionRow` / `LinkedInSessionRow` build "a connected
   account" three ways (different chrome, icon sizes, action styles). **[H]**
4. **Inconsistent destructive gating.** `BoardSessionRow` + Privacy gate behind `ConfirmModal`; `LinkedInSessionRow`
   disconnect and `AccountRow` credential-delete fire immediately. **[H]**
5. **i18n + a11y gaps.** Whole sections hardcoded English (PerformancePreferences, TechStack, OllamaResourcesPanel,
   Embeddings); `SettingsSidebar` keyboard-unreachable; toggles lack `role="switch"`; `bg-brand/08` in StepSchedule/
   StepAction (§3.1). ✅ the raw `<button>` elements themselves are converted (#232) — semantics still missing. **[H]**

**Convergence plan — make all 9 tabs read as one app:** (1) route every settings card through `<SettingsSection
icon label>` + every caps header through `<SectionLabel>` (add a lint/snapshot guard); (2) extract the app
sidebar's violet pill into a shared `NavPill`, apply to SettingsSidebar + every selector; (3) one destructive
contract — `ConfirmModal` on every disconnect/delete/reset; (4) collapse the three account rows into one prop-driven
row; (5) i18n + ARIA sweep + keyboard-reachable nav. **Owner: frontend-reviewer (+ resume-export for account rows).**

---

## 10. Lens — Performance _(OPEN; performance-profiler)_

| Sev | File:line                       | Fix                                                                                                                                                   |
| --- | ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| H   | `CinematicBackground/index.tsx` | RAF cursor-blob + parallax run forever, even when window hidden → pause on `visibilitychange`.                                                        |
| H   | `globals.css`                   | `body[data-modal-open] .app-content{filter:blur(6px)}` flattens the whole animated shell each frame → fixed overlay div above `.app-content` instead. |
| H   | `use-mouse-parallax.ts`         | `setPos` React state on every pointermove re-renders ~20 background nodes → ref + direct style mutation.                                              |
| M   | `JobsPage`                      | 500 un-virtualized `motion.div` rows each with `backdrop-filter` → `@tanstack/react-virtual`; drop per-row blur.                                      |
| M   | `utilities.css`                 | Several blurred aurora/nebula layers always composited → cut to 2 by default; `display:none` in reduced-glass/low-memory.                             |
| M   | `preferences-store.ts`          | `performanceMode:'balanced'` (default) does little → give it the blur-halving + 2-layer aurora; `low-memory` = no animation.                          |

(Re-verify line numbers against current `main` — these files moved in #231; the patterns persist.)

---

## 11. Screenshots — captured locally, not committed

Light + Dark + a11y-modifier captures of every route were taken **locally** during this review (mock-client
Playwright harness, `apps/tauri/e2e`, 1440×900) to ground the visual claims above. They are **not committed**
— screenshots are large binaries, so `docs/assets/ux-audit/` is gitignored; re-run the harness to regenerate
them on demand. The first audit's stale pre-#231 flow/settings captures were removed; those flow findings
(ScrapeForm fake-progress bar; ApplyDrawer auto-submit affordance; "Select an AI model first"
dead-prerequisite) remain valid and are folded into §2, §5, and §8.

---

## 12. Prioritized action plan _(remaining, each ≈ one PR)_

Ordered by the weighting (consistency → a11y → friction → Autopilot → IA/perf). Effort **S/M/L**.

| #   | PR                                                                                                                                                                                                      | Effort  | Owner                               |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- | ----------------------------------- |
| 1   | `bg-brand/08` fix (2 files) + invalid-opacity ESLint guard                                                                                                                                              | **S**   | frontend-reviewer                   |
| 2   | Focus foundation: scope global `:focus-visible{outline:none}` + add input ring + `ModalShell aria-labelledby` + toggle/segment ARIA                                                                     | **S–M** | frontend-reviewer · tauri-security  |
| 3   | Extract shared `SegmentedControl` to `@ajh/ui`; `<GlassCard>` for GenerationCard/InteractionRow                                                                                                         | **S**   | frontend-reviewer                   |
| 4   | State-coverage sweep: `EmptyState`+CTA (Jobs/Search/Résumés/Monitoring/Dashboard), `ErrorState`+retry (health/Analyze/Generate), skeletons (Autopilot/Generated), `ConfirmModal` on destructive actions | **M**   | frontend (+ resume-export)          |
| 5   | Optimistic updates for delete/bookmark/move/toggle                                                                                                                                                      | **M**   | frontend-reviewer                   |
| 6   | Contextual setup (generalize `AuthHint`) + persist ScrapeForm state + keyboard shortcuts/cheat-sheet                                                                                                    | **M**   | frontend (+ scraping-applier)       |
| 7   | Autopilot: remove auto-apply UI (delete `StepAction` end-to-end; remove Applied counter/action pill/dead icons)                                                                                         | **M**   | scraping-applier-expert             |
| 8   | Autopilot: native notifications + tray + validated `ajh://` deep-link                                                                                                                                   | **L**   | tauri-security (+ scraping-applier) |
| 9   | Autopilot: catch-up + reliability (immediate first tick, `run_status`, min-score gate, thread cancel token, battery pause, optional autostart)                                                          | **M**   | scraping-applier-expert             |
| 10  | Settings convergence: `SettingsSection` across 9 tabs; shared `NavPill`; one account-row; `ConfirmModal` everywhere; i18n + a11y sweep                                                                  | **L**   | frontend (+ resume-export)          |
| 11  | Documents IA: rename Résumés→Documents (Résumés/Cover-Letters/Activity tabs) + sidebar regroup + Search→⌘K                                                                                              | **M**   | resume-export · frontend            |
| 12  | Perf: pause RAF offscreen; modal overlay (not subtree filter); ref-based parallax; virtualize Jobs; aurora budget per perf-mode                                                                         | **M**   | frontend (perf-profiler review)     |
| 13  | _(macOS only, optional)_ native window vibrancy via `tauri-plugin-window-vibrancy`                                                                                                                      | **S**   | rust-backend-architect              |

**Suggested sequencing:** 1–3 (consistency/a11y residue) → 4–6 (state + friction) → 7–9 (Autopilot) → 10–11 (settings + IA) → 12–13.

---

## Appendix — method & attribution

Regenerated against `main` @ `2627f1c6` (post-#231 + #232). Findings are from direct code review of
`apps/tauri/src/renderer/**` and `packages/ui/**`; visual claims are grounded in the fresh Playwright captures
(§11). The first audit's raw findings came from five orchestrated reviewer agents (frontend-reviewer,
resume-export-expert, scraping-applier-expert, tauri-security-reviewer, performance-profiler); this regeneration
re-verifies them against current code inline. Security items surfaced in passing (`system_open_external` URL
allowlist, CSP loopback wildcard, the `ajh://` deep-link allowlist above) are tracked by `tauri-security-reviewer`
and are out of scope for this UX audit but noted so they aren't lost. No code is changed by this document.
