window.BENCHMARK_DATA = {
  "lastUpdate": 1781870149821,
  "repoUrl": "https://github.com/saeedkolivand/ai-job-hunter-app",
  "entries": {
    "Export render": [
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3daba33e9aa263a6c84bee93b2a934ebcdbc00fb",
          "message": "ci: workflow catalog, badges, and an all-actions version upgrade (#302)\n\n* ci: add an auto-generated workflow catalog with a drift guard\n\nSixteen workflows are hard to track and a hand-maintained summary rots.\nGenerate it instead: scripts/gen-workflow-catalog.mjs reads every\n.github/workflows/*.yml and writes two things:\n\n- .github/workflows/README.md — a table (name, triggers, role, purpose)\n  plus a live status-badge grid. Each description is pulled from the\n  workflow's own leading comment, so the source of truth stays in the file.\n- README.md — the same badge grid, spliced between marker comments.\n\nRoles are derived: only CI Pipeline is \"required\" (its `CI OK` umbrella is\nthe sole required check); the rest are advisory, security, or deploy.\n\nEnforcement: workflow-lint.yml gains a catalog job that runs\n`pnpm gen:workflows:check` (regenerate + git diff --exit-code) and fails on\ndrift. workflow-lint is not a required check, so a failure nudges without\nblocking merge. Run `pnpm gen:workflows` to refresh.\n\nPromote `yaml` to a root devDependency for the parser.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ci: bootstrap the benchmark data branch and bump codeql-action to v4\n\nTwo CI issues surfaced on the first runs after the benchmarks landed:\n\n- benchmark.yml failed on both push and PR with \"couldn't find remote ref\n  benchmarks\" (git exit 128): github-action-benchmark fetches the data branch\n  to read the baseline, but nothing had created it yet — a bootstrap deadlock\n  that would recur on every run. Add an idempotent step that creates an empty\n  orphan benchmarks branch in a throwaway clone (the main checkout is left\n  untouched) before the action runs, and mark the store step continue-on-error\n  so this advisory job can never show red on an infra hiccup.\n\n- Bump github/codeql-action/upload-sarif from v3 to v4 in semgrep.yml and\n  scorecard.yml (v3 is deprecated in December 2026). codeql.yml was already v4.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ci: upgrade all workflow actions to their latest versions\n\nAudited every action across .github/workflows and .github/actions against\nits latest upstream release and upgraded the stragglers (SHA-pinned to the\nrelease commit, comment set to the exact version):\n\n- step-security/harden-runner -> v2.19.4\n- swatinem/rust-cache          -> v2.9.1\n- awalsh128/cache-apt-pkgs-action -> v1.6.0\n- anthropics/claude-code-action -> latest v1 tag\n- astral-sh/setup-uv           -> v8.2.0 (major v6->v8; called with no inputs,\n  it only installs uv, so the bump is behaviour-neutral)\n- actions/setup-node           -> v6 (major v5->v6; every call pins\n  node-version 22, so v6's runner-runtime change does not affect us)\n- cycjimmy/semantic-release-action -> v6.0.0 (major v4->v6; v6 bundles\n  semantic-release 25, matching the repo's own .releaserc + devDeps, so it\n  aligns the action with the project rather than changing release behaviour)\n\nComment-accuracy only (already pinned to the latest SHA): reviewdog\naction-setup (v1.5.0), reviewdog action-actionlint (v1.72.0), pnpm/action-setup\n(v6.0.8), softprops/action-gh-release (v3.0.0).\n\nEverything else was already on its latest major (actions/checkout v6,\nupload-artifact v7, cache v5, dependency-review v5, github/codeql-action v4,\nthe pages actions, dorny/paths-filter, crate-ci/typos, lycheeverse/lychee,\ndavelosert/vitest-coverage-report-action, github-action-benchmark,\nossf/scorecard-action, taiki-e/install-action).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ci: escape backslashes in workflow-catalog table cells\n\nCodeQL flagged incomplete string escaping (High): cell() escaped pipes but\nnot the backslash itself, so a backslash in the input would not be escaped\nfirst. Escape backslashes before pipes.\n\nNo change to the current output (no cell contains a backslash today); this is\na defensive fix that clears the alert.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ci: add a live workflow status dashboard page\n\nA single self-contained landing/ci-dashboard.html whose JS queries the public\nGitHub Actions API in the browser and renders every workflow grouped by role\n(gating / advisory / security / deploy), each with its latest status, relative\ntime, duration, an 8-run history strip, and a link to its runs.\n\nIt is always live: it fetches on open, auto-refreshes every 5 minutes (with a\nvisible countdown), and has a \"Refresh now\" button. Anonymous API access is\nCORS-enabled for this public repo and two calls per refresh stay well under the\n60/hr-per-visitor limit; rate-limit and error states are handled.\n\nDeployment is free and additive: the file lives in landing/, so the existing\npages.yml publishes it at /ci-dashboard.html alongside the unchanged\nindex.html. It is marked noindex and is unlinked from the marketing page. The\nrole mapping mirrors scripts/gen-workflow-catalog.mjs. Linked from the README\nCI section.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ci: add a selectable auto-refresh interval to the dashboard\n\nA small segmented toggle (Off · 1m · 5m · 15m) beside the \"Refresh now\"\nbutton on the CI dashboard. The choice persists in localStorage; \"Off\"\nstops the timer (countdown shows \"auto-refresh off\") while manual refresh\nstill works. Default stays 5m. The control notes the ~60 req/hr anonymous\nGitHub API limit, and rate-limit errors remain handled gracefully.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-05T03:10:07+02:00",
          "tree_id": "23e3064b789517f16ca200114c39ccf5863d27cc",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/3daba33e9aa263a6c84bee93b2a934ebcdbc00fb"
        },
        "date": 1780622242123,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3003778,
            "range": "± 72582",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3485677,
            "range": "± 21432",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 281262,
            "range": "± 2327",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Saeed Kolivand",
            "username": "saeedkolivand",
            "email": "saeedkolivand1997@gmail.com"
          },
          "committer": {
            "name": "Saeed Kolivand",
            "username": "saeedkolivand",
            "email": "saeedkolivand1997@gmail.com"
          },
          "id": "ce1d609365231efc4ddcaa01e77f802f087dd199",
          "message": "@\nci: push the benchmark action own commit so dashboard updates\n\ngithub-action-benchmark v1.22.1 makes its OWN local commit even with\nauto-push:false — it does not leave the refreshed landing/benchmarks in\nthe working tree. The commit step guarded on `git status --porcelain`,\nwhich is always clean after the action commits, so it always hit the\n\"nothing to commit\" early-exit: the data sat in an unpushed local commit\nthat died with the runner and the Pages deploy step was skipped. Result:\nruns went green but the dashboard never moved (this run, #350, #351).\n\nGuard on `git log origin/main..HEAD` (the action unpushed commit) instead\nand push HEAD; amend that commit with [skip ci] so the data push does not\nre-trigger CI while pages.yml is still kicked explicitly.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n@",
          "timestamp": "2026-06-11T03:56:57Z",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ce1d609365231efc4ddcaa01e77f802f087dd199"
        },
        "date": 1781150842208,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3372476,
            "range": "± 185360",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3994950,
            "range": "± 244868",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 300221,
            "range": "± 16625",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "github-actions[bot]",
            "username": "github-actions[bot]",
            "email": "41898282+github-actions[bot]@users.noreply.github.com"
          },
          "committer": {
            "name": "github-actions[bot]",
            "username": "github-actions[bot]",
            "email": "41898282+github-actions[bot]@users.noreply.github.com"
          },
          "id": "58a278043bacd41fb5a3703eb6ea64d44474ba39",
          "message": "chore: sync version files to v0.91.2 [skip ci]",
          "timestamp": "2026-06-11T18:17:36Z",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/58a278043bacd41fb5a3703eb6ea64d44474ba39"
        },
        "date": 1781203297428,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3053732,
            "range": "± 30075",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3549934,
            "range": "± 22157",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 278094,
            "range": "± 1665",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "github-actions[bot]",
            "username": "github-actions[bot]",
            "email": "41898282+github-actions[bot]@users.noreply.github.com"
          },
          "committer": {
            "name": "github-actions[bot]",
            "username": "github-actions[bot]",
            "email": "41898282+github-actions[bot]@users.noreply.github.com"
          },
          "id": "b29df0646abed28d7c7fbec734eb7a960a199c15",
          "message": "chore: sync homebrew cask to v0.92.0 [skip ci]",
          "timestamp": "2026-06-11T23:52:45Z",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b29df0646abed28d7c7fbec734eb7a960a199c15"
        },
        "date": 1781223472001,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3063421,
            "range": "± 29305",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3526920,
            "range": "± 23835",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 276014,
            "range": "± 1764",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "508321e148c56e29c5f4cbe2205b73dbdd14f14c",
          "message": "feat: jobs collapse — unified record, appended migration, footer tracks every kind (#368)\n\n* refactor: unify the rust job-record shape with shared and add a jobs migration\n\nPhase 6 foundation (jobs collapse), behavior-preserving:\n- Expand JobStatus to the shared 7 variants (queued/running/streaming/completed/\n  failed/cancelled/retrying; `pending` kept as a legacy from_str alias) and\n  JobRecord to the full shared shape (payload, retries, maxRetries, updatedAt,\n  startedAt, finishedAt). jobs_list / jobs_get now return the canonical record.\n- Append a `jobs_add_lifecycle_fields` SQLite migration (never edits create_jobs);\n  the user_version runner applies it exactly once. open()/persist/load kept in\n  sync with the new columns; crash recovery flags every in-flight status.\n- Add the L3 emit wrapper in commands/jobs.rs (job_start/progress/complete/fail/\n  cancel) — the single mutator boundary that runs the L1 transition then emits the\n  matching typed jobs:event. The L1 JobTracker stays AppHandle-free (R2). Call\n  sites are repointed to it in the next commit.\n- Tests: lifecycle timestamps, a migration persist/reload round-trip, and\n  interrupted-job recovery.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: emit jobs events for every job transition so the footer tracks all kinds\n\nRepoint every tracker.lock().{start,update_progress,complete,fail,cancel} call\nsite across commands/{ai,pipeline,autopilot,scrape} and the AI providers\n(anthropic/openai/gemini/ollama/cli_agent) to the L3 wrapper added in the\nprevious commit. Each wrapper runs the L1 transition then emits the matching\njobs:event, so the footer activity monitor now reflects EVERY job of EVERY kind\n(ai.generate, ai.pull_model, ai.reembed, pipeline.generate, autopilot.run,\nscrape.board, scrape.url) — previously only a couple of paths emitted ad-hoc, so\nai.generate / autopilot.run were silent. job.started now fires on dispatch.\n\nRemove the now-redundant ad-hoc job.completed / job.failed emits in scrape.rs and\nai.rs that duplicated what the wrapper emits (data preserved byte-for-byte); the\nper-item job.stream data streams are kept. scrape progress now also rides\njobs:event via job_progress. Drop pipeline.rs's now-unused JobTracker/Mutex/\nManager imports.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-12T20:29:32+02:00",
          "tree_id": "c1372dacc57a4fbdfeede6419a7b5f3f4669d3d7",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/508321e148c56e29c5f4cbe2205b73dbdd14f14c"
        },
        "date": 1781289898447,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3181140,
            "range": "± 15070",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3732761,
            "range": "± 91316",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 286884,
            "range": "± 1838",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e927d6aa9b3138f5f6f4e36738858a32060f8034",
          "message": "feat: app-wide single-source accent color with a user picker (#372)\n\n* feat: single-source accent color with auto-contrast and accent buttons\n\n- collapse the accent to one source (--color-brand); brand-dim + the brand\n  glows now derive from it via color-mix, so any accent re-tints them\n- add a runtime accent applier to the theme engine: ThemePrefs.accentSource\n  (default/system/custom) + accentColor; applyAccent sets --color-brand, a\n  derived brand-soft, and an auto-contrast --color-action-foreground, clears\n  the override for 'default', and applies pre-paint via restoreTheme\n- add color.ts (parseHex/luminance/lighten/readableForeground) + tests\n- replace stray violet literals (to-primary gradients, violet-700) with\n  brand-derived tokens so progress bars re-tint with the accent\n- give each view's primary call-to-action variant=\"primary\" (token-driven\n  accent) so the primary action stands out; secondary actions stay neutral\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add an accent color picker to appearance settings\n\n- add a Default chip + 8 macOS-style preset swatches to AppearanceCard, wired\n  through the theme engine (accentSource/accentColor) so picking one re-tints\n  the whole app from the single --color-brand with live preview and persistence\n- add the accent i18n keys (en); de falls back to en\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: read the system accent color for the appearance picker\n\n- add a system_accent_color command: Windows via the windows crate UISettings\n  (UI_ViewManagement feature, the accent that honours \"Automatic accent color\"),\n  macOS via `defaults read -g AppleAccentColor` mapped to the fixed macOS accent\n  palette, other platforms report unsupported\n- wire the IPC end to end (contract + channel + tauri-client + mock client +\n  a useSystemAccent service hook + query key)\n- show a \"System\" accent chip in Appearance settings only when supported; on\n  Linux or a read failure it is hidden, with no error UI\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: give appearance its own settings section\n\n- promote Appearance out of General into a dedicated top-level Settings section\n  (Palette icon) and move the AppearanceCard (theme, accent, text size,\n  transparency, contrast) into it\n- trim the General description and add the appearance section labels (en)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: document the accent color system and decision record\n\n- DESIGN_SYSTEM.md: the accent model (single-source --color-brand, derived\n  shades, default/system/custom sources, auto-contrast, one-primary-CTA rule)\n- ADR 0004: single-source user-customizable accent color\n- knowledge/ui-theming-accent.md: thin pointer to the theme/accent applier\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: add appearance to the session-store settings section union\n\nThe session store keeps its own SettingsSection union for the persisted\nactiveSection; sync it with the new 'appearance' SectionId so SettingsPage\ntypechecks.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-13T02:21:45+02:00",
          "tree_id": "6aff5147d7a0cc515d36cc756b1e23180febb857",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/e927d6aa9b3138f5f6f4e36738858a32060f8034"
        },
        "date": 1781311020834,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3061803,
            "range": "± 84126",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3616282,
            "range": "± 57712",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 283467,
            "range": "± 4647",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "25265223e22499da8b63a189fe487f2c5661222f",
          "message": "feat: extension store readiness, pairing deep link, and rename to ai-job-hunter (#374)\n\n* fix: make browser extension store-compliant and drop zod from bundle\n\nRemove the zod runtime from the @ajh/extension bundle: zod v4's JIT\nFunction(\"\") feature-probe tripped AMO's DANGEROUS_EVAL. Split the pure\nconstants/types into a zod-free @ajh/shared/extension-protocol subpath and\nreplace the schema safeParse in bridge.ts with a hand-written guard. The\ndesktop/renderer/rust-gen consumers keep the barrel exports unchanged.\n\nFix the Firefox manifest store-lint errors: set the real AMO id\n(job-importer@aijobhunter.app) in the manifest and the auth.rs allowlist,\nadd gecko.data_collection_permissions (none), and raise strict_min_version\nto 140 with a gecko_android floor of 142 so the data-consent key is honored.\n\nweb-ext lint dist/firefox is now 0 errors / 0 warnings; the Function( count\nin the firefox bundle is 0.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: add privacy policy page for extension store listings\n\nAdd landing/privacy.html as the public privacy-policy URL for the Chrome Web\nStore and Firefox AMO listings. Matches the landing design system; covers the\nloopback-only browser extension and the desktop app's real outbound data\n(user-configured AI providers under the user's own key, job-board scraping,\nupdate checks), with no telemetry. Contact: contact@aijobhunter.app.\n\nLink it from the landing footer.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: redesign extension popup to paper-blend brand\n\nRestyle the popup to the landing's paper/ink/red brand: cream hand-drawn\ncard, Patrick Hand title with a red scrawl underline, per-phase status\npills, red primary buttons, and a dashed token field. All controller\nhooks (ids, classes, ARIA) and AA contrast are preserved.\n\nPatrick Hand is bundled locally as a woff2 (OFL), so no remote font is\nloaded at runtime — the store \"no remotely-hosted resources\" posture\nstays intact.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: add store screenshots, promo art, and asset generator\n\nAdd a deterministic headless-Chromium pipeline\n(apps/extension/scripts/gen-store-assets.mjs) that captures the real\npopup in 3 states and composites doodle-annotated 1280x800 store\nscreenshots plus a 440x280 promo tile and a 1400x560 marquee, with an\ninventory README. No AI image generation — text is crisp and every\noutput dimension is exact.\n\nAdd a scoped ESLint override for the generator (Node + browser globals,\nsince its page.evaluate callbacks run in the page context).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add ajh://settings/extension deep-link target\n\nAdd an allowlisted ajh://settings/extension deep link that focuses the\napp and routes the renderer to Settings then Accounts then Browser\nextension, carrying a token-focus signal, so the browser extension can\nsend users straight to their pairing token. Reuses the cold-start-robust\nmenu route-intent (PendingMenu buffer pulled by the renderer).\n\nAlso handle first-instance cold-launch URLs in setup() by parsing argv\nand the deep-link plugin's current URL — previously only a second launch\nor a macOS reopen was handled, which also silently dropped autopilot\ncold-launches.\n\nExtend MenuNavigateEvent with an optional focus field. The renderer\nhighlight and the extension popup button are follow-ups.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: guide unpaired users from the extension popup to the token\n\nComplete the extension-pairing deep link end to end. The popup now shows\nan \"Open AI Job Hunter\" button (app-not-running) and a \"Find my token\"\nbutton (not paired) that open ajh://settings/extension. On arrival the\nrenderer scrolls the pairing token into view and gives it a one-shot\nring highlight, so a user goes from \"not connected\" to the token in one\nclick.\n\nThe focus signal rides the optional MenuNavigateEvent.focus field and a\none-shot ui-store flag consumed by ExtensionBridgeSection (mirrors the\nautopilot focus pattern). Navigation only: it never copies or exposes\nthe token.\n\nHarden the cold-start deep-link parse to use args_os() so a non-Unicode\nlaunch argument can't panic the app at boot, and refresh the deeplink\nmodule doc now that the scheme is registered.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: cover the extension-pairing focus flag flow\n\nAdd unit tests for the new extensionTokenFocus ui-store flag (default,\nset, clear) and for use-menu-navigation honoring MenuNavigateEvent.focus:\nfocus 'extension-token' sets the flag and still navigates + selects the\naccounts section, while the native-menu path (no focus) leaves the flag\nuntouched and still navigates.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: rename repo and product to ai-job-hunter\n\nRename the GitHub repo ai-job-hunter-assistant-app to ai-job-hunter-app\nand drop \"Assistant\" from the product name (productName, window title,\nand the AI-Job-Hunter-* release/updater artifact stems). Update every\ngithub.com/saeedkolivand/... reference, the updater endpoint, the\nrelease workflow, the Homebrew cask, badges, landing, and docs.\n\nThe bundle identifier (com.ajh.desktop) and the updater pubkey are left\nunchanged, so existing installs keep their data and continue to accept\nupdates; GitHub redirects the old slug for already-shipped binaries. The\nnew artifact names take effect on the next release.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: point extension-protocol parity test at the constants module\n\nThe zod-drop moved EXTENSION_MESSAGE_TYPES (and its literal wire-type\nstrings) into the zod-free extension-protocol-constants.ts; the old\nextension-protocol.ts only re-exports it. Repoint the Rust parity test's\ntext scan at the constants module so it finds the literals again.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* style: satisfy clippy doc lint and tidy popup button markup\n\nAdd a blank doc line in deeplink/mod.rs so the paragraph after the\nallowlist list is not parsed as a lazy list continuation\n(clippy::doc_lazy_continuation, -D warnings in the pre-push hook). Also\nwrap the popup \"Open AI Job Hunter\" button onto its own lines.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* style: rustfmt the deep-link cold-start handler\n\nApply cargo fmt to the cold-start argv-parsing block in lib.rs so\n`cargo fmt --check` (pre-push hook) passes. Formatting only.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: make the extension popup follow the system theme\n\nAdd a warm-dark variant of the paper-blend popup under\nprefers-color-scheme: dark. The light theme is unchanged (default); the\ndark overrides are driven entirely by CSS variables, keeping the\nhand-drawn character — Patrick Hand title + red scrawl underline,\nsemantic pills, red primary buttons, dashed token field — on warm-dark\nsurfaces with AA-contrast cream text. All controller hooks preserved.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-13T11:00:44+02:00",
          "tree_id": "84644cdaeccfb4145a3e3278a5141e25c600ac5d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/25265223e22499da8b63a189fe487f2c5661222f"
        },
        "date": 1781342342142,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3213285,
            "range": "± 21021",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3783896,
            "range": "± 115350",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 288574,
            "range": "± 1761",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bab8527ab587fb607ea03b2aaefb26af4d2eeae7",
          "message": "fix: accept firefox moz-extension uuid origins in extension bridge (#375)\n\nFirefox assigns every extension install a random per-profile internal\nUUID and uses it in moz-extension:// origins (anti-fingerprinting), never\nthe AMO gecko id. The bridge origin allowlist pinned Firefox to the gecko\nid, so the published add-on's handshake origin could never match and the\nloopback WS bridge returned 403 — the Firefox extension could never pair.\n\nSplit the origin check: Chrome stays pinned to the store id in\nALLOWED_EXTENSION_IDS; Firefox is accepted by UUID shape (8-4-4-4-12\nlowercase hex, scheme+host only) via a zero-dep hand-written validator.\nThe 256-bit per-frame pairing token over a loopback-only listener remains\nthe real auth boundary; the origin check is defense-in-depth. A web page\nstill gets 403 (non-extension scheme). Drops the dead gecko-id allowlist\nentry and documents why Firefox is not pinned by id.\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-13T12:07:16+02:00",
          "tree_id": "ea8e1748c42ed4f26d299f8ab3abbbb2bcb86703",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/bab8527ab587fb607ea03b2aaefb26af4d2eeae7"
        },
        "date": 1781345742954,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3081489,
            "range": "± 74291",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3540778,
            "range": "± 25061",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 280308,
            "range": "± 2387",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b0a3e7e1ecb01b793f6b983f21e47af47edb05a4",
          "message": "fix: backend error-path hardening + dead-code cleanup and refactors (#377)\n\n* docs: update extension-bridge origin validation for firefox uuid shape\n\nFirefox assigns per-install random UUIDs for moz-extension:// origins, never\nthe AMO gecko id, so the bridge now validates Firefox origins by UUID shape\n(8-4-4-4-12 hex pattern) instead of a fixed gecko id that never appears.\nChrome stays pinned to the stable CWS id. Updated README and ADR-015 to\nclarify the defense-in-depth origin check vs. the per-frame token boundary,\nand documented the Chrome CWS id placeholder as a release-blocking checklist.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: remove safe-tier dead code (unused deps + dead exports)\n\nAudited with knip + codegraph (Rust clean per cargo machete/clippy).\nRemoves only provably-unreferenced, zero-behavior items:\n\n- deps: clsx, jspdf, tailwind-merge (apps/tauri) — @ajh/ui cn() owns\n  clsx/tailwind-merge; PDF export is Rust/Typst. zod (apps/extension) —\n  the bundle is deliberately zod-free (hand-written guards).\n- dead hooks/consts: useLanguage, useResume (preferences-store),\n  useSetPerformanceMode (use-system), useDataCapability\n  (CapabilityProvider), TAILOR_TEMPLATE/TAILOR_ATS_MODE\n  (useTailorGeneration), ANALYSIS_STRATEGY (prompts truncation, an alias\n  of the live LARGE_MODEL_STRATEGY).\n\ntsc --noEmit clean, lint:strict green, knip findings cleared with no new\norphans, full test suite (1619) passing.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: ignore root-level extension zips and drop stale test mock keys\n\n- .gitignore: ignore packaged ai-job-hunter-*.zip artifacts dropped at the\n  repo root by the package/release step (the store-packages/ dir was already\n  ignored; the release step also emits a root-level zip).\n- ApplyPage.test.tsx: remove the vi.mock keys TAILOR_TEMPLATE/TAILOR_ATS_MODE\n  left inert after those constants were deleted in this branch.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: harden backend error paths and hoist per-call regex compilation\n\nAddresses the High + cheap-Medium findings from the whole-backend Rust\ncode-quality audit. Security-reviewed (credential/auth path: fail-closed\non a degraded keyring is structurally guaranteed; no secret leakage in\nthe new error/log strings; LinkedIn wire output byte-identical).\n\nCorrectness (panics / silent failures -> propagated AppError):\n- credentials::init_keyring no longer .expect()-panics at startup; returns\n  AppResult and lib.rs logs a warning + degrades (credential ops then fail\n  closed via AppError instead of crashing the app on an unavailable keyring).\n- credentials::save_meta no longer swallows write errors (.ok()); returns\n  AppResult through set()/remove(); create_dir_all failure is logged.\n- linkedin client get_default_headers: static headers use\n  HeaderValue::from_static; the runtime CSRF token / cookie now propagate a\n  parse error instead of .parse().unwrap() panicking on a malformed token.\n\nPerf / DRY (no behavior change):\n- hoisted per-call regex compilation to module-level LazyLock statics in\n  scraping/http (14), scrape_url, germantechjobs; CSS Selector statics in\n  glassdoor/xing/indeed/linkedin api_client.\n- unified the duplicated Personio XML extraction into\n  personio::parse_xml_feed (shared regex set + per-position DTO; each caller\n  keeps its distinct JobPosting assembly).\n- de-duplicated the Chrome user-agent literal onto net::http::DEFAULT_UA.\n\ncargo check/clippy --lib clean; per-module tests + architecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: clarify keyring degradation and fail-closed guarantee in security rules\n\nUpdated security-rules.md to document that keyring init degrades gracefully\n(warns instead of panic) when system keyring service is unavailable, with\nsubsequent credential reads/writes erroring rather than falling back to\nplaintext—fail-closed behavior guaranteed by keyring-core v1 semantics.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: migrate scraping/export/validate/browser off anyhow to apperror\n\nReplaces anyhow::Result / Box<dyn Error> with the centralized AppResult /\nAppError across the scraping HTTP client, LinkedIn client, validate, and\nexport (model_docx, pdf) layers, plus browser/mod.rs (M3/M4/M5 from the\nbackend code-quality audit). Error categories are now preserved to the\ncommand boundary instead of being flattened into an opaque string.\n\nVariant mapping: transport/status -> Network; body/document/JSON decode and\nTypst render -> Parse; runtime header-build config -> Config; size limits ->\nValidation; cancellation -> Cancelled. Context messages are folded into the\nerror so no diagnostic is lost.\n\nSecurity: every changed error message is structural (library error string or\na fixed phrase) - no resume/cover-letter text, secrets, or file contents are\ninterpolated into any error or log line.\n\nanyhow is retained (29 other modules still use it; error.rs keeps the\nFrom<anyhow::Error> bridge). LinkedInHttpClient::new() still .expect()-panics\non client build (no anyhow there to migrate) - left for a follow-up.\n\ncargo check/clippy --lib clean; scraping/validate/export/browser module tests\nplus architecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: centralize epoch-ms timestamp casts in db helpers\n\nReplaces ~40 scattered, unannotated u64<->i64 millisecond-timestamp casts\nat the SQLite row boundary (applications, jobs, ai_generations, referrals,\ndocuments, autopilot_scheduler) with a documented helper pair ts_to_db /\nts_from_db in the L0 db module (M10 from the backend audit).\n\nThe helpers use lossless saturating try_into and are byte-identical to the\nold `as` casts over the real domain (epoch-ms stays below i64::MAX until\n~year 292 million; stored values are always non-negative), so existing rows\nround-trip unchanged. Non-timestamp casts (counts, bool flags, vector dims,\ntimezone offsets, the u128->u64 clock source) were intentionally left alone.\n\ncargo check/clippy --lib clean; per-module tests + 3 new db::ts_tests +\narchitecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: drop false-positive dead_code allows and document the rest\n\nM1/M2 from the backend audit. Most of the broad #![allow(dead_code)]\nsuppressions were hiding nothing:\n\n- scraping/http and the LinkedIn client/api_client/rate_limiter modules\n  have live callers (board scrapers + the linkedin registry scraper), so\n  the blanket allows were pure false positives -> removed.\n- ipc_contracts/*: replaced the scattered per-struct #[allow(dead_code)]\n  with one documented file-level allow (serde DTOs mirroring the TS\n  contract; fields are (de)serialized across IPC, never read in Rust).\n- browser/mod.rs (BrowserController, chromiumoxide CDP automation) is\n  genuinely dormant (zero live callers, never wired into SCRAPERS). Kept\n  with a documented allow + rationale; flagged as a deletion candidate\n  pending a product decision (delete vs reserve for JS-rendered scrapers).\n\ncargo check/clippy --lib clean; module tests + architecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* style: rustfmt migration drift and assert http test reads response\n\nFull-target clippy (cargo clippy --all-targets --all-features -- -D warnings,\nthe pre-push gate command) caught two things the agents' --lib checks missed\nbecause --lib does not compile test code:\n\n- scraping/http/test.rs: TestResponse.message was deserialized but never read\n  (dead_code). Strengthened the assertion to check the parsed value, which\n  reads the field and verifies deserialization rather than just is_ok().\n- rustfmt drift introduced by the anyhow->AppError migration in credentials,\n  indeed, linkedin api_client/client (+ its test) - ran cargo fmt --all.\n\ncargo clippy --all-targets --all-features -- -D warnings clean; cargo fmt\n--check clean; architecture guard 11/11.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: reflect anyhow to apperror migrations and browser subsystem status\n\nUpdate architecture docs to note error-layer conversions in scraping/http,\nscraping/linkedin/client, validate, export/{model_docx,pdf}, browser; keyring\ndegradation now explicit; browser/BrowserController is dormant/reserved. Refresh\nlast-updated dates. Persist 4 architecture lessons: anyhow-boundary bridging,\nkeyring fault-tolerance, ts_to_db/ts_from_db centralization, Personio DRY.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: remove dead browser-automation module\n\nDeletes apps/tauri/src-tauri/src/browser/ (BrowserController, a chromiumoxide\nCDP wrapper). It was dormant: zero live callers, never wired into the SCRAPERS\nregistry - a never-enabled attempt at browser-based scraping. Today scraping is\nHTTP (reqwest via scraping/http + the LinkedIn http/api clients); Chromium is\nused only for the manual-login + cookie-capture flow, which lives in\nscraping/board_login (boards_connect / importCookies).\n\nchromiumoxide is retained - board_login, glassdoor, and platform/chrome still\nuse it (cargo machete confirms it is not unused). Also drops the now-stale\n\"browser\" entry from the tests/architecture.rs L0 classification so the\nno-dead-allowlist-entry guards pass.\n\ncargo check/clippy --all-targets --all-features -- -D warnings clean; cargo fmt\n--check clean; architecture guard 11/11; cargo machete clean.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: regenerate ipc_contracts to fix gen:ipc drift\n\nStage 3 of this branch hand-edited the @generated ipc_contracts/*.rs files\n(consolidating per-struct #[allow(dead_code)] into a file-level one). Those\nfiles are produced by `pnpm gen:ipc`, so the committed form no longer matched\nthe generator and CI's gen:ipc:check failed on 10 files. The pre-push hook\ndoes not run that check, so it slipped through.\n\nRegenerated to the canonical form (per-struct #[allow(dead_code)], which still\nsuppresses the serde-DTO dead_code under clippy --all-targets -- -D warnings).\nThis reverts only the cosmetic ipc_contracts allow-consolidation; the rest of\nthe dead_code cleanup (removing genuinely-false-positive allows from\nscraping/http and the linkedin modules) stands. Generated files must not be\nhand-edited - to change their shape, change packages/shared/scripts/\ngen-ipc-rust.ts instead.\n\ngen:ipc:check up to date; clippy --all-targets --all-features -- -D warnings\nclean; cargo fmt --check clean.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* build: add gen:ipc:check to the pre-push hook\n\nThe pre-push gate ran eslint + prettier but not gen:ipc:check, so a hand-edit\nto a @generated ipc_contracts/*.rs file passed the local push and only failed\nin CI's Lint & Format job. Run the same IPC codegen drift check locally so this\nclass is caught before push.\n\n(check:landing-drift is the other Lint & Format check still missing from the\nhook - left for a follow-up.)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* build: path-filter the pre-push gate to changed files\n\nMirrors CI's dorny/paths-filter locally: the hook reads the pushed refs from\nstdin, diffs the push range, and runs only the gate groups whose paths changed\n(frontend vs rust, matching the filters in ci-pipeline.yml). A docs-only or\nJS-only push now skips the cargo steps entirely - so it can't hit the\najh-tauri.exe build lock - and a Rust-only push skips the JS gate.\n\nSafety: any uncertainty (no push range / detached) falls back to the FULL gate,\ncross-cutting paths (packages/**, .github/**) trip both groups, gen:ipc:check\nruns when node OR rust changed, and PREPUSH_FULL=1 forces everything. CI stays\nthe authoritative gate. Decision logic tested across docs/rust/node/mixed/\ncross-cutting scenarios.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ci: surface test failures with dorny/test-reporter\n\nAdds JUnit output and two dorny/test-reporter steps so failing tests appear\nas a GitHub check + inline PR annotations instead of buried in CI logs:\n\n- nextest: a [profile.ci.junit] report (cargo-llvm-cov nextest already runs\n  the ci profile) -> target/llvm-cov-target/nextest/ci/junit.xml.\n- vitest: --reporter=junit alongside the default reporter (the default text\n  still feeds the coverage-summary grep) -> reports/vitest.junit.xml.\n- two report steps (if: always, fail-on-error: false) pinned to\n  dorny/test-reporter@a43b3a5f # v3.0.0; the Rust path is globbed to handle\n  cargo-llvm-cov's target-dir redirection.\n\nchecks: write is scoped to the tests job; CI runs on pull_request (not\npull_request_target), so fork tokens stay read-only - security-reviewed safe.\nThe \"Check for Test Failures\" step remains the sole pass/fail gate; the\nreporters are purely additive.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* build: make pre-push gate non-mutating and lockfile-strict\n\nAddresses CodeRabbit review on PR #377:\n- format step ran `pnpm format` (prettier --write) which silently rewrites the\n  working tree instead of failing -> switched to `pnpm format:check`\n  (prettier --check), so committed format drift actually blocks the push,\n  matching CI's format-guard.\n- cargo check/test/clippy omitted --locked while CI runs with it -> added\n  --locked so a lockfile drift fails locally instead of only in CI.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: remove drift-prone module census and deleted browser references\n\nReplace module list + count in architecture.md with thin pointer to\nerror.rs + enforcing test r6_no_stringly_result. Remove reference to\ndormant/deleted browser module in automation-domain.md; clarify Chromium\nis used only for board_login cookie capture, not scraping transport.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* build: add landing-drift to the pre-push gate with a landing filter group\n\nCloses the last CI-parity gap CodeRabbit flagged: CI's lint-format job runs\ncheck:landing-drift but the hook did not. Adds a LANDING path-filter group\n(landing/**) so a landing-only push runs just the drift check, and runs\ncheck:landing-drift whenever node/rust/landing paths change.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: return a result from the linkedin http client constructor\n\nnew() built its reqwest client with .expect(\"failed to build LinkedIn HTTP\nclient\"), so a TLS/config init failure aborted the process. It now returns\nAppResult<Self> (AppError::Network); the live caller (boards/linkedin search)\npropagates via ?, and the 5 test callers .expect() it. Completes the\nanyhow->AppError fallibility pass for the LinkedIn client.\n\ncargo check/clippy --all-targets --all-features --locked clean; linkedin\nmodule tests + architecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-13T23:39:49+02:00",
          "tree_id": "2024fe52b601f019f73ce8b23b3f31d46ac86a3f",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b0a3e7e1ecb01b793f6b983f21e47af47edb05a4"
        },
        "date": 1781387896309,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3183921,
            "range": "± 20175",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3736424,
            "range": "± 138454",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287496,
            "range": "± 1757",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "81f6c70cc883f1073de8216271ed2ba653321cc9",
          "message": "feat: persist self-invalidating job match-score and vector caches (#378)\n\n* feat: persist self-invalidating job match-score and vector caches\n\nmatch_resume re-embedded the job text on every call (only the résumé\nvector was cached), so the Job board scored rows one-by-one with a live\nembedding call each time. Add two persisted, self-invalidating caches in\nDocumentStore:\n\n- posting_vectors: job-vector cache guarded by embedding space + a stable\n  sha2 text_hash (translation-aware), read via a centralised\n  posting_vector_or_embed resolver. PostingsCache stays owned by search.\n- match_scores: full result cache keyed on\n  (resume_id, job_id, provider, model, semantic_enabled, formula_version,\n  job_text_hash). Insert-only documents make a résumé edit a new resumeId\n  and thus a natural miss; MATCH_FORMULA_VERSION busts on formula changes.\n\nSelf-invalidation is key-based, so no event sweeps. clear_all() now wipes\nboth new tables (factory-reset/GDPR), and ai_set_embedding_config evicts\nboth on a real embedding-space change. Adds sha2 as a direct dep\n(resolves to the already-present 0.10.9).\n\nPure helpers (posting_vector_is_fresh, embedding_space_changed,\nsemantic_enabled_bit) are extracted as test seams. Documented in ADR-017.\n\nDeferred to follow-ups: pre-embed-on-scrape, provider-aware scheduler\nconcurrency, batch-embed provider trait.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: refine notification bell color and make job save button primary\n\n- NotificationBell: drop the boxed ghost border, brighten the icon, and\n  tint it brand when there are unread (pairs with the badge); sits cleanly\n  next to the borderless window controls.\n- PostingRow: Save switches glass -> primary so it reads as the primary\n  row action; Tailor stays glass.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* perf: batch keyword match-scoring to remove the per-row scoring crawl\n\nDefault scoring is keyword-only (semanticScoring defaults false → no\nembedding), but the per-row ScoringScheduler throttled it to\nCONCURRENCY=1, serialising N IPC calls + React-Query round-trips. That\nthrottle existed only to protect the embedder the default never uses, so\nit produced the visible … crawl for zero benefit.\n\nReplace it with a single backend pass:\n\n- Extract the per-job kernel into score_one; match_resume becomes a\n  one-job caller (behaviour-preserving, parity-verified).\n- New command match_resume_batch(resumeId, jobIds[], semanticEnabled)\n  scores all postings in one Rust pass — résumé + keywords resolved once,\n  loops score_one sequentially, shares the ADR-017 match_scores cache\n  (keyword keys use semantic_enabled=0). MATCH_BATCH_MAX=1000 caps the\n  batch as a server-side DoS guard (Zod .max is type-only here).\n- Frontend: useJobMatchScores batch hook (keepPreviousData, order-stable\n  key) + MatchScoresProvider; RowMatchScore is now presentational with\n  per-row pending. The ScoringScheduler provider is deleted (dead).\n\nEmbedding speed (Ollama /api/embed batch, payload trim, warm-on-scrape)\nis deferred to a later opt-in-only change.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-14T02:41:02+02:00",
          "tree_id": "144d26d79caddd947af2c97adc9d61055b7b8eed",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/81f6c70cc883f1073de8216271ed2ba653321cc9"
        },
        "date": 1781398095629,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2500687,
            "range": "± 82522",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2949199,
            "range": "± 78140",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 223749,
            "range": "± 1389",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3235b059238f91d6e087c90048c7040b4240bbd2",
          "message": "feat: two-tone accent gradients, aurora revival, and chrome-extension publish (#379)\n\n* feat: derive two-tone accent gradients and revive accent-tinted aurora\n\nEvery accent (preset, system, custom hex) now yields a coherent two-tone\ngradient instead of the hard-coded violet->indigo. Adds --color-brand-2\n(+soft) as the gradient end: hand-tuned per preset, hue-rotation fallback\nfor system/custom, static for default (look unchanged). text-gradient,\ngradient-border, glass tones, and the revived slim aurora background all\nride the brand tokens. Appearance swatches preview the gradient.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: pin published chrome web store id for the extension bridge\n\nReplaces the publish-time placeholder in the extension-bridge origin\nallowlist with the real Chrome Web Store id, so the published extension's\nchrome-extension origin passes is_allowed_origin and the desktop handshake\naccepts it. Firefox/moz-extension and dev-override paths unchanged.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: announce published chrome extension in readmes and landing footer\n\nChrome extension is live on the Web Store. Add a CWS badge + install link\nto the root README and extension README, flip the extension's unpublished\nbadge/notes to Chrome-published (Firefox/AMO pending), correct the stale\nauth.rs placeholder note now that the real id is pinned, and add a Chrome\nextension link beside Privacy in the landing footer.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: sync design-system and add adr-018 for accent gradients and aurora\n\nCorrect the stale claims that brand gradients are color-mix-derived and the\naurora was removed: gradients now derive from the --color-brand/--color-brand-2\ntoken pair, and a slim accent-tinted aurora was revived. Add ADR-018 recording\nthe aurora revival (reverses the removal in 8688eb91).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: harden appearance swatch gradient assertions against jsdom\n\nRead the raw style attribute instead of the CSSOM and scope queries to the\nrender container, so the two-tone swatch hex extraction no longer depends on\njsdom's order-sensitive background-shorthand re-serialization (full-suite flake).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: assert accent swatches via data attributes, not jsdom css\n\nAdd data-accent-color/-color2 attributes (and a default-dot testid) as a\nCSS-independent test seam, and assert swatch two-tone colors on those. jsdom's\ncssstyle rejects a hex-stop linear-gradient under the full monorepo run, so the\nprior style-string detection found zero swatches. No runtime behavior change.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: drive nav and decoration accents from brand tokens plus ui polish\n\nRe-tint every component that hardcoded the violet/indigo pair (NavPill, glass\ndecorations, monitoring sparkline, onboarding, board-session glow) from\n--color-brand/--color-brand-2 so they follow the accent. Plus: two-tone gradient\nprimary CTA, settings chevron that travels with the active pill, light sidebar\nflattened to the content-card surface (single --color-card source), HoverPopover\ntrigger->panel dead-zone bridged (no more from-top close), footer AI status links\nto AI settings with the duplicate sidebar chip removed, and a white close-X on\nthe red Windows hover. Adds tests for NavPill, Button, HoverPopover,\nSettingsSidebar, and StatusBar.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: gate cursor-glow on reduced motion and harden accent derivation\n\nAddress CodeRabbit review on the accent work: skip the cursor-glow pointer/RAF\nloop under prefers-reduced-motion (paint once at center); in applyAccent only\nreuse a hand-tuned accentColor2 for the 'custom' source so switching to 'system'\nno longer inherits a stale preset gradient-end; guard rotateHueHex against\nnon-finite deg. Also fixes noUncheckedIndexedAccess tsc errors in the new\nSettingsSidebar test.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: thin-pointer the accent section and correct adr-018 motion claims\n\nAddress CodeRabbit: trim the DESIGN_SYSTEM accent-derivation block to point at\napplyAccent / rotateHueHex / readableForeground instead of copying their literals;\ncorrect ADR-018 so it states the cursor-glow RAF runs in balanced+performance\n(not keyframes-only) and that reduced motion gates both the CSS keyframes and the\nJS cursor-glow loop.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-14T05:21:32+02:00",
          "tree_id": "af43d2c0659e18d1a0410bcff0c7d6134487d63b",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/3235b059238f91d6e087c90048c7040b4240bbd2"
        },
        "date": 1781407751758,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2956861,
            "range": "± 20439",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3466296,
            "range": "± 54900",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 261070,
            "range": "± 4203",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "78cd2bd55944cda1a831eb34c1172615d06da0cd",
          "message": "feat: custom performance mode with resolved profiles and real backend tiers (#380)\n\n* feat: resolve performance mode to one profile with per-element custom tuning\n\nEvery mode (low-memory/balanced/performance/custom) resolves to one PerformanceProfile (visual flags + backend tiers).\n\nPresets are constants reproducing today's exact output; custom is a user-editable profile.\n\nCinematicBackground, the data-perf-* attributes, and the IPC backend slice read the resolved profile instead of the bare mode string.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: apply real performance tiers in the shell (concurrency, keep-alive, cache bounds)\n\nsystem_set_performance_mode applies the resolved backend config: scraper concurrency (clamped 1-16), Ollama keep_alive, and match_scores/posting_vectors cache TTL + row-cap eviction.\n\nA new L0 performance module (OnceLock<ArcSwap<PerformanceConfig>>, default balanced) holds the live tuning so the AppHandle-less Ollama embed builder can read keep_alive. Local/Ollama only.\n\nCaches gain created_at indexes; balanced now bounds them (2000 rows / 7d) while performance stays unbounded.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: record resolved performance-profile decision (adr-019)\n\nADR-019 captures the single resolved-profile architecture, the L0 process-global keep-alive source, and the cache-bounding tradeoff; DESIGN_DECISIONS performance section points at it.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* style: rustfmt the performance-tier shell changes\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: address coderabbit review on custom performance mode\n\nGuard the one-shot cache prune with try_state so a degraded boot no longer panics command handling (DocumentStore::open failure is non-fatal).\n\nPerformanceModeProvider caches the IPC payload only after the push resolves, so a failed first call retries instead of deduping forever.\n\nresetPreferences clears the optional customPerformance that shallow-merge left stale.\n\nresolveBackendConfig falls back to balanced tiers on malformed input, preserving the null no-limit cache sentinel.\n\nDisable modal blur on the reduced blur tier too (low-memory maps to reduced), restoring the historical low-memory look.\n\nBump PROTOCOL_VERSION 1.0.0 to 1.1.0 for the changed IPC payload shape.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: harden custom perf-mode tests against global-config races and flaky waits\n\nAdd serial_test and mark the 18 cache tests touching the process-global PerformanceConfig (or the match_scores/posting_vectors caches) #[serial].\n\nThis stops config mutations leaking across the parallel cargo test binary.\n\nRestore window.matchMedia after stubbing in CinematicBackground; replace setTimeout(0) effect settles with deterministic waitFor on observable changes.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: clarify adr-019 trust-boundary note for custom-mode payloads\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: keep statusbar popover open when the cursor enters from above\n\nOpening upward, HoverPopover portalled its panel under the descending cursor.\n\nA synthetic mouseenter never fires for an element inserted under a present pointer, so the panel never cancelled the wrapper's scheduled close.\n\nHover-close is now geometry-based: while open, a rAF-throttled document pointermove keeps it open when the pointer is within the trigger or panel rect (8px bridge).\n\npointerleave closes it; the racy mouseenter/leave close handlers are removed.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: reorder landing footer and drop the tracking quip\n\nHeart sits between the byline and the Privacy/Chrome-extension line, which now sits last; removed the 'we don't track you' footnote fragment.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-14T10:14:50+02:00",
          "tree_id": "139d7ef2b38a9db509a032372eec66f914cded28",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/78cd2bd55944cda1a831eb34c1172615d06da0cd"
        },
        "date": 1781425974989,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 3180424,
            "range": "± 84836",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 3839463,
            "range": "± 101873",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 298088,
            "range": "± 8272",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "74199796c5f74ba482bf93e920033074398091e7",
          "message": "Codebase health sweep: data-safety, scoring, providers, security, perf + 3 user-bug fixes (#381)\n\n* fix: make data-store restore, status writes, and migrations atomic\n\n- wrap ai_generations/applications import (clear+repopulate) in one transaction (C1)\n- pre-validate all data_import sections before mutating any store (C2)\n- central db::open sets busy_timeout + WAL; route our stores through it (H9)\n- wrap status row + history event writes in one transaction (H10)\n- run each migration body + user_version bump in one transaction (H11)\n- map SQLite-boundary errors to AppError::Storage instead of stringly errors (M4)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: unify provider-context and ai-stream helpers across generation paths\n\n- extract resolveActiveProvider + resolveEffectiveTier (dedupe 10 copies)\n- fix compact-prompt tier bug where resume-ai ignored the provider arg\n- extract awaitAiStream with the abort-before-register guard resume-ai lacked\n- pass a provider profile to prompt builders instead of a bare tier string\n- add @ajh/prompts/provider subpath export\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: unify autopilot ranking on the shared keyword-coverage kernel\n\n- delete naive jaccard simple_similarity; autopilot uses the ats keyword kernel (embedding-free)\n- move keyword_coverage into documents::keywords as the single source of the formula\n- batch match: build id->text map once under one lock (O(1) per job, not O(n*batch))\n- detect_field: word-boundary match + score-all-take-max (no 'finance' misclassification)\n- surface unstemmed/readable gap terms instead of snowball stems\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: stop defaulting analyze scores to 50 and wire dead settings controls\n\n- analyze: missing/NaN scores become null 'Not scored' instead of a fabricated 50\n- debugMode now gates the provider debug badge (was cosmetic)\n- persist recent job locations (was ephemeral useState behind a 'Recent' label)\n- resume download routes through the shared exportTXT util, not imperative DOM\n- add resetOnboarding store action; add useGeocodeSuggest + lazy provider-models hooks\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: cache fonts, offload typst compile off the async runtime, drop dead export code\n\n- hoist LoadedFonts to a process-wide LazyLock (no re-parse of 15 faces per render) (H4)\n- remove 4 unreachable embedded fonts + fix the stale face-count doc (M1/L3)\n- run pdf/docx compile inside spawn_blocking instead of on the tokio worker (H5)\n- apply_to_header falls back to contact-profile full_name when header name is blank (H6)\n- delete the dead legacy resume-docx arm + its helpers; model_docx is the only path (M2)\n- delete the no-op move_orphan_headers / reflow_overflow autofix stubs (M3)\n- document the dormant accent path explicitly; fix the stale export module doc (M4/L1)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: explain backend performance options and tighten renderer polling\n\n- add per-tier help (concurrency/keep-alive/cache) sourced from the real config mapping\n- raise useSystemHealth + extension-bridge polling to 30s; drop redundant useJob polling\n- extract one shared useFormatRelativeTime hook (dedupe 3 copies) with full m/h/d/w/mo tiers\n- i18n the performance, embeddings, tech-stack, and thinking-bubble strings (en + de)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: smooth aurora ribbon motion with seamless eased keyframe loops\n\naurora/nebula keyframes were open ramps (0% != 100%) run linear infinite, so each\nloop hard-snapped back ~160vw and the 50% stop injected a sharp vertical reversal\nunder linear timing - the bouncing/twitching. rewrote them as closed loops with\nease-in-out timing; colors, gradients, blur, and reduced-motion handling unchanged.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: pin windows installer to currentuser scope to stop update and pin drift\n\nno bundle.windows config meant nsis used a default scope while an msi was also\nshipped; the updater only applies nsis, so installs drifted across scope/path and\npinned shortcuts kept launching the stale exe after an update. pin nsis installMode\nto currentUser + webviewInstallMode downloadBootstrapper; document the one-time\nclean-reinstall migration for existing users.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: live-update accent color when the os accent changes\n\n- add a windows WinRT UISettings::ColorValuesChanged watcher (managed AccentWatcher\n  state keeps the subscription alive) that emits system:accentChanged via emit_event\n- renderer re-pulls the system accent on that event and on window focus, and re-applies\n  the theme when accentSource is 'system' (the previously-frozen hex now refreshes)\n- add reapplySystemAccent helper in @ajh/ui; wire system:accentChanged through\n  @ajh/shared events + the system IPC contract + the tauri client + mock client\n- macOS live-accent is a follow-up; the WinRT callback still needs a real windows test\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: dedupe provider streaming, add 429/5xx backoff, gate anthropic thinking\n\n- extract one stream_response loop owning cancel-check/chunk-read/emit/complete; each\n  provider supplies only its delta parser (SSE/JSON-array/NDJSON framing preserved) (H17)\n- add bounded exponential backoff honoring Retry-After on non-streaming complete/embed\n- gate anthropic extended-thinking on model capability (no unconditional +50% tokens/400 risk)\n- probe test_key via GET /v1/models instead of a hardcoded model snapshot\n- re-embed documents in bounded-concurrent chunks of 4 instead of fully serial\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: share an export picker and collapse the generation-card accordion code\n\n- extract a shared ModalShell-based ExportPicker used by GenerationCard + autopilot output\n- replace 5 hand-rolled height-animation accordion blocks with a local Section component\n- GenerationCard 665 to 567 lines; autopilot export dropdown is now the accessible modal\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: harden glassdoor/smartrecruiters/personio/xing scrapers and drop apply-engine ghosts\n\n- glassdoor: stream on_item per card, stable external_id (url-hash fallback), wire\n  location into the query, warn+bail on a captcha/sign-in wall instead of silent-empty\n- smartrecruiters: jitter between detail fetches + skip-on-error instead of aborting the batch\n- personio: dotall regex so multi-line descriptions are not truncated at the first newline\n- fetch_json logs parse failures; xing prefers the data-testid title; germantechjobs warns on empty parse\n- delete dead shutdown + to_cookie_params ghosts; replace blanket allow(dead_code) with targeted per-field allows\n- note: scrape_board filter propagation deferred (fields absent from the ipc schema; needs a contract change)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: remove dead components, barrels, and unused exports\n\n- delete 7 unrendered files (dashboard AIInsights/ContinueWorking/RecentActivity + its barrel,\n  jobs MatchScoreCard, onboarding PrefsStep, ai-generate WizardField)\n- remove 8 dead exports and de-export 3 symbols that are only used internally\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: add anti-abuse limits to ai/scrape commands and bound cache/db work\n\n- add an in-memory rate + concurrency limiter (generous defaults) on ai_generate and\n  scrape_board/scrape_url, plus a per-provider daily ceiling; new RateLimited error (H13)\n- add KvCache::prune and call it from system_set_performance_mode (was unbounded)\n- count embedding vectors via SQL instead of deserializing every blob; gate\n  backfill_vector_dims behind a one-time migration; prune posting vectors on a cadence\n- delete the dead autopilot_rank helper; fix 2 clippy warnings (clippy now clean)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: remove dead ipc commands and stream field; propagate scrape filter fields\n\n- remove 13 dead #[tauri::command]s (7 documents vector-ops + 6 boards-config) with\n  zero renderer callers, plus their handler registrations and now-dead store helpers\n  (kept get_vector/upsert_vector used by match_resume; moved cosine coverage to ai_provider)\n- remove the unread stream field from the ai generate contract (everything streams)\n- add 7 optional scrape filter fields to the contract and propagate them into BoardSearchInput\n  (linkedin consumes them; ScrapeForm UI controls are a follow-up)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: cover privacy reset, credential metadata, import rollback, and score formula\n\n- privacy: Resettable reset() per store + a registry-label lock matching lib.rs setup (C3)\n- credentials: metadata list/remove/clear_all/namespacing round-trip (C4; OS keychain not mockable)\n- import rollback: a malformed later record aborts and leaves prior data intact (R1 guard)\n- match: combined = round(0.6*sem+0.4*ats), degrade to keyword-only, explanation reflects state (A6)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: gate the rust suite with --locked and guard migration/wal/retry/personio paths\n\n- add --locked to the ci nextest run so Cargo.lock drift can't pass tests\n- db::open WAL read-back + run_migrations partial-failure rollback (user_version not advanced)\n- send_with_retry loop via wiremock (429x2 then 200, max-attempts giveup, terminal 4xx, 200)\n- personio dotall regex captures multi-line descriptions (regression guard)\n- de-flake credentials test_now_ms (asymmetric upper bound, no wall-clock window)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: record sweep adrs (020-022), knowledge notes, and architecture status\n\n- adr-020 unified autopilot scoring kernel; adr-021 windows installer currentUser scope;\n  adr-022 atomic store transactions + centralized db::open\n- new knowledge notes: matching-algorithm, persistence, anti-abuse-limits\n- sync ARCHITECTURE_STATUS + PATTERNS; remove the completed browser-extension roadmap row\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: url-encode renderer-controlled linkedin scrape filter values\n\nthe newly-wired job_type/work_type/experience_level/sort_by filters were interpolated\nraw into the search url while sibling params encode; wrap them in urlencoding::encode\nto prevent query-param injection/truncation from a malicious or looping renderer.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: satisfy tsc for nullable section score and relative-time key map\n\n- narrow sec.score inline so the non-null branch types as number (not number | null)\n- extract a non-indexed JOBS_KEYS fallback so 'keys' is defined under noUncheckedIndexedAccess\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: satisfy architecture conformance for new modules and removed code\n\n- route the retry test client through net::http::shared() (R5)\n- classify the new limits module at L0 (every-module-classified)\n- return AppResult from validate_sections and the stream test bound instead of Result<_, String> (R6)\n- allowlist platform/accent_watcher holding AppHandle + importing events (bootstrap shell-reach, R2/R7)\n- drop the now-dead autopilot_helpers allowlist entry (R7)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* style: rustfmt branch files and fix a clippy field-reassign in a privacy test\n\n- use struct-update syntax for the ContactProfile test value (clippy field_reassign_with_default)\n- apply cargo fmt across files changed on this branch (whitespace only, no behavior change)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add an instant macos accent-change watcher\n\n- observe AppleColorPreferencesChangedNotification via CFNotificationCenter\n  (core-foundation-sys, already in the lockfile) and emit system:accentChanged\n- keep the observer alive in managed state; remove-observer-then-free on drop\n- macos arm is unverified on this windows dev host; the macos ci build is the verifier\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: animate aurora on macos webkit and keep the default accent swatch true\n\n- aurora/nebula keyframes animated transform with vw/vh; webkit (wkwebview) does not\n  animate viewport units in transforms (webkit bug 91554), so they froze on macos.\n  convert the keyframe translations to element-relative % (the documented workaround);\n  chromium/webview2 behavior is visually unchanged. needs macos runtime confirmation.\n- the Default accent dot read the live --color-brand, which applyAccent overrides when a\n  custom/system accent is active; add un-overridden --color-brand-base/-2-base tokens and\n  use them for the dot so it always shows the shipped default.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: address coderabbit rust review on the sweep\n\n- gemini: skip depth-0 array separators so wrapped objects parse (+ array regression test)\n- reembed: count write failures as failed (not done); cancelled runs report cancelled not completed\n- autopilot: score the same title+description+requirements blob as match_resume (shared posting_text_blob)\n- xing: insert dedup id after the empty-title guard so a bad parse can't suppress later cards\n- privacy: single-source MANAGE_RESETTABLE_LABELS used by setup + the completeness test\n- referrals: pre-validate then clear+repopulate in one transaction (no destructive partial restore)\n- ai_generations: carry application_id through export/import so backup keeps the FK link\n- scrape: apply the rate/concurrency limiter to scrape_resolve_url too\n- smartrecruiters: run pacing/progress on detail-fetch failures; http: stop logging raw body previews\n- keywords: deterministic display_forms; pipeline cache: guard negative ttl/max_rows\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: address coderabbit review and correct the scoring-kernel description\n\n- describe coverage_score accurately: embedding-free, deterministic, zero API calls, no\n  semantic fallback; only the Jobs-page combined score_one uses embeddings (0.6 sem + 0.4 ats)\n- fix the coverage_score example signature; clarify restore is per-store atomic, not cross-file\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: address coderabbit ts review (stream timeout/cancel, nullable scores, accent re-apply)\n\n- stream-promise: timeout now rejects and cancels the backend job; abort paths swallow cancel rejections\n- analyze: whitespace-only scores become null not 0; schema + system-prompt allow null scores (never fabricate a midpoint)\n- use-system: compare the accent query-key array instead of the internal react-query queryHash\n- use-jobs: document the hard requirement that a useJobEvents subscription be mounted\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: recreate the readme banner as a self-contained svg matching the landing style\n\ndark #0a0c11 + red #e24b4a, the favicon face doodle, a paper 'please hire him' speech\nbubble, a hand-drawn red underline, the 'it does everything but press send' tagline, and\nsubtle grain. no external fonts/scripts (renders on github; design holds if filters are stripped).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: resolve coderabbit findings on health-sweep pr\n\n- cache/mod.rs: fix prune max-rows off-by-one — Some(0) now clears the\n  table (was keeping 1 row); cutoff uses OFFSET n-1 so exactly the n\n  newest rows survive (was n+1). Adds row-cap unit tests.\n- stream-promise.ts: guard the job-status setInterval against overlapping\n  async polls (pollInFlight flag + reset in .finally) so a slow\n  api.jobs.get never stacks concurrent polls.\n- utilities.css: remove dead aurora-4 / nebula-3 / nebula-4 keyframes and\n  their .animate-* classes (zero consumers; the only keyframes still in\n  vw/vh) — completes the macOS-WebKit element-relative-% migration.\n- keywords.rs: posting_text_blob joins with \"\\n\" (was a multi-line\n  literal).\n- docs: thin matching-algorithm.md + adr-020 to source pointers; drop the\n  copied scoring formula literals and the coverage_score signature block\n  so the runtime code stays the single source of truth.\n- gitignore: ignore /rtk/ (rtk token-saver local history db).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: correct keyword scorer description from jaccard to coverage\n\nmatching-algorithm.md described the kernel as \"Jaccard-based (intersection\n÷ union)\" but keyword_coverage() computes |job ∩ resume| / |job| — the\nshare of the job's keyword set matched by the resume, not Jaccard. Fix the\nwording to match the source (documents/keywords.rs::keyword_coverage).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-14T17:17:28+02:00",
          "tree_id": "bf33df2cda21e88c494c7a9da3909911c7862a3d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/74199796c5f74ba482bf93e920033074398091e7"
        },
        "date": 1781450723313,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1889754,
            "range": "± 65308",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2528566,
            "range": "± 32282",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 292814,
            "range": "± 3588",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "22298c3d7a3da96649f75cefc93ea59e67eebf8f",
          "message": "feat: ui improvements across applications, settings, jobs, onboarding, and the design system (#383)\n\n* feat: default accent emerald->amber + theme-aware control borders\n\nSwap the shipped default brand from violet->indigo to emerald\n(#2ea36b dark / #0f6b3f light) -> amber (#f7a325 / #c2740a) across both\nschemes plus the rgb aliases. Accent stays single-source, so the focus\nring, aurora/nebula, glows and gradients re-tint automatically.\n\nAdd a scheme-flipping --color-brand-foreground (dark ink in dark, white\nin light) for the primary button, so its label clears AA on the bright\namber gradient tail (white-on-amber was 2.05:1 in dark).\n\nMove the notification bell and jobs filter/sort borders off hardcoded\nwhite opacities onto the themed foreground/border tokens so they are\nvisible in both light and dark.\n\nRetint the landing hero (emerald) and beat1 (amber) section backgrounds\nto echo the new accent; backgrounds only, section seams unchanged.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: make default input and dropdown fills visible in light and dark\n\nThe default Input wrapper, bare default input, and Dropdown trigger\nfilled at rgb(glass/0.08) (and the bare input at white/5), which is\nnear-invisible on the light canvas and faint on dark. Switch the resting\nfill to bg-card (solid white in light, the #272729 tile in dark, like the\ndropdown panel) and the hover/open state to bg-muted, so the controls\nread as distinct fields in both schemes.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: ui polish across themes - visibility, button sizing, toast and menus\n\nLight/dark visibility:\n- New --color-field token (white in light, inset #1a1a1d in dark) so the\n  default Input + Dropdown fills read on every surface; notification-bell\n  and jobs filter/sort borders move to themed foreground/border tokens.\n\nButton sizing:\n- Drop size=\"sm\" app-wide so every Button defaults to h-8; align the default\n  Input and Dropdown (md) to h-8; icon-only buttons become square h-8 w-8.\n\nJobs:\n- Toolbar (scrape/clear/filter/sort) and scrape-form buttons all h-8; fix the\n  filter field width jumping on type (w-48 moved to the wrapper); Save flips\n  to a \"View\" link (jobs.view, en/de) once the posting is saved/bookmarked.\n\nToast and action menu:\n- Redesign the toast onto the themed glass surface with a per-variant glow\n  from the left and a white icon glyph (fixes the invisible check); tighten\n  the ActionMenu (176px, smaller padding and text).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: persist scraped jobs across nav and clear old results only on a successful new search\n\nScraped jobs lived only in ephemeral renderer state (livePostings); the postings\nquery was invalidated at scrape start when the backend cache was still empty, so\nthe list vanished on navigation. Now the query is invalidated on job.completed\n(and on the first new result) so it reflects the app-lifetime PostingsCache, which\npersists across navigation and clears on app close.\n\nNew-search replacement: add a `replace` flag to ScrapeBoardRequest. When set, the\nscrape's first streamed item clears the live cache before adding itself, so an\nerrored or empty search keeps the previous results; \"show more\" still appends. The\nrenderer no longer wipes results up-front.\n\nAlso folds in pending UI polish on this branch:\n- redesign the toast notification surface for light/dark, fix the first-toast\n  enter animation, and animate the Save->View button width change\n- bump posting-row status chips from the un-remapped -200 step to -300 so the\n  saved/remote/applied/viewed chips stay legible in light mode\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: start jobs scrape on enter and make confirm buttons clearly visible\n\n- Jobs scrape form: pressing Enter in the query field now starts the scrape,\n  gated by the same condition as the Start button (not scraping, query non-empty).\n  LocationInput keeps its own Enter handling for picking an autocomplete suggestion.\n- ConfirmModal: the confirm action was a low-contrast outline (transparent bg, faint\n  colored border + colored text) that was hard to see in both themes. Switch all four\n  variants (danger/warning/info/success) to solid filled buttons with AA-contrast text,\n  so destructive actions like \"Clear scraped jobs\" read clearly. Ghost Cancel unchanged.\n- Toast: move the icon glyph white out of the inline style object into a named constant\n  so lint:strict (--max-warnings 0) passes; rendered color unchanged.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: retry linkedin search without geoid when it returns empty\n\nLinkedIn's guest jobs endpoint soft-blocks geoId-filtered queries to an empty\nresult set (a 200 with zero cards, indistinguishable from \"no jobs\"), while the\nfree-text location filter stays robust. Because the scraper resolved a geoId from\nthe location and appended it to every page, location searches frequently came back\nempty while no-location searches worked — confirmed against the live endpoint.\n\nsearch_paginated now treats an empty first page that carried a geoId as a soft\nblock: it drops geoId (and distance, which LinkedIn ignores without a geoId), waits\na short jittered backoff, and retries the page once with free-text location only,\nkeeping geoId off for the remaining pages. Legitimate empty results without a\ngeoId, and successful geoId searches, are unaffected.\n\nThe jittered backoffs (retry + inter-page) now run through a cancellable_sleep\nhelper so an in-flight cancel is honored mid-await instead of only at the next loop\niteration. Adds network-free unit tests for the helper.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: honor requested job count per board with a central scrape cap\n\nThe renderer used to convert the requested item count to a page count via\nceil(amount/25) — wrong for every board, since page sizes differ (LinkedIn 10,\nWorkday 20, single-shot boards return everything). The IPC contract now carries\n`amount` (target item count, 1-100) instead of `pages`, and the scraper engine\nenforces it centrally: it counts streamed postings, drops anything past `amount`,\nand cancels the job token the instant the target is reached so each board's own\npagination stops early at its real page size. Single-shot boards are truncated.\n\nA per-board page budget (engine passes 10; each board clamps to its own max)\nbounds request volume, and `amount` is clamped on both the Zod and Rust sides.\n\nAlso fixes LinkedIn pagination skipping jobs: PAGE_SIZE was 25 while the guest\nendpoint returns 10 per request, so multi-page scrapes stepped the offset past\njobs 10-24, 35-49, … Set PAGE_SIZE to the real 10 so offsets align.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: make destructive menu items legible in dark and align control heights\n\n- ActionMenu destructive items and the FieldArrayList trash hover used\n  text-action-delete (= --color-destructive, a dark L=30% red meant for filled\n  button backgrounds), which was nearly invisible as text on dark surfaces. Use\n  the theme-adaptive text-red-400 (bright on dark, remapped to deep red-600 on\n  light) so destructive actions read clearly in both themes.\n- ActionMenu width now fits its content (min 176, max 300, right-anchored to the\n  trigger) instead of a fixed 176, so long items like \"Remove tracking (keep\n  documents)\" are no longer truncated while short menus stay compact.\n- Applications filter input height h-7 -> h-8 to match the Track-a-job button.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: deep-linkable application detail and autopilot apply routes\n\n- /applications/$id: application detail hub — status dropdown + timeline,\n  save-on-blur editable fields, documents via the shared GenerationCard with a\n  Generate CTA; clicking an application row opens it (keyboard-accessible).\n- /autopilot/apply: the apply flow is now an in-session route (URL + browser\n  back) instead of an in-body view swap; cold URL redirects to /autopilot.\n- Restructure both sections into a layout (<Outlet/>) + index route so the\n  detail/apply pages mount as full pages.\n- Fix the root unknown-path guard that redirected every dynamic route to '/'\n  (it matched the resolved pathname against route patterns). It now inspects the\n  resolved route match, so param routes are kept; extracted to lib/router-guard.ts\n  with a real-router regression test.\n- Promote GenerationCard to a shared features/documents slice (used by resumes\n  and applications).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: tabbed application detail page with inline document generation\n\nRebuild the application detail page (/applications/$id) as a slim header over a\nbordered 4-tab panel (Overview / Timeline / Brief & answers / Documents) with the\nactive tab kept in the URL (?tab=). The Documents tab generates a tailored resume\nand cover letter inline, at full parity with the autopilot apply flow (incl. the\nquestions assistant and referral finder).\n\nExtract the apply-flow body into a shared TailorFlow component under\nfeatures/documents/components/TailorFlow, consumed by both the autopilot apply page\n(now a thin host) and the application detail page via an injected persistence object\nplus an onController controller seam, so each surface owns its own session slice and\nthey share one generation per job. Add an applicationApply session slice; key the\ngeneration session by application id when the job URL is empty to avoid cross-app\nlive-session bleed.\n\nAdd a documents.getText IPC (documents_get_text) so the generator seeds the user's\ndefault resume.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: default accent to a teal-gold-rose three-stop gradient\n\nReplace the shipped emerald->amber 2-stop brand accent with a three-stop\nsweep: --color-brand teal #366569 (the single accent for links, focus ring,\nsolid buttons, badges, action-primary), a new --color-brand-mid gold #ba9249,\nand --color-brand-2 rose #995f5c. The hero brand gradients (primary button,\ngradient text, aurora) become three-stop; subtle ambient glows stay two-stop.\n\nDeepen each stop in the light scheme for white-canvas legibility, tune\n--color-brand-foreground to dark ink (legible over the bright gold middle where\nthe label centres) and add a scoped text-shadow on the primary button label for\nthe darker teal/rose ends. Add a mixHex helper so the runtime accent applier\nderives a midpoint for custom/system accents (no gold left stuck mid-gradient),\nand add the mid vars to ACCENT_VARS so the Default accent restores them.\n\nAlso fix three stale tokens surfaced en route: the old amber aurora-indigo RGB\ntriplet, the light-scheme violet brand glow, and the missing light --rgb-brand.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: refine buttons, search inputs, and documents selection mode\n\n- Résumé \"Upload\" tab shows the primary brand look when selected and a muted\n  state when not, so the upload/paste segmented control stays legible.\n- Button default text → font-medium + md text-[11px]; rescale sm (text-[10px])\n  and lg (text-[13px]) for a monotonic 10/11/13 progression.\n- Documents page search input now uses the shared Input pattern; Jobs and\n  Applications search inputs drop the text-xs override so all match at text-sm.\n- Documents page hides Select-all + per-card checkboxes behind a \"Select\"\n  action button (selection mode + Done), and keeps that button visible when a\n  search filter returns no results.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* build: resolve @ajh/translations to source for dev hot-reload\n\nAlias @ajh/translations to packages/translations/src/index.ts in the renderer\nVite config (mirroring the existing @ajh/ui source alias), so editing a\ntranslation.json or the i18n config hot-reloads in dev instead of serving the\nprebuilt dist, which previously required a manual package rebuild.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: fix tsc type errors in tailor-flow and application-detail tests\n\nThe TailorFlow persistence-injection helper spread overrides as\nPartial<TailorFlowPersistence>, widening the typed vi.fn setters back to plain\nfunctions and failing strict tsc; type the mock setters via vi.fn<T>() and the\noverrides against the mocked shape. In the application-detail test, type the\nsession mock so applyForId is string | null rather than the inferred literal\nnull. These passed under vitest/esbuild but failed tsc in CI; no behavior change.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: rename the resumes feature/page/route to documents\n\nThe page has long been titled \"Documents\"; align the code. Move\nfeatures/resumes (ResumesPage, InteractionRow, constants) into\nfeatures/documents as DocumentsPage, rename the route /resumes -> /documents,\nand update the sidebar nav, dashboard quick-action, the notification deep-link\ntable, and the native menu shortcut (Ctrl/Cmd+5) in the Rust backend.\n\nThe internal resumes.* i18n namespace and the session-store resumes slice are\nintentionally left as-is (shared by GenerationCard, hooks, ExportPicker, etc.).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: update button css-check story to the font-medium default\n\nThe Button base weight changed from font-normal (400) to font-medium (500), but\nthe CssCheck story still asserted computed fontWeight === '400' and only ran in\nthe Storybook/chromium vitest project (skipped by the package test filter, so it\nsurfaced at pre-push). Assert '500' to match; still a non-UA value, so the\nstory's \"design-system CSS loaded\" guarantee is preserved.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* style: apply rustfmt to backend\n\nPure `cargo fmt` line-wrapping with no logic change: wrap the NAV_ITEMS menu\ntuple and the documents_get_text test's store call chain, and rewrap two\nscraper/cancellable-sleep lines. Resolves the pre-push `cargo fmt --check` gate\n(this branch had never been pushed, so accumulated rustfmt drift surfaced now).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: redesign application detail layout and documents flow\n\n- group the overview tab into Notes / Contact / Tracking cards with label-above fields and consistent inputs\n- move the status dropdown beside the job-link button and rename \"open job link\" to \"Job link\"\n- make the documents tab a full-height tailoring host (drop the saved-generations list) to mirror the autopilot apply flow\n- update the detail tests and en/de translations\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: return to the last viewed application from the sidebar\n\nThe sidebar Applications item now reopens the last-viewed application detail\n(id + active tab) instead of always resetting to the list. Session-scoped\n(the Sidebar is always mounted) and cleared back to the list when the user\nlands on the list route itself.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: set default accent to teal-peach-coral sweep\n\n- update the dark and light default sweep stops plus their soft and base steps\n- resync the rgb-brand and aurora rgb aliases to the new start and end\n- derive soft steps with the runtime lighten algorithm so static defaults match the applier\n- update the theme test that pinned the previous default mid\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: route autopilot apply into the applications hub\n\n- promote Applications to 2nd in the sidebar nav\n- autopilot Apply now creates (or reuses) the Application via saveFromPosting and deep-links to /applications/$id?tab=documents, carrying the autopilot résumé seed and match-level badge\n- the detail Back button returns to Autopilot when opened from Apply (via ?from=autopilot)\n- remove the standalone /autopilot/apply ApplyPage + ApplyPageRoute + route and the dead autopilot apply session state\n- update affected tests and en/de translations\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add interview-questions assistant for questions to ask the interviewer\n\nAdds an AI assistant that suggests sharp questions the candidate can ASK the\ninterviewer, alongside the existing application-answers assistant.\n\n- new InterviewQuestion type persisted on the ai_generations aggregate: additive\n  SQLite migration, merge-upsert by job URL, IPC contract, export/import and GDPR\n  reset all thread the field (ADR-007)\n- interview-questions prompt grounded in the job ad + fenced company-research\n  brief (ADR-010) + résumé context + user seed topics, with a quality bar that\n  bans generic / careers-page / self-serving questions\n- research is ALWAYS gathered for this flow (not gated on the cover-letter\n  toggle) so questions cite concrete, current company/role intel\n- generateInterviewQuestions + a lenient delimited parser (no provider JSON-mode\n  dependency) and a shared useInterviewQuestions hook\n- new \"Interview prep\" application-detail tab: seed topics, generate/regenerate,\n  questions grouped by audience\n- tests: backend round-trip + merge, prompt fencing, parser; fixtures updated\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add interview-questions trigger to the tailor flow toolbar\n\n- TailorFlowController gains openInterviewQuestions + interviewQuestionsCount\n- TailorFlow instantiates useInterviewQuestions and renders an InterviewQuestionsModal (apply-time second surface alongside the Interview-prep tab)\n- the Documents-tab toolbar gets a \"Questions to ask\" button with a count badge\n- modal title/hint i18n (en/de) and TailorFlow test mocks for the new hook + modal\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: show saved docs on the documents tab and target interview questions by audience\n\nDocuments tab rehydrates the latest saved ai_generations record into the\ngeneration session (guarded, no-clobber) so re-entry opens on the generated\nresume/cover instead of the configuring wizard.\n\nInterview-questions assistant gains audience targeting: recruiter/HR (1st round)\nand hiring-manager questions stay non-technical (role, team, process, culture,\nwork-life); team/leadership go deeper. A multi-select audience picker drives\ngeneration (default recruiter/HR + hiring manager, ~4 questions per audience),\nand results render in an Accordion grouped by interviewer, shared by the detail\ntab and the apply-time modal.\n\nAlso folderizes packages/prompts/src/generate so each module lives in its own\ndirectory, matching the repo module-folder convention.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: update landing diagram paths for the prompts generate folderization\n\nThe architecture-map cited flat generate/*.ts paths; point them at the new\nper-module folders so the landing-drift guard passes.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: warn that ollama needs a web-search key when generating interview questions\n\nInterview-prep generation always runs company web research; Ollama-family\nproviders need the free Ollama account key for it. Surface a non-blocking hint\n(reusing aiGenerate.research.ollamaKeyHint) in both the Interview-prep tab and\nthe apply-time modal when an Ollama provider lacks the key, computed once in\nuseInterviewQuestions (needsResearchKey) so both surfaces stay in lockstep.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add a timeline ui component and use it for the application timeline\n\nNew @ajh/ui Timeline (antd parity): coloured/custom dots on a connecting rail,\nopposite-side labels via mode (left/right/alternate), a pending node with a\nspinner, and reverse ordering. Brand-aware default dot colour. The application\ndetail Timeline tab now renders through it, colouring dots by status.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: keep the applied autopilot expanded when returning from applications\n\nApplying from a found job deep-links into the application detail; pressing Back\nre-mounted the Autopilot page with every card collapsed. Remember the applied\nautopilot id on a new `lastAppliedId` session field and, on the page's next\nmount, promote it to `focusedId` so that card re-expands its found-jobs list\n(consumed once). Robust to the async router navigation.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: show the contact-profile photo as the sidebar footer avatar\n\nWhen the user has uploaded a contact-profile photo (a local data: URL), render\nit as the sidebar footer avatar instead of the generic user icon; fall back to\nthe icon when no photo is set.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add an image component with a zoom/rotate/flip preview lightbox\n\nNew @ajh/ui Image: loading placeholder, error fallback, and a hover mask that\nopens a full-screen lightbox (zoom via buttons/wheel/double-click, rotate, flip,\nreset, drag-to-pan, Escape/backdrop close). Image.PreviewGroup shares one viewer\nacross child images with prev/next navigation.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: drop third-party-library references from component comments\n\nRemove mentions of the third-party UI library these primitives were modelled on\nfrom their comments/docstrings (and one test name) — no behaviour change.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: make the create-autopilot buttons primary\n\nPromote the header \"New autopilot\" button and the empty-state \"create first\"\nbutton from the glass variant to primary.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: make application-detail surfaces theme-aware and add overview input placeholders\n\nThe detail page, its tabs, the interview modal, and the track-job modal used\nhardcoded white-opacity borders/backgrounds (border-white/[…], bg-white/[…])\nthat vanish in the light theme. Swap them for theme-flipping tokens\n(border-[var(--border-soft)] and bg-foreground/[…]) so the panel, dividers, and\nbadge read correctly in both themes. Add placeholders (en/de) to the Overview\nnotes, contact name/email, and compensation inputs.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: give the application-detail tab bar a card background\n\nUse the theme-aware `bg-card` for the tablist row (white in light mode, a dark\ntile in dark mode) and clip the panel to its rounded corners, so the tabs read\nas a distinct header strip over the faint panel body.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: drop the application-detail panel body fill\n\nThe faint bg-foreground/[0.02] fill read as an odd gray block below the cards in\nlight mode. Make the panel body transparent so it blends with the app background;\nthe white tab strip + hairline border still frame the tabbed content.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: redesign the application detail as a single white sheet\n\nReplace the bordered box + nested floating cards (which washed out in the light\ntheme) with one solid card sheet: the tab bar is a header row with a hairline,\nand the Overview sections (Notes / Contact / Tracking) are flat blocks separated\nby hairlines (IconBadge + SectionLabel headers, no inner cards).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: remove the drop shadow from the notes textarea\n\nThe glass variant's box-shadow read as a heavy float on the white detail sheet;\nflatten the notes field with shadow-none.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: remove the orphaned hybrid-search feature\n\nSearch was dropped from the sidebar nav but left wired up end-to-end. Remove the\nwhole self-contained slice: the SearchPage/route/ROUTES.SEARCH, the dashboard\ntile (and fix the stale Resumes->Documents label), the Cmd/Ctrl+K shortcut +\nShortcutsOverlay entry + tests, the notification-route allowlist and nav.search\ntranslations, the useSearch hook + tauri-client namespace, the search IPC\ncontract + HybridSearchRequestSchema + SearchHit type (+ gen-ipc-rust entry),\nand the Rust search_hybrid command + ipc_contracts/search.rs + handler. Shared\nscraping state (PostingsCache) is untouched.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: drop the search feature from the landing architecture diagrams\n\nRemove the search nodes/edges/issue-entries from architecture-map and the\nhybrid-search section + route/channel references from how-it-works, matching the\nhybrid-search feature removal so check:landing-drift passes.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add antd-style tag and form components\n\nTag + Tag.CheckableTag (presets, status, custom, closable, icon, bordered) and a thin Form/FormField wrapping react-hook-form.\n\nRHF is re-exported through @ajh/ui so feature code never imports it directly. Both theme-safe; unit tests and stories included.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: make resume builder and step indicators legible in light mode\n\nSurface-card step sheets, theme-token borders, the Alert primitive for warnings, and a StepDots inactive-dot fix.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: adopt the tag component for chips, scores, and status badges\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: stop the posting row opening the job link and tag its badges\n\nPostings have no detail view, so the row is no longer clickable; Open stays in the row menu. Status badges become plain Tags (non-interactive).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: route raw img tags through the image component\n\nPdfPreview (zoomable), avatars and template thumbnail (preview disabled); ESLint now bans raw img. Also: remove-photo is danger, add-link is info.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: render résumé template previews as svg\n\nThe ignored generator now exports per-template SVGs (vector, crisp at any zoom) and the glob matches *.svg; old PNGs removed.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add a download page with per-platform installers\n\nWide macOS/Windows/Linux rows with click-to-copy commands; a release CI job auto-syncs the versioned asset links; index CTA and footer link to it.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add console easter eggs to the landing pages\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add autopilot and applications dashboard quick actions\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: move match-level to shared lib\n\nShared by autopilot, applications, and now jobs; no longer a cross-feature import.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: route the jobs tailor button through the application apply flow\n\nTailor now saves the posting and opens the application's apply (documents) flow seeded with the row's match level, mirroring autopilot Apply, instead of going to /ai-generate.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: refresh onboarding wizard and tour for current feature set\n\nRender steps generically from a provider-filtered list: drop the Research step\nunless local Ollama is active, and clamp the step index on a provider flip so it\ncan never strand past the shortened list.\n\nAdd an Extension step (install the published Chrome extension; Firefox marked\ncoming soon) and an Appearance step (colour scheme + accent via the live theme\nengine) as the final steps before the tour. Add an Applications spotlight to the\ntour and reword the autopilot copy for the find-and-notify behaviour.\n\nExtract the shared scheme/accent constants so settings and onboarding stop\nduplicating them. Includes en/de translations, tests for the wizard, new steps,\nand tour ordering, plus the UX audit update.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: pin esbuild to a patched range to clear a dev-dependency advisory\n\nesbuild <0.28.1 (GHSA-gv7w-rqvm-qjhr) allows code execution via\nNPM_CONFIG_REGISTRY in the Deno install path. It is a transitive dev-only\ndependency (vitest/vite), never shipped in the Tauri bundle.\n\npnpm 11 ignores package.json#pnpm.overrides, so the pin lives in\npnpm-workspace.yaml as overrides.esbuild '>=0.28.1 <0.29'. The tree now\nresolves to a single esbuild 0.28.1 and pnpm audit --audit-level high is clean.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: swap the default skill policy from caveman to ponytail\n\nReplace the always-on caveman output style with the ponytail lazy-senior-dev\nskill in CLAUDE.md, AGENTS.md, and the SessionStart style-policy hook. Takes\neffect from the next session, when the hook injects the policy at session start.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: origin-aware application back and score-gated job results\n\nApplication detail Back now returns to its origin via an explicit `from` search\nparam (jobs | autopilot | applications), preserved across tab switches and\ndefaulting to the applications list for deep links (notification View, sidebar\nrestore). Entry points tag their origin; the label follows suit.\n\nJobs results are hidden behind a status loading state during scraping and match\nscoring, then revealed sorted by match score descending; with no default resume\nthey show immediately in the current sort order.\n\nAlso bundled (shared i18n/JobsPage files prevented a finer split): onboarding\npolish (hide titlebar chrome behind the onboarding backdrop, rename the browser\nstep CTA to Continue, primary forward buttons, theme-aware tour dots, ghost\nskip), removal of orphaned i18n keys and the useFormatRelativeTime shim, trimmed\ninternal-only @ajh/ui barrel exports, and removal of temporary router-guard\ninstrumentation.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: expand the tech-stack list and allow free-text entries\n\nMove the technology catalogue to a shared constants module (~200 entries across\nlanguages, frameworks, databases, and tools) and let users add ANY typed\ntechnology via Enter or the Add button — previously only exact matches from a\n~15-item hardcoded list could be added. Chip removal now animates.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: surface account warnings via alert and animate notification removal\n\nReplace the hand-rolled amber/red warning banners on the Accounts page with the\nAlert primitive (legible in light mode via foreground-mix text). Deleting a\nnotification now collapses and fades the row while the remaining items reflow\nupward; also tokenize the dropdown's white overlays.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: drop unused remote, seniority, and salary job-preference fields\n\nThese columns were persisted but had no settings UI writing them (only a dead\nautopilot read), so they were always null. Remove them from the Rust store via a\nSQLite-safe table-recreate migration that preserves location and tech stack, and\nnarrow the shared schema. Old backups still import (unknown keys ignored). The\nautopilot work-type no longer seeds from the removed field.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: seed the jobs scrape location from the preferred location\n\nPrefill the scrape form's location once from the saved job-preference location\nwhen it's empty, guarded so it never clobbers a user edit and never writes back\nto settings.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: make the settings surfaces legible in light mode\n\nThe settings UI was built dark-first with translucent white overlays that vanish\non the near-white light canvas. Sweep bg-white/border-white overlays to\nforeground-token equivalents across settings, contact, and the sidebar; replace\nthe hand-rolled launch-at-login and debug toggles with the Switch primitive;\ngive the danger zone, backend popover, CLI output block, action cards, and\ncontact modals theme-aware surfaces; route the language selector through Button\nand useTranslation; and remove the section-header divider.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: reveal jobs results when match-scoring errors instead of hanging\n\nThread the batch match-score query's isError through MatchScoresProvider\nso JobsResults opens its gate on a scoring failure. Previously isPending\nnever resolved on error and the list stayed hidden behind an infinite\nspinner; now the (unscored) results reveal.\n\nAlso harden the job-preferences column-drop migration with DROP TABLE IF\nEXISTS for defensive clarity, and add tests: the isError escape hatch,\nand full interaction coverage for the ImagePreview lightbox\n(zoom/rotate/flip/drag/keyboard) which was ~55% covered.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: ignore nested coverage dirs in eslint\n\n`coverage/**` only matched the repo root, so a generated\n`packages/ui/coverage/` made lint:strict (and the pre-push hook) fail.\n`**/coverage/**` covers per-package coverage output too.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-16T03:22:35+02:00",
          "tree_id": "9d9749e823975460a17fb386e7929c34b77c05d6",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/22298c3d7a3da96649f75cefc93ea59e67eebf8f"
        },
        "date": 1781574087477,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1940101,
            "range": "± 61116",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2595966,
            "range": "± 68165",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 298295,
            "range": "± 2124",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "465ea0b7eed5b97e92cde3f776df14c98f62d180",
          "message": "fix: import jobs as applications only, sidebar nav, tab order, popup UX + Firefox launch (#385)\n\n* fix: import extension jobs into applications only, not the jobs feed\n\nhandle_import wrote the parsed posting into the in-memory PostingsCache (the Jobs/discovery feed) on top of creating the saved Application, so imported jobs appeared in both lists.\n\nAn import is a deliberate pursuit, not a discovery, so it now creates the Application only. Detail-page tailoring uses the Application's own fields, so ATS and cover-letter are unaffected.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: make sidebar nav always open the section root\n\nThe Applications sidebar item remembered the last-viewed detail and re-pointed its link at the detail route. Clicking it from a detail page was a no-op, leaving the list unreachable from the sidebar.\n\nRemove the remember-last-detail machinery so every nav item is a plain Link to its route and always lands on that section's main page.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: lead application detail tabs with documents\n\nReorder the detail tab strip to Documents, Interview, Brief, Timeline, Overview so the tailor/generate flow leads.\n\nThe default landing tab stays overview (an explicit behavioral contract pinned by route-outlet-nesting.test.tsx); only the strip order changes.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add firefox extension install across onboarding, download page, and docs\n\nThe Firefox extension is now published on Mozilla Add-ons (AMO).\n\nEnable the onboarding Add-to-Firefox button (was a disabled coming-soon) and rename the onboarding.extension.firefoxSoon string to addToFirefox (en + de, with the test updated).\n\nAdd the AMO link to the download-page card, the landing + README footers and badges, and flip the dev README + manifest note from pending to published.\n\nNo auth.rs change: the bridge already accepts Firefox by moz-extension UUID shape.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: confirm pairing and add a help toggle in the extension popup\n\nOn a successful pair the Save and pair button now shows an Authorized confirmation briefly before the popup flips to the import view, instead of flipping instantly.\n\nReplace the redundant offline title (the app-not-running line, already shown by the status pill) with a help (?) toggle by the pill that explains the local-only connection.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: default the application detail to the documents tab\n\nNow that Documents leads the strip, make it the default landing tab too: a missing/unknown ?tab now coalesces to DETAIL_TABS[0] instead of a hardcoded overview.\n\nOpening an application lands on the primary tailor/generate tab rather than the last tab.\n\nUpdate route-outlet-nesting Case 4 to assert the new default; stub the Documents-tab data hooks so the panel renders without an AppClient in that integration test.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: harden extension pairing flow and cover it with tests\n\nWrap the post-validation pairing flow in try/catch so a rejected setToken/refreshStatus cannot strand the button disabled and labelled Authorized; always restore the actionable state.\n\nAdd behavioral popup tests: help-toggle aria sync, the Authorized-then-flip success path, and button restoration on a rejected pairing.\n\nAlso prettier-format popup.html (lint-staged does not cover .html), fixing the CI format check.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: settings extension copy reflects firefox availability\n\nThe settings extension tagline still said Firefox coming soon; update en + de to read \"Available for Chrome and Firefox\" now that AMO is live.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: lock the import-isolation contract (application-only)\n\nExtract handle_import's persistence into persist_import_application, which takes only the ApplicationStore (no PostingsCache) — so an import structurally cannot enter the Jobs/discovery feed.\n\nAdd unit tests: an import creates exactly one Saved Application from the posting, and an applied flag advances the status.\n\nResolves the CodeRabbit import-isolation finding without needing a Tauri AppHandle harness (push_and_notify requires the notification plugin + a window).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: cover the non-connected pairing branch and guard fake timers\n\nAdd a savePairing test for the branch where token save succeeds but the status refresh does not reach the connected view — the button must reset to Save & pair and re-enable.\n\nWrap the fake-timer tests in try/finally so a thrown assertion always restores real timers and cannot leak fake timers into later tests.\n\nAddresses CodeRabbit's two re-review findings on popup.test.ts.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-16T17:18:28+02:00",
          "tree_id": "06ceb2005de47bcf7f66376bb96e3b9d7a57e6e9",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/465ea0b7eed5b97e92cde3f776df14c98f62d180"
        },
        "date": 1781624252309,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1982601,
            "range": "± 88408",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2682959,
            "range": "± 54084",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 324966,
            "range": "± 9690",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "96274db22c8bf3d5b1dbd715610e086eae324afc",
          "message": "feat: native-messaging transport for firefox https-only mode (#387)\n\n* fix: accept firefox null origin on extension bridge handshake\n\nFirefox sends Origin: null for a WebSocket opened from an extension\n\nbackground script (it strips the moz-extension UUID rather than leak it,\n\nBugzilla 1607936 / 1257989), so the bridge handshake was rejected with 403.\n\nAccept the null origin: the origin gate is defense-in-depth only — the real\n\nboundary is the per-frame 256-bit pairing token over a loopback-only listener.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add native-messaging transport for firefox https-only mode\n\nFirefox HTTPS-Only Mode upgrades the extension's ws://127.0.0.1 bridge\n\nconnection to wss://, which the plain loopback bridge can't serve, so the\n\npublished extension always showed app not running. Add a native-messaging\n\ntransport: the browser spawns our own exe in --native-host mode as a stdio\n\nrelay to the running app's loopback ws bridge, immune to the HTTPS-Only upgrade.\n\nThe extension tries connectNative first and falls back to the ws probe when the\n\nhost isn't registered, so Chrome and older apps keep working. Same wire envelope\n\non both transports; the per-frame pairing token flows through unchanged.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: document native-messaging transport in extension readme\n\nPromote the native-messaging section from Future to the active primary\n\ntransport (ws fallback), correct the Firefox Origin: null reality, add the\n\nnativeMessaging permission row, and refresh the stale reviewer test notes.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: disconnect native port on bridge connect failure paths\n\nClose the native-messaging Port before rejecting on the ready-timeout and\n\nbridge.ready{ok:false} paths, so reconnect backoff can't leak spawned\n\nnative-host processes; the onDisconnect path stays a no-op (already closed).\n\nAlso remove the dead moz-extension:// argv check in run_native_host_if_invoked\n\n(Firefox passes the manifest path, already matched) and route register.rs's\n\nHOME read through a new platform::config::home_dir() helper (R4 env-in-platform).\n\nAddresses CodeRabbit review on #387.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-16T19:41:34+02:00",
          "tree_id": "a6b0f228c744d47cc9868412d1a5fd633c876460",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/96274db22c8bf3d5b1dbd715610e086eae324afc"
        },
        "date": 1781632179739,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1879551,
            "range": "± 58491",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2418326,
            "range": "± 49518",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 269728,
            "range": "± 15928",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dfc837d5d87aebf4d60aea34a70ca78b7a8ada97",
          "message": "feat: import selected job from board list views + reset/pairing fixes (#390)\n\n* feat: import the selected job from board search and list views\n\nImports from a board's search/list view (where the selected job id lives in\na query param, e.g. LinkedIn currentJobId, Indeed vjk) previously failed: the\ntab URL is the search shell, not a direct job URL, so the resolver fell back\nto a generic parse of the wrong page.\n\nAdd a centralized canonical_job_url() that maps a recognized list/SPA view URL\nto the canonical single-job URL, and re-resolve that in handle_import for both\nimport modes. Ids are validated before interpolation and the canonical URL\nstill passes the existing normalize + SSRF host guards.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: clear username on factory reset\n\ndefaultPreferences omitted userName, so resetPreferences() (which spreads the\ndefaults over state) left the existing username untouched while clearing every\nother preference. Add userName to the defaults so reset wipes it too.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: settle extension popup status after pairing\n\nAfter saving the pairing token the popup did a single status refresh and then\nrelied on a background status push, which can be missed on a just-woken MV3\nworker, leaving it stuck on the \"looking for the desktop app\" spinner. Poll the\nbridge status until the phase leaves searching (bounded) so the view always\nsettles to the paired state.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: contain status-poll rejection during pairing\n\nrefreshUntilSettled let a send() rejection bubble; a transient MV3\nmessage-channel failure mid-poll (after a successful setToken) hit the\nsavePairing catch and falsely showed \"pairing failed\". Catch it in the loop and\nfall back to the offline view instead.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-16T23:09:59+02:00",
          "tree_id": "cf285ce0fea5a641921946e5afe7ce72dcc8ad06",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/dfc837d5d87aebf4d60aea34a70ca78b7a8ada97"
        },
        "date": 1781644705780,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1899144,
            "range": "± 41447",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2480023,
            "range": "± 19682",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 283832,
            "range": "± 2301",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "51081940+saeedkolivand@users.noreply.github.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "44aa3472fa11f43c991e22f8f83134b4ba98db3e",
          "message": "fix(security): restrict system_open_external to http and https schemes (#435)\n\nReject non-http(s) URL schemes in system_open_external before handing the\nrenderer-supplied URL to the OS opener, closing a file:/custom-scheme handler\nlaunch vector. Narrow the CSP connect-src loopback wildcards to the exact ports\nin use: Ollama (11434) plus the extension-bridge WS range (47615-47620).\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-19T13:36:30+02:00",
          "tree_id": "441c21e17fef0d9c458f8eacbc2637412241b0bd",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/44aa3472fa11f43c991e22f8f83134b4ba98db3e"
        },
        "date": 1781870149400,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1964986,
            "range": "± 54296",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2637908,
            "range": "± 69629",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 307226,
            "range": "± 4408",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}