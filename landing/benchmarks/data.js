window.BENCHMARK_DATA = {
  "lastUpdate": 1781450723794,
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
      }
    ]
  }
}