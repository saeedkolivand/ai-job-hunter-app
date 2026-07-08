window.BENCHMARK_DATA = {
  "lastUpdate": 1783536470757,
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
          "message": "fix: backend error-path hardening + dead-code cleanup and refactors (#377)\n\n* docs: update extension-bridge origin validation for firefox uuid shape\n\nFirefox assigns per-install random UUIDs for moz-extension:// origins, never\nthe AMO gecko id, so the bridge now validates Firefox origins by UUID shape\n(8-4-4-4-12 hex pattern) instead of a fixed gecko id that never appears.\nChrome stays pinned to the stable CWS id. Updated README and ADR-015 to\nclarify the defense-in-depth origin check vs. the per-frame token boundary,\nand documented the Chrome CWS id placeholder as a release-blocking checklist.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: remove safe-tier dead code (unused deps + dead exports)\n\nAudited with knip + codegraph (Rust clean per cargo machete/clippy).\nRemoves only provably-unreferenced, zero-behavior items:\n\n- deps: clsx, jspdf, tailwind-merge (apps/desktop) — @ajh/ui cn() owns\n  clsx/tailwind-merge; PDF export is Rust/Typst. zod (apps/extension) —\n  the bundle is deliberately zod-free (hand-written guards).\n- dead hooks/consts: useLanguage, useResume (preferences-store),\n  useSetPerformanceMode (use-system), useDataCapability\n  (CapabilityProvider), TAILOR_TEMPLATE/TAILOR_ATS_MODE\n  (useTailorGeneration), ANALYSIS_STRATEGY (prompts truncation, an alias\n  of the live LARGE_MODEL_STRATEGY).\n\ntsc --noEmit clean, lint:strict green, knip findings cleared with no new\norphans, full test suite (1619) passing.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: ignore root-level extension zips and drop stale test mock keys\n\n- .gitignore: ignore packaged ai-job-hunter-*.zip artifacts dropped at the\n  repo root by the package/release step (the store-packages/ dir was already\n  ignored; the release step also emits a root-level zip).\n- ApplyPage.test.tsx: remove the vi.mock keys TAILOR_TEMPLATE/TAILOR_ATS_MODE\n  left inert after those constants were deleted in this branch.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: harden backend error paths and hoist per-call regex compilation\n\nAddresses the High + cheap-Medium findings from the whole-backend Rust\ncode-quality audit. Security-reviewed (credential/auth path: fail-closed\non a degraded keyring is structurally guaranteed; no secret leakage in\nthe new error/log strings; LinkedIn wire output byte-identical).\n\nCorrectness (panics / silent failures -> propagated AppError):\n- credentials::init_keyring no longer .expect()-panics at startup; returns\n  AppResult and lib.rs logs a warning + degrades (credential ops then fail\n  closed via AppError instead of crashing the app on an unavailable keyring).\n- credentials::save_meta no longer swallows write errors (.ok()); returns\n  AppResult through set()/remove(); create_dir_all failure is logged.\n- linkedin client get_default_headers: static headers use\n  HeaderValue::from_static; the runtime CSRF token / cookie now propagate a\n  parse error instead of .parse().unwrap() panicking on a malformed token.\n\nPerf / DRY (no behavior change):\n- hoisted per-call regex compilation to module-level LazyLock statics in\n  scraping/http (14), scrape_url, germantechjobs; CSS Selector statics in\n  glassdoor/xing/indeed/linkedin api_client.\n- unified the duplicated Personio XML extraction into\n  personio::parse_xml_feed (shared regex set + per-position DTO; each caller\n  keeps its distinct JobPosting assembly).\n- de-duplicated the Chrome user-agent literal onto net::http::DEFAULT_UA.\n\ncargo check/clippy --lib clean; per-module tests + architecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: clarify keyring degradation and fail-closed guarantee in security rules\n\nUpdated security-rules.md to document that keyring init degrades gracefully\n(warns instead of panic) when system keyring service is unavailable, with\nsubsequent credential reads/writes erroring rather than falling back to\nplaintext—fail-closed behavior guaranteed by keyring-core v1 semantics.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: migrate scraping/export/validate/browser off anyhow to apperror\n\nReplaces anyhow::Result / Box<dyn Error> with the centralized AppResult /\nAppError across the scraping HTTP client, LinkedIn client, validate, and\nexport (model_docx, pdf) layers, plus browser/mod.rs (M3/M4/M5 from the\nbackend code-quality audit). Error categories are now preserved to the\ncommand boundary instead of being flattened into an opaque string.\n\nVariant mapping: transport/status -> Network; body/document/JSON decode and\nTypst render -> Parse; runtime header-build config -> Config; size limits ->\nValidation; cancellation -> Cancelled. Context messages are folded into the\nerror so no diagnostic is lost.\n\nSecurity: every changed error message is structural (library error string or\na fixed phrase) - no resume/cover-letter text, secrets, or file contents are\ninterpolated into any error or log line.\n\nanyhow is retained (29 other modules still use it; error.rs keeps the\nFrom<anyhow::Error> bridge). LinkedInHttpClient::new() still .expect()-panics\non client build (no anyhow there to migrate) - left for a follow-up.\n\ncargo check/clippy --lib clean; scraping/validate/export/browser module tests\nplus architecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: centralize epoch-ms timestamp casts in db helpers\n\nReplaces ~40 scattered, unannotated u64<->i64 millisecond-timestamp casts\nat the SQLite row boundary (applications, jobs, ai_generations, referrals,\ndocuments, autopilot_scheduler) with a documented helper pair ts_to_db /\nts_from_db in the L0 db module (M10 from the backend audit).\n\nThe helpers use lossless saturating try_into and are byte-identical to the\nold `as` casts over the real domain (epoch-ms stays below i64::MAX until\n~year 292 million; stored values are always non-negative), so existing rows\nround-trip unchanged. Non-timestamp casts (counts, bool flags, vector dims,\ntimezone offsets, the u128->u64 clock source) were intentionally left alone.\n\ncargo check/clippy --lib clean; per-module tests + 3 new db::ts_tests +\narchitecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: drop false-positive dead_code allows and document the rest\n\nM1/M2 from the backend audit. Most of the broad #![allow(dead_code)]\nsuppressions were hiding nothing:\n\n- scraping/http and the LinkedIn client/api_client/rate_limiter modules\n  have live callers (board scrapers + the linkedin registry scraper), so\n  the blanket allows were pure false positives -> removed.\n- ipc_contracts/*: replaced the scattered per-struct #[allow(dead_code)]\n  with one documented file-level allow (serde DTOs mirroring the TS\n  contract; fields are (de)serialized across IPC, never read in Rust).\n- browser/mod.rs (BrowserController, chromiumoxide CDP automation) is\n  genuinely dormant (zero live callers, never wired into SCRAPERS). Kept\n  with a documented allow + rationale; flagged as a deletion candidate\n  pending a product decision (delete vs reserve for JS-rendered scrapers).\n\ncargo check/clippy --lib clean; module tests + architecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* style: rustfmt migration drift and assert http test reads response\n\nFull-target clippy (cargo clippy --all-targets --all-features -- -D warnings,\nthe pre-push gate command) caught two things the agents' --lib checks missed\nbecause --lib does not compile test code:\n\n- scraping/http/test.rs: TestResponse.message was deserialized but never read\n  (dead_code). Strengthened the assertion to check the parsed value, which\n  reads the field and verifies deserialization rather than just is_ok().\n- rustfmt drift introduced by the anyhow->AppError migration in credentials,\n  indeed, linkedin api_client/client (+ its test) - ran cargo fmt --all.\n\ncargo clippy --all-targets --all-features -- -D warnings clean; cargo fmt\n--check clean; architecture guard 11/11.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: reflect anyhow to apperror migrations and browser subsystem status\n\nUpdate architecture docs to note error-layer conversions in scraping/http,\nscraping/linkedin/client, validate, export/{model_docx,pdf}, browser; keyring\ndegradation now explicit; browser/BrowserController is dormant/reserved. Refresh\nlast-updated dates. Persist 4 architecture lessons: anyhow-boundary bridging,\nkeyring fault-tolerance, ts_to_db/ts_from_db centralization, Personio DRY.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: remove dead browser-automation module\n\nDeletes apps/desktop/src-tauri/src/browser/ (BrowserController, a chromiumoxide\nCDP wrapper). It was dormant: zero live callers, never wired into the SCRAPERS\nregistry - a never-enabled attempt at browser-based scraping. Today scraping is\nHTTP (reqwest via scraping/http + the LinkedIn http/api clients); Chromium is\nused only for the manual-login + cookie-capture flow, which lives in\nscraping/board_login (boards_connect / importCookies).\n\nchromiumoxide is retained - board_login, glassdoor, and platform/chrome still\nuse it (cargo machete confirms it is not unused). Also drops the now-stale\n\"browser\" entry from the tests/architecture.rs L0 classification so the\nno-dead-allowlist-entry guards pass.\n\ncargo check/clippy --all-targets --all-features -- -D warnings clean; cargo fmt\n--check clean; architecture guard 11/11; cargo machete clean.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* chore: regenerate ipc_contracts to fix gen:ipc drift\n\nStage 3 of this branch hand-edited the @generated ipc_contracts/*.rs files\n(consolidating per-struct #[allow(dead_code)] into a file-level one). Those\nfiles are produced by `pnpm gen:ipc`, so the committed form no longer matched\nthe generator and CI's gen:ipc:check failed on 10 files. The pre-push hook\ndoes not run that check, so it slipped through.\n\nRegenerated to the canonical form (per-struct #[allow(dead_code)], which still\nsuppresses the serde-DTO dead_code under clippy --all-targets -- -D warnings).\nThis reverts only the cosmetic ipc_contracts allow-consolidation; the rest of\nthe dead_code cleanup (removing genuinely-false-positive allows from\nscraping/http and the linkedin modules) stands. Generated files must not be\nhand-edited - to change their shape, change packages/shared/scripts/\ngen-ipc-rust.ts instead.\n\ngen:ipc:check up to date; clippy --all-targets --all-features -- -D warnings\nclean; cargo fmt --check clean.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* build: add gen:ipc:check to the pre-push hook\n\nThe pre-push gate ran eslint + prettier but not gen:ipc:check, so a hand-edit\nto a @generated ipc_contracts/*.rs file passed the local push and only failed\nin CI's Lint & Format job. Run the same IPC codegen drift check locally so this\nclass is caught before push.\n\n(check:landing-drift is the other Lint & Format check still missing from the\nhook - left for a follow-up.)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* build: path-filter the pre-push gate to changed files\n\nMirrors CI's dorny/paths-filter locally: the hook reads the pushed refs from\nstdin, diffs the push range, and runs only the gate groups whose paths changed\n(frontend vs rust, matching the filters in ci-pipeline.yml). A docs-only or\nJS-only push now skips the cargo steps entirely - so it can't hit the\najh-tauri.exe build lock - and a Rust-only push skips the JS gate.\n\nSafety: any uncertainty (no push range / detached) falls back to the FULL gate,\ncross-cutting paths (packages/**, .github/**) trip both groups, gen:ipc:check\nruns when node OR rust changed, and PREPUSH_FULL=1 forces everything. CI stays\nthe authoritative gate. Decision logic tested across docs/rust/node/mixed/\ncross-cutting scenarios.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ci: surface test failures with dorny/test-reporter\n\nAdds JUnit output and two dorny/test-reporter steps so failing tests appear\nas a GitHub check + inline PR annotations instead of buried in CI logs:\n\n- nextest: a [profile.ci.junit] report (cargo-llvm-cov nextest already runs\n  the ci profile) -> target/llvm-cov-target/nextest/ci/junit.xml.\n- vitest: --reporter=junit alongside the default reporter (the default text\n  still feeds the coverage-summary grep) -> reports/vitest.junit.xml.\n- two report steps (if: always, fail-on-error: false) pinned to\n  dorny/test-reporter@a43b3a5f # v3.0.0; the Rust path is globbed to handle\n  cargo-llvm-cov's target-dir redirection.\n\nchecks: write is scoped to the tests job; CI runs on pull_request (not\npull_request_target), so fork tokens stay read-only - security-reviewed safe.\nThe \"Check for Test Failures\" step remains the sole pass/fail gate; the\nreporters are purely additive.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* build: make pre-push gate non-mutating and lockfile-strict\n\nAddresses CodeRabbit review on PR #377:\n- format step ran `pnpm format` (prettier --write) which silently rewrites the\n  working tree instead of failing -> switched to `pnpm format:check`\n  (prettier --check), so committed format drift actually blocks the push,\n  matching CI's format-guard.\n- cargo check/test/clippy omitted --locked while CI runs with it -> added\n  --locked so a lockfile drift fails locally instead of only in CI.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: remove drift-prone module census and deleted browser references\n\nReplace module list + count in architecture.md with thin pointer to\nerror.rs + enforcing test r6_no_stringly_result. Remove reference to\ndormant/deleted browser module in automation-domain.md; clarify Chromium\nis used only for board_login cookie capture, not scraping transport.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* build: add landing-drift to the pre-push gate with a landing filter group\n\nCloses the last CI-parity gap CodeRabbit flagged: CI's lint-format job runs\ncheck:landing-drift but the hook did not. Adds a LANDING path-filter group\n(landing/**) so a landing-only push runs just the drift check, and runs\ncheck:landing-drift whenever node/rust/landing paths change.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: return a result from the linkedin http client constructor\n\nnew() built its reqwest client with .expect(\"failed to build LinkedIn HTTP\nclient\"), so a TLS/config init failure aborted the process. It now returns\nAppResult<Self> (AppError::Network); the live caller (boards/linkedin search)\npropagates via ?, and the 5 test callers .expect() it. Completes the\nanyhow->AppError fallibility pass for the LinkedIn client.\n\ncargo check/clippy --all-targets --all-features --locked clean; linkedin\nmodule tests + architecture guard green.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
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
          "id": "62f3ff46cadd417efe4cc3664ce45dcf52238cd5",
          "message": "fix(scraping): close glassdoor browser on error and share linkedin rate limiter (#436)\n\n* fix(scraping): close glassdoor browser on error and share linkedin rate limiter\n\n- glassdoor: store first-page nav error and break to labeled loop so\n  browser.close() and handle.await always run before propagating; also\n  close browser on new_page failure\n- linkedin rate_limiter: promote to process-wide static LazyLock so\n  concurrent scrapes share one window instead of each getting a fresh one;\n  fix thundering-herd in wait_for_slot by re-checking under lock after\n  sleep (loop until slot is genuinely free)\n- linkedin client: store &'static RateLimiter instead of owned value\n- workday: hoist Regex::new to static LazyLock to avoid recompile per call\n- linkedin search_paginated: fire on_progress(fraction) after each page\n  instead of only at completion\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n* fix(scraping): preserve first-page error over glassdoor teardown failure\n\nWhen first_page_err is set, the original browser.close().await? could\npreempt the captured root-cause error with a secondary teardown failure.\nCapture close_res first, await the handle, then return first_page_err if\nset — suppressing any close error — otherwise propagate close_res normally.\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Sonnet 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-06-19T19:05:11+02:00",
          "tree_id": "ce67e5fe6ba170068c90f1cc86e0a6acd8de974e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/62f3ff46cadd417efe4cc3664ce45dcf52238cd5"
        },
        "date": 1781889837117,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1972551,
            "range": "± 39130",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2602912,
            "range": "± 59410",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 311436,
            "range": "± 8396",
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
          "id": "67a6a6b83d386755e40a961dea80691de8a95d9a",
          "message": "refactor: dedupe now_ms, job-id, and renderer doc helpers (#438)\n\n* refactor: dedupe now_ms, job-id, and renderer doc helpers\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n* refactor: address coderabbit review on dedupe pr\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Sonnet 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-06-19T19:47:54+02:00",
          "tree_id": "8de8a522d1180807a8554c2af2d56d2115fba04a",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/67a6a6b83d386755e40a961dea80691de8a95d9a"
        },
        "date": 1781892409357,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1188965,
            "range": "± 108810",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1678295,
            "range": "± 85368",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 206428,
            "range": "± 16075",
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
          "id": "2dd85c20dddc3b0b835f7cd3ce36452fbba0e9fa",
          "message": "fix(job-match): align evidence grounding with scorer and add guidance framing (#442)\n\n* fix(job-match): align evidence grounding with scorer and add guidance framing\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n* fix(job-match): normalize punctuation tokens and harden language handling\n\n- strip boundary punctuation from résumé tokens before alias lookup so\n  trailing commas and surrounding parens (e.g. \"JavaScript,\" \"(Kubernetes)\")\n  no longer cause false ABSENT grounding labels; intra-token chars like\n  c++ and node.js are preserved\n- add punctuation-edge synonym tests covering both cases\n- soften divergent_language_pair integration test precondition from\n  assert_eq!(old_cov, 0.0) to assert!(old_cov < new_cov) so it is not\n  coupled to the exact german snowball stemmer output\n- guard cjk and other non-latin-script jd languages from english stemming\n  by treating them as divergent (normalized-only) in jd_matches_resume_locale\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n* fix(renderer): surface match-score guidance disclaimer in jobs ui (#447)\n\nadds a keyboard-accessible HoverPopover info trigger next to the MatchBand\nin RowMatchScore so the guidance disclaimer (jobs.scoreGuidance) is\ndiscoverable without cluttering every list row\n\nCo-authored-by: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n* fix(security): replace redos-prone boundary-trim regex with linear scan\n\nThe `stripBoundaryPunctuation` helper in emphasis.ts used an alternation\nregex (`/^[...]+|[...]+$/g`) that backtracks polynomially on a long run\nof boundary punctuation — a real redos vector since normalizeTerm calls\nit on every whitespace-split token of uncontrolled résumé/jd input.\n\nReplace with a linear O(n) char-by-char scan using a Set lookup (charAt\nto satisfy noUncheckedIndexedAccess). Behavior is identical: only\nleading/trailing boundary punct is stripped; internal chars like c++,\nnode.js, and c# are preserved. Adds a regression test asserting a\n100 000-char punctuation token completes instantly and returns ''.\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n* test(jobs): assert guidance popover content is revealed on interaction\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Sonnet 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-06-19T22:35:41+02:00",
          "tree_id": "29bfe460d52d89b97aa9e5e2d18a9fcb3434fd76",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/2dd85c20dddc3b0b835f7cd3ce36452fbba0e9fa"
        },
        "date": 1781901841880,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1918207,
            "range": "± 56153",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2540225,
            "range": "± 34275",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 297712,
            "range": "± 3702",
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
          "id": "b9817a9fb2bf2dfcac863b9343d30863504b05a1",
          "message": "fix(scraping): repair drifted board scrapers and add live smoke tests (#450)\n\n* fix(scraping): repair drifted board scrapers and add live smoke tests\n\nLive-verified all 17 guest/company-scoped boards against their real endpoints\nand repaired the ones that had drifted:\n\n- arbeitsagentur: detail API renamed fields (refnr -> referenznummer, etc.); made\n  DetailResp fields Option so one rename no longer drops all jobs; use fetch_text\n  so the X-API-Key header survives; detail fetch is now opportunistic.\n- germantechjobs: site dropped __NEXT_DATA__ (now a client SPA) -> switched to the\n  RSS feed via feed_rs; description included in the keyword filter; salary-bracket\n  strip narrowed to real [..] brackets only.\n- ycombinator: Algolia key was revoked -> switched to the HN Firebase job feed\n  (credential-free, stable); company extraction matches the full \"(YC \" prefix only.\n- personio: feed moved to /xml; description regex scoped to the <jobDescriptions>\n  block.\n\nHarness: one #[ignore = \"live network\"] smoke test per testable board (run with\ncargo test live_search_returns_results -- --ignored) for future drift detection.\nworkday (Cloudflare bot management) and stepstone (IP-based bot block) can't be\nexercised from a programmatic client and are documented in their tests, not fixed.\n\nhttp: MAX_BYTES stays 8MB globally; FetchOptions gains an opt-in max_bytes so only\ngermantechjobs' ~10MB RSS lifts the cap, preserving the shared OOM guard.\n\nFollow-up (separate PR): fetch_json overwrites caller headers, masking auth errors\nas empty results.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix(scraping): bound live tests with timeouts and tighten board parsers\n\nAddress CodeRabbit review on #450:\n- All live #[ignore] smoke tests wrap search() in a 30s tokio timeout so a\n  stalled endpoint fails fast instead of hanging the run.\n- personio: legacy description fallback now scopes to the singular\n  <jobDescription> block instead of the whole position (no sibling <value> leak).\n- ycombinator: company parsing extracted to a pub(crate) parse_company() helper\n  (now unit-tested directly); empty prefix (title starts with \"(YC \") falls back\n  to the author handle instead of an empty company.\n- germantechjobs: dropped the redundant desc_hay containment check (haystack\n  already includes the description); added a SALARY_BRACKET_RE strip test.\n\nDeferred to a follow-up PR: http fetch buffers the body via text() before the\nsize check, so the byte cap only rejects post-buffer (content-length pre-check\nremains the practical guard) — bundling with the fetch_json header fix.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-20T15:11:05+02:00",
          "tree_id": "d78464349d9ab420feb534b6cfe14dfa9d4d30ca",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b9817a9fb2bf2dfcac863b9343d30863504b05a1"
        },
        "date": 1781961561416,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1909940,
            "range": "± 49704",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2561350,
            "range": "± 118419",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 290966,
            "range": "± 12979",
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
          "id": "9b40e7bdbdbe5d2a2672694d2c9d3aad664a1fea",
          "message": "fix(scraping): preserve caller headers and stream-cap http responses (#454)\n\n* fix(scraping): preserve caller headers and stream-cap http responses\n\nTwo hardening fixes to the shared scraper HTTP helper (surfaced during the board\nverification sweep):\n\n- fetch_json no longer drops caller-supplied headers. It previously replaced\n  opts.headers with only `accept: application/json`, silently dropping headers\n  like arbeitsagentur's X-API-Key — which made an auth failure look like \"empty\n  results\". It now merges, adding the JSON accept only when the caller didn't set\n  one. arbeitsagentur moves back to fetch_json (the fetch_text workaround is gone).\n- fetch_text now enforces the size cap while streaming. It read the whole body via\n  Response::text() before checking the cap, so a response that omits/lies about\n  Content-Length could OOM. It now reads bytes_stream() and aborts the moment the\n  buffer exceeds the cap (peak ~cap + one chunk), keeping the content-length\n  pre-check. Charset is decoded via encoding_rs (already transitive via reqwest),\n  honoring the Content-Type charset with a UTF-8 fallback — German/legacy\n  encodings round-trip (covered by utf-8, iso-8859-1, and no-charset tests).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix(scraping): tighten http charset parsing and size-cap boundary\n\nAddress CodeRabbit review on #454:\n- Charset parameter is matched case-insensitively and surrounding quotes are\n  stripped, so Content-Type like `Charset=\"ISO-8859-1\"` decodes correctly instead\n  of silently falling back to UTF-8.\n- The streamed size cap is checked before extending the buffer\n  (buf.len() + chunk.len() > cap), so a single large chunk can't momentarily\n  allocate past the cap. Boundary stays consistent with the content-length\n  pre-check.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-20T16:10:41+02:00",
          "tree_id": "f4fc9315aa0a30f2c8094ed644b209cf3674d078",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9b40e7bdbdbe5d2a2672694d2c9d3aad664a1fea"
        },
        "date": 1781965739387,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1874924,
            "range": "± 56181",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2510906,
            "range": "± 14777",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 292370,
            "range": "± 1629",
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
          "id": "92bf97973d9e2843c2940ae68cac7df98cbf4149",
          "message": "feat(scraping): scrape multiple job boards per run with rate-limit hardening (#455)\n\n* fix(scraping): preserve caller headers and stream-cap http responses\n\nTwo hardening fixes to the shared scraper HTTP helper (surfaced during the board\nverification sweep):\n\n- fetch_json no longer drops caller-supplied headers. It previously replaced\n  opts.headers with only `accept: application/json`, silently dropping headers\n  like arbeitsagentur's X-API-Key — which made an auth failure look like \"empty\n  results\". It now merges, adding the JSON accept only when the caller didn't set\n  one. arbeitsagentur moves back to fetch_json (the fetch_text workaround is gone).\n- fetch_text now enforces the size cap while streaming. It read the whole body via\n  Response::text() before checking the cap, so a response that omits/lies about\n  Content-Length could OOM. It now reads bytes_stream() and aborts the moment the\n  buffer exceeds the cap (peak ~cap + one chunk), keeping the content-length\n  pre-check. Charset is decoded via encoding_rs (already transitive via reqwest),\n  honoring the Content-Type charset with a UTF-8 fallback — German/legacy\n  encodings round-trip (covered by utf-8, iso-8859-1, and no-charset tests).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix(scraping): tighten http charset parsing and size-cap boundary\n\nAddress CodeRabbit review on #454:\n- Charset parameter is matched case-insensitively and surrounding quotes are\n  stripped, so Content-Type like `Charset=\"ISO-8859-1\"` decodes correctly instead\n  of silently falling back to UTF-8.\n- The streamed size cap is checked before extending the buffer\n  (buf.len() + chunk.len() > cap), so a single large chunk can't momentarily\n  allocate past the cap. Boundary stays consistent with the content-length\n  pre-check.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat(scraping): scrape multiple boards per run with rate-limit hardening\n\nReplace the one-board-per-call scrape path with a bounded-concurrent\nmulti-board engine shared by the Jobs page and Autopilot.\n\n- engine: run_one/run_boards/scrape_boards fan out up to 3 boards (browser\n  boards serialized to 1) under a parent/child cancellation-token tree; a\n  per-board amount cap stops only its own board, a user cancel stops all.\n  Partial success returns per-board BoardScrapeSummary.\n- http: shared per-host RateLimiter registry (promoted from LinkedIn's) plus\n  429/503 Retry-After backoff with jitter in fetch_text.\n- contracts: board -> boards (max 6) for scrape + autopilot; command renamed\n  scrape_boards; AutopilotTarget keeps back-compat via serde alias.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01FzECrMJGzH9gWt61v9v1Qi\n\n* fix(scraping): bound board batch size and harden http retry pacing\n\nAddress review findings on the multi-board scrape path:\n\n- cap+dedupe the boards batch to 6 inside scrape_boards (server-side\n  defense-in-depth; the Zod max is renderer-only, so a crafted IPC payload\n  or tampered autopilots.json could otherwise fan out unbounded scrapes\n  against the user's own authenticated sessions - CWE-770).\n- saturating_mul the Retry-After seconds so a hostile header can't overflow.\n- gate every fetch_text attempt through the per-host limiter (not just the\n  first) and record each completed send, so 429/503 retries stay paced.\n- clarify the AutopilotTarget back-compat deserializer doc comment.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01FzECrMJGzH9gWt61v9v1Qi\n\n* feat(jobs): select multiple job boards to scrape in one run\n\nRenderer half of multi-board scraping, wired to the new scrape_boards\ncommand.\n\n- ScrapeForm board picker becomes a multi-select toggle group with\n  select-all / clear and a count badge; ScrapeFormState.board -> boards.\n- per-board auth is encapsulated in a BoardConnectChip so each selected\n  login-board shows its own connect state without hooks-in-a-loop.\n- JobsPage surfaces the per-board completion summary, keeping partial\n  success (e.g. \"5 of 6 boards - linkedin failed\").\n- Autopilot wizard target step selects multiple boards; schema, wizard\n  state and steps follow boards: string[].\n- en + de translations for the new strings.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01FzECrMJGzH9gWt61v9v1Qi\n\n* fix(jobs): accessible keyboard nav and partial-success cues for board picker\n\nAddress review findings on the multi-select board picker:\n\n- add makeMultiSelectKeyHandler: arrow keys move a single roving focus\n  (one tabIndex=0) without toggling; Space/Enter toggles the focused board.\n  Apply to the Jobs picker and the autopilot wizard target step; drop the\n  invalid aria-multiselectable and redundant role=\"button\".\n- BoardConnectChip: label the disconnect button while pending; raise the\n  \"needs login\" row contrast to meet AA.\n- footer shows partial success (some boards failed) in amber, reserving\n  green for full success; failure list uses localized board names.\n- announce the selection count (aria-live) and use numeric-count plurals;\n  larger select-all/clear touch targets.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01FzECrMJGzH9gWt61v9v1Qi\n\n* test: cover multi-board engine path, board picker, and per-board auth\n\n- engine: exercise FakeScraper::browser through run_boards so the\n  browser-semaphore path is asserted to collect results.\n- renderer: useBoardStatuses routing/aggregation, BoardConnectChip\n  connect/disconnect + accessible labels (incl. LinkedIn routing),\n  ScrapeForm multi-select toggle/select-all/clear/last-board guard, and\n  the JobsPage partial-failure summary (localized names, ok stays true).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01FzECrMJGzH9gWt61v9v1Qi\n\n* test(scraping): close coverage gaps on retry-after, batch cap, and cancel gate\n\n- assert the Retry-After clamp survives a hostile huge header.\n- exercise the boards dedupe+cap through the public scrape_boards entry\n  point, not a re-implementation; partial-success-under-cancel is verified\n  the same way via a test-only resolver seam.\n- cover the stepstone per-host limiter override.\n- tighten the ScrapeForm board-toggle assertions to exact array shape and\n  drive the keyboard handler via a real keydown event.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01FzECrMJGzH9gWt61v9v1Qi\n\n* chore(jobs): remove auth-mode badge and hint superseded by board-connect chip\n\nThe single-board auth affordance is gone now that each selected\nlogin-board renders its own per-board connect chip; drop the unused\ncomponents and their orphaned jobs.* translation keys.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01FzECrMJGzH9gWt61v9v1Qi\n\n* docs: sync multi-board scraping references and rate-limit defense docs\n\n- automation-domain.md: update scraping engine notes to cover multi-board fan-out (≤3 concurrent), per-host rate limiting with 429/Retry-After backoff, and MAX_BOARDS_PER_BATCH = 6 server-side cap\n- anti-abuse-limits.md: add multi-board batch limit (CWE-770 defense) and update scrape_board → scrape_boards command examples\n- PATTERNS.md: note multi-board batch cap in anti-abuse rate+concurrency ownership line\n- landing/how-it-works.html: update all scrape_board references to scrape_boards, reflect multi-board UI flow (up to 6 boards, ≤3 concurrent)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01FzECrMJGzH9gWt61v9v1Qi\n\n* test(jobs): satisfy strict typecheck in multi-board test files\n\nvitest transpiles without type-checking, so these slipped past the test\nrun but failed tsc --noEmit in the pre-push gate.\n\n- use the vitest v4 single-arg vi.fn<Fn>() signature\n- non-null assert mock.calls[0] under noUncheckedIndexedAccess\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01FzECrMJGzH9gWt61v9v1Qi\n\n* test(jobs): drop non-null assertions in scrape test mocks\n\nMirror the lint-safe mock.calls optional-chaining pattern already used in\nScrapeForm.interaction.test.tsx so eslint strict (--max-warnings 0) passes.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* style(scraping): apply cargo fmt to multi-board scrape code\n\nThe branch authored on web wasn't rustfmt-formatted; CI cargo fmt --check\nfailed. No logic change.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): preserve board input order, share browser semaphore, harden cancel tokens\n\nAddress CodeRabbit findings on PR #455 (each verified against the code):\n\n- engine: buffered(3) over buffer_unordered(3) to honor the input-order contract\n- engine: browser semaphore is now a ScraperEngine field, serializing browser\n  boards across concurrent scrape jobs, not just within a single batch\n- engine/commands: register the cancel token before spawn and only remove\n  internally-minted tokens, so a fast cancel is not dropped and autopilot's\n  pre-registered token survives the scrape\n- engine: a cancelled run that recovered zero items now returns Err, not Ok\n- engine: reject an empty boards list at the scrape entrypoint\n- rate_limiter: document reset() as sync-only (blocking_lock panics in async)\n- ui: guard non-array job.completed boards; no-op roving-tabindex on empty groups\n- tests: input-order, cross-job browser serialization, empty-boards, cancelled\n  empty-Ok, pre-registered-token survival; per-item board id, malformed payload,\n  count=0, and an assertive waitFor\n- docs: replace copied limits with symbol pointers; LinkedIn casing; owner column\n\nRejected as by-design (replied on the PR): autopilot boards relaxed validator\n(future-proof registry growth) and semantic status colors (lint bans only hex).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): connect indeed, glassdoor and xing via the registered login command\n\nboards.ts invoked boards_connect/boards_disconnect, which are not registered Tauri commands, so the generic connect chips silently failed (invoke rejected with command-not-found).\nLinkedIn worked only because its namespace hardcodes the real boards_login_with_browser. Use the registered boards_login_with_browser/boards_logout and lock the channel names in namespaces.test.ts.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ui(jobs): lead the board catalog with linkedin, glassdoor, xing and indeed\n\nSCRAPERS is the catalog display order; pull the major browser-auth boards to the front so they lead the Jobs board picker. Catalog tests are membership-based, so ordering is unaffected.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: correct readme accuracy drift across the repo\n\n- root: 18+ -> 20 boards; split UI (en/de) vs generation (11 langs); browser auto-detect (no Settings -> Scraping); performance is its own tab; 21 -> 23 agents\n- apps/desktop: contracts/ and tauri-client/ are directories; ai_provider lives under commands/; motion tokens import from @ajh/ui\n- fonts: real calibri filenames + a vendored Typst fonts table; extension: firefox published on AMO; store-assets: 6 raw shots at 360x2\n- prompts: exported vs internal folders (+builder); corpus: required-key column; knowledge: list event-system/notification-center/ui-theming-accent\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: correct agent fleet count to 23 on the landing page\n\nagent-system.html hardcoded 21 (author+critic only) in the hero, meta tags,\nand footer while its own roster renders 23 (9 authors + 12 critics + 2\ncross-cutting). Align with CLAUDE.md / README / .claude/agents.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: list only the compiled fonts in the fonts readme\n\nThe vendored-fonts table wrongly listed jetbrains_mono and playfair_display as\ncompiled via include_bytes! and omitted the four Carlito faces. world.rs compiles\nexactly 11 (carlito x4, inter x2, source_serif4 x3, manrope x2); the other two\nfamilies are fetched by download-fonts.ps1 but not bundled — now noted as such.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-06-20T22:08:37+02:00",
          "tree_id": "db087ea2b9b7647a67acc8088b9b6477f634310d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/92bf97973d9e2843c2940ae68cac7df98cbf4149"
        },
        "date": 1781986622788,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1864571,
            "range": "± 41876",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2595251,
            "range": "± 62577",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 279208,
            "range": "± 4607",
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
          "id": "b9e90be42edb32c5f726378a68d646015a05804b",
          "message": "fix(jobs): list glassdoor in the board picker (#456)\n\nGlassdoor was listed()->false (hidden) because it predated the board login\nwiring. The connect flow merged in #455 supplies that, so surface it as a\nbest-effort board. It stays anonymous (frequently bot-blocked) — the browser\nscraper does not yet load the saved session, so login is not wired into it.\nCatalog test updated: glassdoor listed, 20 boards listed.\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-20T22:27:48+02:00",
          "tree_id": "866e5676f190c689242a96111c296db8ce03bad8",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b9e90be42edb32c5f726378a68d646015a05804b"
        },
        "date": 1781987742676,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1878992,
            "range": "± 44146",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2499567,
            "range": "± 4147",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 291555,
            "range": "± 1607",
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
          "id": "0e3535f2642266a974b5610f16adcc07371276e3",
          "message": "feat(jobs): repair broken job boards and gate logged-out scraping (#462)\n\n* fix(scraping): repair german tech jobs feed and rewrite the parser\n\nThe /rss feed returns 403; switch to /job_feed.xml, a custom\njobs/job XML schema. Replace feed_rs with a regex-per-block parser\nextracting company, location, salary, and DD.MM.YYYY dates.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): skip logged-out required boards and add a sign-in prompt\n\nRequired boards with no session are now skipped before any\nrequest and reported via BoardScrapeSummary.skipped='needs-login',\nclosing the renderer-only #458 gate on autopilot and IPC paths.\n\nGlassdoor moves from Guest to Required and reuses its persisted\nlogin profile; the renderer adds a sign-in prompt for logged-out\nrequired boards and a sticky skipped-board notification.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs(scraping): correct endpoint recon and document auth skip\n\nCorrect the German Tech Jobs recommendation (was wrongly 'parser\nunchanged') and reframe Indeed/Xing as HTTP, login-gated boards\nrather than fragile-selector scrapers.\n\nMark Glassdoor best-effort-with-login, document the skipped\ncontract, and note the chromiumoxide WS warning is benign on the\npinned version.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): gate required boards on a valid auth status, not just cookies\n\nThe required-board skip gate only checked for empty cookies or a stale\nsession. session_is_stale returns false when the auth-status file is\nmissing, unreadable, has connected:false, or lacks connected_at, so a\nboard with leftover cookies but no usable session still ran. Also skip\nwhen session_age_ms is None.\n\nGlassdoor: create the Chrome profile dir with tokio::fs inside the async\nsearch instead of blocking std::fs.\n\nAdd an engine regression test for the no-valid-status branch and drop\ndead setup in the stale-session test.\n\nResolves CodeRabbit review findings on the pull request.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-21T02:55:25+02:00",
          "tree_id": "6241b06cc02793f3d76a4a48b92d17a16e5fe294",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/0e3535f2642266a974b5610f16adcc07371276e3"
        },
        "date": 1782004462186,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1939105,
            "range": "± 65637",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2555892,
            "range": "± 23563",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 293823,
            "range": "± 4273",
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
          "id": "06c5617a1aaf505621fb0a6767f0c8c56cf438e5",
          "message": "feat(scraping): add company identifier for company-scoped boards (#464)\n\nApplicant-tracking boards (greenhouse, lever, ashby, recruitee, personio,\nsmartrecruiters) have no global keyword search — their public APIs require a\ncompany/board slug, so a free-text keyword returned nothing. Add a\nfirst-class companies identifier end to end: the scrape-boards IPC request,\nthe generated Rust contract, BoardSearchInput, and a conditional company\nfield in the scrape form shown only for boards that declare requires_company\n(via catalog metadata, no hardcoded list). The six company-scoped boards\niterate the companies list with per-company endpoints and partial-failure\nisolation, and the engine skips a requires_company board that has no\ncompanies as needs-company, mirroring the existing needs-login skip and UI\nprompt. smartrecruiters also gains real keyword search via its q param.\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-21T08:41:57+02:00",
          "tree_id": "104fde5f62beeb1f14253e1b9ab440e84bb54418",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/06c5617a1aaf505621fb0a6767f0c8c56cf438e5"
        },
        "date": 1782024645237,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1860927,
            "range": "± 54273",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2461073,
            "range": "± 29643",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 266666,
            "range": "± 1936",
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
          "id": "be2ff48b44e94fd3f16e8e857388e273203713a3",
          "message": "feat(scraping): add adzuna and jsearch aggregator with key settings (#465)\n\n* feat(scraping): add adzuna and jsearch aggregator with key settings\n\nAdd an aggregator board to replace self-scraping the anti-bot boards\n(Indeed, Glassdoor, Xing, Workday, StepStone — unreliable in 2026). It is\nbacked by a provider registry: adzuna (free, primary) and jsearch (paid\nfallback, invoked only when adzuna errors, never on a legitimately empty\nresult). Keys are user-supplied, stored in the OS keyring (ai:adzuna-app-id,\nai:adzuna-app-key, ai:jsearch-key) and read at runtime; with no key the\nboard returns empty and never crashes. A Settings field on the Jobs tab lets\nusers paste their own free Adzuna key, with a link to developer.adzuna.com.\nSecrets are stripped from HTTP logs, each provider guards on is_configured(),\nand the cancel signal is checked before the paid fallback fires. Retiring the\nfive legacy boards is a follow-up PR.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): address aggregator review findings\n\nCodeRabbit review on #465:\n- Remove the OnceLock provider cache so a newly-saved Adzuna/JSearch key\n  takes effect on the next search without an app restart.\n- read_credential now returns AppResult<Option<String>>: NoEntry -> Ok(None),\n  a real keyring fault -> Err(AppError::Storage); optional keys still degrade\n  gracefully (log + treated as absent). Docstring reconciled.\n- Harden HTTP log redaction to scheme://host/path (drops query, userinfo and\n  fragment) and URL-encode the Adzuna app_id/app_key, closing the key-in-log\n  path.\n- Fix the cancel test to exercise the pre-fallback cancel guard (cancel\n  during the provider call, not before invocation).\n- Settings: show generic i18n errors instead of raw backend strings, add a\n  dedicated removeError key, and cover save/remove rejection paths.\n- Add Rust tests for the read_credential error branches and the provider\n  degradation paths (via keyring_core::mock).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(settings): guard aggregator key save/remove against re-entrant submits\n\nAddress CodeRabbit re-review on #465:\n- handleSave/handleRemove return early when the mutation is already pending\n  (rapid Enter / double-click no longer fires parallel mutateAsync calls).\n- Reset the shared keyState mock in afterEach so a failed assertion can't\n  leak connectivity state into later tests.\n- Add tests covering both pending-state early returns.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-21T09:55:44+02:00",
          "tree_id": "a8c959591c34cb4b0b9167d7e5ff59990b51a115",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/be2ff48b44e94fd3f16e8e857388e273203713a3"
        },
        "date": 1782029062598,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1896630,
            "range": "± 64985",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2521150,
            "range": "± 16842",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 291946,
            "range": "± 2498",
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
          "id": "ea681ef2c4961e20293ede4e394cb16873f550c4",
          "message": "fix(scraping): harden company-scoped boards (ssrf, fan-out caps, stale events) (#467)\n\n* fix(scraping): harden company-scoped boards (ssrf, fan-out caps, stale events)\n\nAddress CodeRabbit's review of #464, which was merged before its findings\nwere resolved:\n- CRITICAL: validate Personio company slugs before composing the subdomain\n  URL (an unvalidated slug like 127.0.0.1:8443/foo was an SSRF vector);\n  mirrors the Recruitee hostname-label guard.\n- Cap per-board company fan-out (ashby/greenhouse 50, smartrecruiters 20),\n  dedupe and drop blank slugs, and return Err when every company fetch\n  fails instead of a silent empty result.\n- Namespace Personio job ids per company to avoid cross-tenant collisions.\n- Skip needs-company boards when the companies list is whitespace-only.\n- Reject blank company entries at the Zod schema boundary.\n- Guard JobsPage skip notifications to the active scrape round so stale\n  job.completed events no longer raise false sticky warnings.\n- Extract normalize_companies and slug/timestamp helpers; add unit tests\n  (Personio SSRF guard, per-board cap/dedupe/sanitize, hook payload).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): refine ats follow-up per review (cancellation, slug rules)\n\nAddress CodeRabbit re-review on #467:\n- ashby/greenhouse: check cancellation before recording first_fetch_error so\n  a cancelled run no longer surfaces as a false board-level Err.\n- recruitee: tighten is_valid_recruitee_slug to full DNS-label rules (<=63,\n  no leading/trailing hyphen), matching the Personio guard.\n- add tests: cancelled-run returns Ok, recruitee slug edge cases, and a\n  Personio cross-tenant id-namespacing regression.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(scraping): validate personio id namespacing via production helper\n\nAddress CodeRabbit re-review on #467: extract make_job_id() in personio so\nthe id-namespacing test asserts on the production format helper instead of\nreimplementing it (the test now fails if namespacing is removed).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): make personio url-resolver ids match the board path\n\nAddress CodeRabbit on #467: the URL resolver built personio:{id} while the\nboard scraper builds personio:{company}:{id} via make_job_id, so the same\nposting got different ids across ingestion paths (breaking dedupe/upsert).\nRoute the resolver through make_job_id, extract personio_company_from_url so\nboth paths share one host->company parser, and replace the tautological id\ntest with one driven from real URL strings (incl. suffix-evasion -> None).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-21T10:41:47+02:00",
          "tree_id": "a7da8821d6fbeef0cfe853077fcd0385637d731f",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ea681ef2c4961e20293ede4e394cb16873f550c4"
        },
        "date": 1782031809745,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1915189,
            "range": "± 62489",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2615483,
            "range": "± 24247",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 308646,
            "range": "± 7140",
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
          "id": "bd3c34424d28f3a169b78e2fed06edb407525309",
          "message": "refactor(scraping): retire 5 anti-bot board scrapers covered by the aggregator (#469)\n\n* refactor(scraping): retire 5 anti-bot board scrapers covered by the aggregator\n\nRemove the Glassdoor, Indeed, Xing, Workday and StepStone scrapers. All five\nwere structurally blocked by anti-bot defences and returned nothing; the\nexisting Adzuna/JSearch aggregator already covers them via API. The SCRAPERS\nregistry goes 21 -> 16 boards.\n\nScraper-only retirement. Single-job import (the browser-extension flow) is\nunaffected: the scrape_url Indeed/Workday URL resolvers, board_login and\ncredential machinery are kept dormant, and contact-profile Xing + SourceBadge\nsource attribution stay. The in-app accounts/login panel is trimmed to LinkedIn\n(the only board with a live in-app login after retirement); board_login configs\nremain so import can be wired for the others later.\n\nAlso drop the now-dead Indeed-only locale field across the shared schema,\nBoardSearchInput, the generated IPC contract, command passthroughs and test\nfixtures; the aggregator localises via country_code. Persisted Autopilot configs\nreferencing a retired board degrade gracefully (free-string boards, unknown-id\nskip with an error summary).\n\nDocs, README, landing claims (16 boards; walled boards via aggregator, not\nChromium) and ADR-026 updated. German-market depth via Adzuna.de is a separate\nfollow-up, not addressed here.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(scraping): use active board ids and assert retired boards are rejected\n\nSwap leftover retired-board fixtures (indeed) for active ids in the board-status\nhook test, and add negative cases asserting ScrapeBoardsRequestSchema rejects\nretired board ids (indeed, stepstone) so they can't be silently re-added to\nBOARD_IDS. Addresses CodeRabbit review on #469.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-21T13:06:27+02:00",
          "tree_id": "8ec9eea6695d9671cd2f7be82f236abca679a883",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/bd3c34424d28f3a169b78e2fed06edb407525309"
        },
        "date": 1782040503689,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1941193,
            "range": "± 72938",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2584372,
            "range": "± 19732",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 300183,
            "range": "± 1860",
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
          "id": "ca59779f66c173afedf177895ee75c5d42dcfc1f",
          "message": "feat: settings search, theme-aware splash, aggregator default + tailoring fix, extension polish (#470)\n\n* chore(scraping): log non-2xx status and parse failures in fetch_json\n\nfetch_json collapsed non-2xx responses and unparseable bodies into an\nopaque Ok(None), and both branches were invisible at the default log\nlevel (non-2xx was silent; parse failures logged only at debug). This\nmade aggregator/Adzuna failures impossible to diagnose from the terminal.\n\nCompute the secret-safe url once and reuse it: warn on non-2xx with the\nstatus code, and lift the parse-failure log from debug to warn. No\nbehavior change — return values are unchanged and the query string\n(which carries api keys) is never logged.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): parse adzuna job id when returned as an integer\n\nAdzuna's search API returns the job `id` as a bare JSON integer (e.g.\n331705081), but AdzunaJob declared `id: String`, so serde rejected the\nentire response with \"invalid type: integer, expected a string\".\nfetch_json then returned None and the aggregator silently fell back to\nJSearch on every Adzuna call — no Adzuna results ever reached the user.\n\nAdd a de_string_or_number deserializer (untagged string|i64 → String) on\nthe id field so both shapes parse; the value is only ever formatted into\nan id string, so String remains the right internal representation and we\nstay resilient if Adzuna switches back. Other required string fields are\ngenuinely strings in the payload and are left unchanged. Adds regression\ntests for both the integer and string id shapes.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): honor posted-date filter in aggregator and cap results to last month\n\nThe aggregator board ignored BoardSearchInput.date_filter entirely; only the\nlinkedin board consumed it. Adzuna was queried with no recency param and its\ndefault relevance sort, so stale postings (e.g. 41 months old) floated to the\ntop regardless of the user's \"past 24h\" selection.\n\nThread date_filter through JobProvider::search into both providers and always\nbound recency so nothing older than a month is ever returned:\n- adzuna: sort_by=date&sort_direction=down (newest first) + max_days_old\n  (24h->1, week->7, month or no filter->30)\n- jsearch: date_posted (today | week | month, defaulting to month)\n\nSub-day windows (30m..8h) collapse to 1 day / today since neither API\nsupports finer than whole-day / today granularity.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): default to aggregator board and fix tailoring its job ads\n\nAggregator job ads now load when tailoring, and the aggregator is the\ndefault board across the jobs page and autopilot.\n\n- Carry the scraped posting description into the saved Application\n  (applications_track + save_from_posting persist req.job_description;\n  ApplicationTrackSchema gains an optional 200KB-capped jobDescription).\n  TailorFlow re-resolves a short/empty carried description and prefers\n  the longer of carried vs fetched.\n- Add net::http::get_guarded_following_redirects so the generic URL\n  resolver follows an aggregator redirect_url to the real posting,\n  re-validating every hop (SSRF-safe, hop-capped).\n- Default the jobs page and the autopilot wizard to the aggregator board.\n- Restyle the titlebar global back button and group the expand-sidebar\n  toggle beside it; drop the floating page overlay.\n- Tests: schema cap, redirect-follower first-hop rejection, PostingRow\n  payload, TailorFlow prefer-longer, Titlebar collapsed button, autopilot\n  default. Plus rustfmt reflow of the prior date-filter commit.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(extension): span title underline fully and keep offline guidance through reconnects\n\n- Underline: `.title` is now `width: fit-content` so the hand-drawn SVG\n  underline anchors to the \"AI Job Hunter\" text instead of the wider flex\n  box — it was ending mid-word (\"Hunt\") instead of under the full wordmark.\n- Reconnect: once the offline view (\"Get the app\" + tips) has shown, a\n  transient `searching` status no longer swaps it for the spinner. The\n  guidance stays until the app actually connects (the pill shows\n  \"Connecting…\" and Retry remains). A `hasShownOffline` flag drives this,\n  reset on connected / not_paired / bad_token.\n- Tests: 3 popup tests for the sticky-offline behavior + a hard guard on\n  the onMessage-listener capture so a missed listener fails loudly.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(settings): add macos-style search with keyboard nav and highlighting\n\nA search field at the top of the Settings sidebar finds individual settings\nacross all sections (macOS Ventura System Settings style).\n\n- Per-setting localized manifest (titleKey + shared keywords + section +\n  anchor); a non-empty query swaps the section nav for a flat results list.\n- Selecting a result switches to its section, scrolls the control into\n  view, and pulses a highlight ring — reduced-motion-aware (instant scroll,\n  no ring under prefers-reduced-motion). Controls carry data-settings-anchor.\n- Keyboard: Ctrl/Cmd+F focus, arrow navigation (with wrap), Enter select,\n  Esc clear. Combobox/listbox ARIA with aria-activedescendant and an\n  aria-live result-count announcement.\n- Localized titles (existing i18n keys) + a shared keyword set; en + de\n  strings added.\n- Tests: match function, a render-based anchor drift guard (every manifest\n  anchor must resolve to a rendered element), full SettingsSidebar keyboard\n  + ARIA coverage with self-verifying matchEntries-derived assertions, and\n  the SettingsContent pulse effect (happy, reduced-motion, null-anchor,\n  activeSection re-fire).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: suppress known jsdom not-implemented stderr noise in the renderer suite\n\nA vitest globalSetup patches process.stderr.write to drop the two jsdom\n\"Not implemented: window.scrollTo\" / \"navigation\" lines emitted by TanStack\nRouter's scroll-restoration path during component tests. They are not\nfailures — filtering them keeps the suite output (and the review gate's\ndiff view) readable.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore(deps): bump vitest packages to 4.1.9\n\nBump vitest, @vitest/coverage-v8, and @vitest/browser-playwright to ^4.1.9\nacross the workspace (root + shared, ui, prompts, tauri, extension). Full\nsuite green on 4.1.9: tauri 1391, ui 431, extension 63, plus shared/prompts.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(splash): add theme-aware native splash screen\n\nA native splash window shows instantly on launch and covers the whole\ncold-start; the main window stays hidden (visible:false) until ready.\n\n- Theme: resolved natively at startup from a <data_dir>/ui-theme mirror the\n  renderer writes via the validated set_theme_mirror command (light|dark\n  only), falling back to the OS theme on first launch. The splash.html is\n  self-contained (no remote resources) with light + dark variants matching\n  the brand gradient, reduced-motion-aware, no FOUC.\n- Reveal coordination: an app_ready command enforces a 700ms minimum\n  display; an idempotent RevealGuard (AtomicBool) plus a 10s safety timeout\n  guarantee the main window is always revealed exactly once even if the\n  renderer never signals. Every reveal path (app_ready, timeout, degraded\n  boot, deep-link cold start) funnels through splash::reveal_main. All async\n  spawned from setup uses tauri::async_runtime::spawn (boot-panic safe).\n- Renderer: AppReadyBridge fires app_ready once after first paint; a\n  MutationObserver on data-color-scheme keeps the theme mirror current on\n  boot and on every theme change.\n- Tests: theme resolution + OS fallback, RevealGuard single-winner under a\n  16-thread race, set_theme_mirror validation (reject + no-write), app_ready\n  branching/min-display math, the renderer bridges, and a plain-#[test]\n  setup-spawn smoke test that catches a bare-tokio-spawn regression.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: classify the splash module in the shell layer\n\nMirror the architecture test's new classification: add `splash` to the\nL3 (shell) layer list in architecture-rules.md and the Rust core module\ntable in ARCHITECTURE.md (native theme-aware cold-start splash window;\nowns the RevealGuard for idempotent reveal coordination).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* style: format splash.html with prettier\n\n* fix: server-side job-description cap, splash spans, a11y and test hardening\n\nAddresses the CodeRabbit review on PR #470.\n\n- Reject oversized job_description server-side (200 KB UTF-8) in\n  applications_track + save_from_posting before persistence, via a typed\n  AppError helper sharing MAX_JOB_DESCRIPTION_BYTES with the store clamp (+tests).\n- scrape_resolve_url: lower the redirect-follow max_hops 5 -> 2 so one\n  limiter slot maps to <=3 fetches (the Limiter has no weight API); comment\n  corrected to match.\n- Splash lifecycle: add observability::Span instrumentation to\n  write_theme_mirror, spawn, and the reveal paths.\n- SettingsSidebar: gate aria-controls to isSearching && results.length > 0\n  so it never references a missing listbox id in the no-results branch.\n- matchEntries: lower-case keywords so keyword matching is truly\n  case-insensitive.\n- Tests: the anchor drift guard now renders the real AISettingsTab (hook-\n  level mocks) so a dropped/typo'd data-settings-anchor fails; restore\n  window.matchMedia between SettingsContent tests; explicit fixture\n  assertions for the SettingsSidebar arrow-key tests; anchored\n  /^(light|dark)$/ regex; multi-byte UTF-8 byte-ceiling test for\n  ApplicationTrackSchema.\n- vitest.global-setup: drop the banned @ts-expect-error (cast instead) and\n  forward all stderr.write args (no encoding/callback drop).\n\nDeferred: a hermetic later-hop SSRF test for get_guarded_following_redirects\nis infeasible (a mock server binds loopback, rejected at hop 0); the per-hop\nguard is covered compositionally by the existing first-hop + literal-rejection\ntests.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: add multi-byte byte-cap tests for the job-description guard\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-22T01:44:58+02:00",
          "tree_id": "476d20ddbd53972823582280232b44074e8a3097",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ca59779f66c173afedf177895ee75c5d42dcfc1f"
        },
        "date": 1782086008751,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1922256,
            "range": "± 69033",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2585395,
            "range": "± 103371",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 302261,
            "range": "± 8338",
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
          "id": "5299a13cdb04e239761e5a45c2ed28915b551324",
          "message": "refactor: centralize credential slot names in @ajh/shared via codegen (#472)\n\n* refactor: centralize credential slot names in @ajh/shared via codegen\n\nMake the Adzuna/JSearch credential slot names a single cross-language source of\ntruth, killing the TS<->Rust literal duplication. Keyring slot strings and wire\nvalues are byte-identical (pure refactor).\n\n- new packages/shared/src/provider-slots.ts — PROVIDER_SLOTS (bare names; the\n  ai: keyring namespace is applied Rust-side at read time)\n- gen-ipc-rust.ts genSlots() (mirrors genEvents) emits ipc_contracts/provider_slots.rs,\n  guarded by gen:ipc:check in CI\n- renderer (AggregatorKeysSettings, ScrapeForm, AdzunaKeyStep) + Rust aggregator\n  and tests reference the one source instead of scattered literals\n- architecture.rs R7 allowlist entry for scraping -> ipc_contracts (compile-time\n  consts, same pattern as the events channel consts)\n- add packages/shared/scripts/tsconfig.json so the IDE resolves node: imports in\n  scripts/; fixes a latent noUncheckedIndexedAccess slip it surfaced in genEvents\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: document provider-slots codegen and scraping upward-import allowlist\n\nSync architecture docs for the credential-slot single-source change: add the\nscraping -> ipc_contracts entry to the R7 upward-import allowlist table, and\nnote the provider_slots.rs codegen (source: packages/shared/src/provider-slots.ts)\nalongside the existing events.rs pattern.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: reference provider-slot symbol instead of hardcoded literal\n\nDrop the hardcoded \"adzuna-app-id\" value from the event-system codegen note;\npoint at PROVIDER_SLOTS / ADZUNA_APP_ID instead, per the docs thin-pointer rule.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-22T16:44:21+02:00",
          "tree_id": "23174c3443b304bbf15be035191412dabd3ba65e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/5299a13cdb04e239761e5a45c2ed28915b551324"
        },
        "date": 1782140574050,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1891334,
            "range": "± 104360",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2496638,
            "range": "± 48791",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 285552,
            "range": "± 7244",
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
          "id": "99e7286d3a68571af4de811362d41c94a9d156b5",
          "message": "fix: show aggregator in autopilot; centralize board and date-filter constants (#473)\n\n* fix: show aggregator in autopilot; centralize board and date-filter constants\n\nThe autopilot wizard hardcoded BOARD_IDS in its picker, so the aggregator board\n(the default) was selected-but-invisible and unusable. Switch the picker to the\ndynamic board catalog (mirrors the jobs ScrapeForm), add the missing-keys hint,\nand nudge missing Adzuna keys from the jobs zero-results state.\n\nBundles the aggregator/board-domain hardcoding cleanup:\n- add 'aggregator' to BOARD_IDS so it validates as a BoardId; new AGGREGATOR_BOARD_ID\n  constant replaces bare 'aggregator' literals (ScrapeForm/JobsPage/wizard-state)\n- dedup the credential board enum to z.enum(AUTH_CAPABLE_BOARDS) (revives the dead\n  const; kept intentionally distinct from BOARD_IDS = scrapeable vs login-capable)\n- date-filter codegen: genDateFilters -> ipc_contracts/date_filters.rs, with a Rust\n  exhaustiveness test so a new DATE_FILTER token unhandled by the aggregator match\n  arms fails (default-equal mappings can't masquerade; only 'month' may equal default)\n- a11y: role=\"status\"/aria-live on the aggregator key-hint and the empty-state swap\n- i18n: jobs.emptyNoAdzunaKeys (+ CTA) in en + de\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: document date-filter codegen and catalog-driven autopilot picker\n\nAdd date_filters.rs to the codegen note (source: DATE_FILTER_OPTIONS in\npackages/shared), record 'aggregator' + AGGREGATOR_BOARD_ID in the board\nregistry doc, and note the autopilot picker is now board-catalog-driven.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: gate missing-adzuna-keys empty-state on resolved key queries\n\nuseHasProviderKey returns undefined while loading, so the prior check treated\nan unresolved query as \"keys missing\" and could flash the wrong empty-state for\nusers who have keys. Gate on isSuccess and check .has === false explicitly.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: strengthen jobs empty-state key tests (provider-aware mock + click path)\n\nMake the useHasProviderKey mock per-slot so a test can't pass with the wrong\nslot queried, and add an interaction test asserting the missing-keys CTA fires\nsetSettings({ activeSection: 'job' }) and navigates to /settings.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: honor the enabled arg in the jobs key-mock to mirror the real hook\n\nuseHasProviderKey returns undefined / isSuccess:false when disabled; the mock\nnow respects its second (enabled) arg so a test can't pass if the component\nbreaks the isEmpty query-gating.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-22T18:30:12+02:00",
          "tree_id": "7c115e376f7a98e1193b45dda86cf51b31ce21f1",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/99e7286d3a68571af4de811362d41c94a9d156b5"
        },
        "date": 1782146340212,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2015258,
            "range": "± 44662",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2672096,
            "range": "± 87523",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 320329,
            "range": "± 6760",
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
          "id": "1196b8c901b0f39e49c79b9e3e674797160ec8d3",
          "message": "refactor: centralize timeouts/durations and use section/stage registries (#474)\n\n* refactor: centralize timeouts/durations and use section/stage registries\n\nConstants-cleanup pass (pure refactor, values/behavior byte-identical):\n\n- Rust: 26 AI-provider HTTP .timeout() literals -> a named-by-operation\n  commands/ai_provider/timeouts.rs module (stream/completion/embed/web-search/\n  list-models/health/show/model-pull; same-value-different-purpose kept separate)\n- Renderer: scattered React Query staleTime/gcTime/refetchInterval -> named\n  QUERY_TIMES constants; recurring setTimeout UX delays -> lib/timings.ts\n  (genuine one-offs left inline)\n- Replace hardcoded string-literal conditional chains with typed lazy render\n  registries Record<Key, () => ReactNode> (compile-time exhaustive): SettingsContent\n  (11-branch activeSection), TailorFlow / AnalyzePage / ResumeBuilderPage (stage)\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: codify the render-registry dispatch pattern\n\nAdd a thin patterns section for the typed Record<Union, () => ReactNode> render\nregistry (lazy, compile-time exhaustive) used for section/stage dispatch, plus\nan anti-pattern entry pointing the old string-literal conditional chains at it.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: slim the render-registry pattern to thin pointers\n\nDrop the embedded registry snippet with literal component names (drift risk);\nkeep the concept, the Record<Union, () => ReactNode> type shape, and the\nexisting source references, per the docs thin-pointer rule.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-22T19:31:25+02:00",
          "tree_id": "1b5ce67e50633439eb8342371f0ec43c539552c3",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/1196b8c901b0f39e49c79b9e3e674797160ec8d3"
        },
        "date": 1782149991107,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1881151,
            "range": "± 45861",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2501546,
            "range": "± 81534",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 289683,
            "range": "± 3085",
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
          "id": "48f9158f178aff1ccdaa1a6f2dc349d1b29c1c7b",
          "message": "refactor: replace the native splash window with an in-app overlay (#475)\n\n* refactor: replace the native splash window with an in-app overlay\n\nSwap the separate native splash window for a self-contained in-app React\noverlay, removing the cross-process machinery it required.\n\n- delete splash/mod.rs (RevealGuard, spawn, reveal paths, theme-mirror) +\n  public/splash.html; main window is now visible at boot (themed backgroundColor\n  covers the brief pre-mount frame)\n- remove the app_ready + set_theme_mirror IPC commands + their SystemContract\n  entries and channels; deep-link cold-start reveals via tray::show_focus\n- new components/layout/AppSplash overlay: brand wordmark + shimmer via design\n  tokens, theme-aware (renderer already resolves the scheme), self-dismiss after\n  ~700ms with a reduced-motion path; pointer-events-none + fallback unmount so a\n  stalled exit animation can never trap the app\n- drop the renderer AppReadyBridge/ThemeMirrorBridge/useSyncThemeMirror bridges\n- architecture.rs: remove the splash L3 entry\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: drop native splash from architecture docs, note in-app overlay\n\nRemove the splash backend module from the L3 list and the module table; note\nthe splash is now the in-app components/layout/AppSplash overlay (no native\nwindow).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: remove the duplicate splash entry from the l3 list\n\nThe L3 module set is listed twice in architecture-rules.md; the prior edit\ndropped splash from the code-block list (line 20) but not the section header.\nRemove it there too so both enumerations agree.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-22T20:43:12+02:00",
          "tree_id": "ff7663b826f6a9263b339af57df1e8f5d696d841",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/48f9158f178aff1ccdaa1a6f2dc349d1b29c1c7b"
        },
        "date": 1782154283215,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1884631,
            "range": "± 48773",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2464306,
            "range": "± 21264",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 286170,
            "range": "± 3211",
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
          "id": "f15e83d190cf720aa3eb0af4d6e6253664b9ce10",
          "message": "fix: buffer cold-start deep-link autopilot focus so it isn't lost (#477)\n\n* fix: buffer cold-start deep-link autopilot focus so it isn't lost\n\nA cold-start ajh://autopilot/<id> deep link emitted the focus event during\nRust setup, before the renderer's useAutopilotFocusNavigation listener\nattached, so the focus intent was lost. Mirror the proven menu cold-start\npattern:\n\n- tray: dedicated PendingFocus buffer + dispatch_focus (buffer the id BEFORE\n  show_focus, then emit as the low-latency trigger + deferred re-emit)\n- commands/autopilot: autopilot_take_pending_focus (atomic take-and-clear)\n- lib: route handle_deep_link's autopilot arm through dispatch_focus; manage\n  PendingFocus EARLY in setup (before the deep-link block — managing it later\n  in tray::build would no-op the cold-start write) + register the command\n- renderer: useAutopilotFocusNavigation keeps the live event AND pulls the\n  buffer on mount + window focus/visibility, exactly-once via the atomic take\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: document the cold-start deep-link focus buffering pattern\n\nNote autopilot.takePendingFocus in API.md and add a cold-start-buffering\nsection to the event-system doc (shell buffers before show_focus; renderer\npulls via take-and-clear), with the early-manage ordering warning. Points at\nPendingFocus / autopilot_take_pending_focus alongside the menu equivalents.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: anchor deep-link focus-pull tests with call assertions\n\nThe dotted-key mock override is correct for this repo's Proxy-based\ncreateMockClient (test-support.tsx resolves dotted keys), but the\nbuffered-pull tests lacked explicit takePendingFocus call assertions.\nAdd toHaveBeenCalledOnce/Times so they provably exercise the pull path\nand fail loudly if the override ever stops applying.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-22T23:55:14+02:00",
          "tree_id": "cdfbbdadb1d0d0fefa2c7490de472f82d23593ac",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f15e83d190cf720aa3eb0af4d6e6253664b9ce10"
        },
        "date": 1782165851512,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1922520,
            "range": "± 36969",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2597156,
            "range": "± 41563",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 304441,
            "range": "± 15480",
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
          "id": "bc5e00e1040936ed59ee4dfef71b36f71fe75d86",
          "message": "fix(autopilot): forward country to aggregator + stop keyword-prefill (with dep bump, landing OG fix) (#483)\n\n* fix(autopilot): forward country code to aggregator and stop pre-filling keyword filter\n\nAutopilot returned zero jobs from the aggregator board while a manual search\nreturned jobs for the same query, due to two divergences from manual search.\n\n1. Autopilot never forwarded a country code, so the aggregator's Adzuna provider\n   defaulted to \"de\" (Germany) for every run. Manual search captures countryCode\n   from the location geocode suggestion and forwards it into Adzuna's URL path.\n   Thread an optional countryCode/country_code end-to-end (shared Zod schema,\n   regenerated IPC contract, AutopilotTarget, autopilot_helpers forwarding) and\n   capture it in the creation wizard's LocationInput, mirroring manual ScrapeFilters.\n\n2. The wizard pre-filled the \"Must include\" keyword filter from the user's entire\n   tech stack, and the backend requires ALL keywords present, so nearly every\n   posting was dropped. Leave the filter empty by default (opt-in) and remove the\n   now-dead prefill plumbing plus the orphaned i18n key.\n\nThe field is optional and absent-by-default: old persisted autopilots deserialize\nto None and behavior is unchanged when unset.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore(deps): bump dev and runtime dependencies\n\nBatch dependency update bundled in at the user's request.\n\n- Tooling: @commitlint/cli, @types/node, eslint, eslint-plugin-storybook,\n  globals, knip, lint-staged, typescript-eslint, jsdom, playwright, vite,\n  tailwindcss, @tauri-apps/cli, @tanstack/router-vite-plugin.\n- Runtime: @tanstack/react-router, @tanstack/react-virtual, @tauri-apps/api\n  and plugins (shell, store, websocket, notification, os, positioner,\n  global-shortcut), @tiptap/*, lucide-react, motion, react-hook-form, zustand,\n  @wxt-dev/browser (extension).\n- pnpm-workspace.yaml: minimumReleaseAgeExclude entries to opt the freshly\n  released typescript-eslint 8.62.0 family, globals 17.7.0 and knip 6.18.0\n  out of the new-release cooldown guard.\n\nFull typecheck, lint:strict, cargo and vitest suites pass with these versions.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(landing): remove stray authoring note from og:description\n\nThe og:description meta tag contained a leftover authoring note —\n\"(the OG image should be the deep-fried 'IT DOES EVERYTHING ELSE' frame)\" —\ninside its content string, so link-preview unfurlers rendered it verbatim.\nDrop the parenthetical so it matches the already-clean twitter:description.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(autopilot): clear captured country code when the location is edited freely\n\nAfter picking a geocode suggestion (which captures the country) and then editing\nthe location text without re-picking, the stale countryCode was still forwarded to\nthe aggregator. Clear it on the free-text onChange; the suggestion-pick path re-sets\nit (LocationInput fires onChange before onSelectSuggestion, so a pick clears then\nsets). Addresses CodeRabbit review on #483.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore(deps): document the pnpm release-age cooldown exemptions\n\nAdd a comment explaining the exemption list opts freshly released tooling versions\nout of the new-release cooldown, and that each entry is transient (remove once the\nversion ages past the window). Addresses CodeRabbit review on #483.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(autopilot): render the boards array in card and drop the stale singular type\n\nThe shared Autopilot target type still declared singular `board: string`, but the\nbackend serializes `boards: string[]`, so `ap.target.board` was undefined at runtime\nand the card's board badge rendered blank.\n\n- Shared type: `board: string` -> `boards: string[]` (now matches the Rust struct and\n  the Zod schema; resolves the drift).\n- AutopilotCard badge: one board shows its localized label via `jobs.boards.*`,\n  multiple show a translated `autopilot.card.boardsCount` (\"{{count}} boards\", de\n  \"{{count}} Boards\").\n- AutopilotPage: passes `boards[0] ?? AGGREGATOR_BOARD_ID` to the application save\n  (was the undefined singular field).\n- wizard-state: drops the now-unnecessary `as unknown as` cast and reads `boards`\n  directly.\n\nAddresses CodeRabbit review on #483.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(autopilot): persist per-job source board so multi-board apply is accurate\n\nRecord the scraping board on each found job (its JobPosting.source) instead of\nfalling back to target.boards[0] in the apply flow, which was wrong for\nmulti-board autopilots. The aggregator board collapses both Adzuna and JSearch\npostings to source \"aggregator\", so the persisted board stays a clean board id.\n\n- Add FoundJob.board (Option<String>, serde-default so old persisted records\n  load as None) and set it from the posting source, empty treated as absent.\n- Carry the board across the record_run dedup merge (append via ..inc; existing\n  rows refresh from the incoming posting like location/description).\n- Expose board?: string on the AutopilotFoundJob TS type (no Zod schema exists\n  for found jobs, so only the type changes).\n- AutopilotPage prefers job.board, falling back to boards[0] then the aggregator\n  default for legacy records.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): surface a diagnostic for adzuna-unsupported countries instead of zero\n\nAdzuna's API is path-based per country (/jobs/{country}/search/1) and only hosts ~19\nmarkets. When the aggregator forwarded a country Adzuna doesn't cover, the request\n404'd and — with no JSearch key — the result was swallowed into an empty list, so the\nrun silently found nothing. Now made worse once autopilot started forwarding the\ngeocode country.\n\n- Add an Adzuna supported-country allowlist; unsupported countries short-circuit before\n  the doomed HTTP call and fall through to JSearch (global) when it is configured.\n- When neither provider can serve the country (Adzuna can't, JSearch unconfigured),\n  return an actionable error via the board summary instead of a silent empty: \"add a\n  JSearch key in Settings for global coverage\". The keyless-empty case (no provider\n  configured at all) is unchanged.\n- Log board errors in autopilot runs alongside skipped boards.\n\nFixes both the manual and autopilot aggregator paths (shared board). Addresses\nCodeRabbit review on #483.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: note the adzuna country-code limitation in the scraping knowledge base\n\nThin pointer to the new ADZUNA_SUPPORTED_COUNTRIES allowlist and the\ndiagnostic-vs-silent-empty behavior in the aggregator board (PR #483).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* style(scraping): apply rustfmt to the aggregator country-guard changes\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(shared): validate the country code as iso 3166 alpha-2 at the schema boundary\n\nBoth the manual scrape request and autopilot target schemas accepted any string for\nthe country code; the value is geocode-sourced (always alpha-2), so a 2-letter regex\nstops malformed values propagating through IPC and scraping without rejecting valid\ninput. Addresses CodeRabbit review on #483.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(autopilot): lock the empty-boards fallback for saved autopilots\n\nAddresses CodeRabbit review on #483.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(scraping): tighten the adzuna unsupported-country diagnostic assertion\n\nRequire the provider prefix, the searched country code, and the supported-market-list\nphrase together (was a 3-way OR satisfiable by a partial match). Addresses CodeRabbit\nreview on #483.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: point to the adzuna allowlist constant instead of listing country codes\n\nReplace the embedded 19-country list with a pointer to ADZUNA_SUPPORTED_COUNTRIES so\nthe knowledge base can't drift from the source of truth. Addresses CodeRabbit on #483.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-23T11:25:26+02:00",
          "tree_id": "3b7fbab0cfb48ec9f7d43774abebca20771d8c81",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/bc5e00e1040936ed59ee4dfef71b36f71fe75d86"
        },
        "date": 1782207899304,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1920703,
            "range": "± 62176",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2536380,
            "range": "± 32625",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 282087,
            "range": "± 10275",
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
          "id": "16ebdf751de96dc998f9f6b3e477517c95f6e243",
          "message": "fix(autopilot): match manual-search filters and surface zero-result reasons (#484)\n\n* fix(autopilot): match manual-search filters and surface zero-result reasons\n\nAn autopilot run returned zero jobs from the aggregator while a manual\nsearch with the same query returned jobs. The run applied three filters\nmanual search never does, all silently:\n\n- a 24h date-window default (manual defaults to any-time),\n- a must-include-ALL keyword filter auto-prefilled with the whole tech\n  stack on pre-#483 saves,\n- a min-match-score gate (default 50).\n\n#483 only changed wizard defaults for NEW autopilots; existing saved ones\nstill zeroed out, and a zero run gave no reason.\n\nFixes:\n- Realign defaults to manual parity. The authoritative AutopilotFilter\n  minMatchScore Zod default goes .default(50) -> .default(0) (regenerates\n  the IPC contract), and wizard buildDefaults dateFilter '24h' -> ''.\n- One-time, marker-gated migration (relax_legacy_filters_once, marker\n  autopilot_relax_v1.done) loosens existing saved autopilots: clears\n  must-include keywords; resets min-score 50->0 and date \"24h\"->None only\n  when they still equal the old defaults. Persist-then-mark, so a failed\n  save retries on next launch.\n- Zero-result diagnostics: scrape_diagnostics surfaces per-board skip and\n  error reasons (no API keys, unsupported Adzuna country) into the run\n  step log, and scrape_done reports raw-scraped vs after-keyword-filter\n  counts.\n- Wizard gains an \"Any\" (no-minimum) match-score tile with aria-pressed,\n  and the schedule summary mirrors it.\n\nTests: 73 cargo autopilot tests (migration orchestrator incl. the\npersist-failure retry guarantee, relax_legacy_filters sentinels,\nscrape_diagnostics format, create fallback default) and renderer\nregression locks on the new defaults.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ui(autopilot): give the scrape diagnostics step a warning glyph\n\nThe scrape_diag run step surfaces why a run found zero jobs; without its\nown step icon it fell back to the generic dot and didn't stand out in the\nrun log. Add a unicode warning glyph matching the existing step icons.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(autopilot): replace 10-arg fixture with base_autopilot override helper\n\nThe migration tests used a 10-positional-arg make_autopilot constructor\nthat tripped clippy::too_many_arguments. Replace it with a zero-arg\nbase_autopilot() returning the worst-case legacy record; each test mutates\nonly the field it exercises, matching the existing found_job fixture style.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(autopilot): make legacy-filter migration idempotent and redact run diagnostics\n\nAddresses CodeRabbit review on #484.\n\n- relax_legacy_filters now gates the must-include keyword clear on a\n  was_legacy sentinel (min_match_score == 50.0 || date_filter == \"24h\"),\n  computed before the resets. An already-relaxed record is a full no-op,\n  so a failed marker write that reruns the migration can no longer erase\n  keywords the user added in between. The marker is now a pure\n  optimization; the common legacy autopilot is still fully relaxed.\n- scrape_diagnostics now runs each per-board reason through sanitize_reason\n  before it reaches the renderer step log: redacts URLs (closing the\n  Adzuna app_id/app_key leak in reqwest transport errors), absolute and\n  drive-less home paths, and bare host:port/IPv4, capped at 200 chars.\n  A control test guards against over-redacting status codes and timestamps.\n- Thin the autopilot/scraping knowledge docs back to symbol pointers\n  (no copied literals).\n\nTests: 79 cargo autopilot tests (idempotency rerun-safety, redaction +\nover-redaction control).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-23T13:22:41+02:00",
          "tree_id": "328e540d099199d293a01b187f211bbedd65d9e4",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/16ebdf751de96dc998f9f6b3e477517c95f6e243"
        },
        "date": 1782214297117,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1917526,
            "range": "± 51505",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2498874,
            "range": "± 100910",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 278396,
            "range": "± 1361",
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
          "id": "b53e296b023f83930f9fc574c922fc7c9284ca92",
          "message": "fix(autopilot): redact standalone credential tokens in scrape diagnostics (#485)\n\n* fix(autopilot): redact standalone credential tokens in scrape diagnostics\n\nFollow-up to CodeRabbit on #484 (two 🔵 nitpicks).\n\n- redact_token now redacts standalone credential-assignment tokens\n  (app_key=, app_id=, api_key=, key=, secret=, token=, password=, …) to\n  <credential-redacted>, gated on the literal '=' so benign words\n  (keyword, bare token, v1.2.3) are untouched. Ordered after the URL\n  branch so a full https://…?app_key=… token still wins <url-redacted>.\n  Defense-in-depth: no current provider emits a bare key into a board\n  error (Adzuna's is only inside the full URL; JSearch's is a header).\n- Add a regression test pinning the documented was_legacy gap: a record\n  with prefilled keywords but score != 50 and date != \"24h\" reads\n  non-legacy, so relax_legacy_filters keeps the keywords.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* refactor(autopilot): drop credential markers subsumed by key=\n\nThe is_credential substring check used app_key=, apikey= and api_key=,\nbut all three contain key=, so that marker already matches them. Remove\nthe three dead entries and note why. Behavior is byte-identical; the\nredaction tests pass unchanged.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-23T14:02:38+02:00",
          "tree_id": "7815f8a3d6d8a1bc89758f335568bdf2a65d6466",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b53e296b023f83930f9fc574c922fc7c9284ca92"
        },
        "date": 1782216669040,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1934519,
            "range": "± 52924",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2580244,
            "range": "± 23297",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 290676,
            "range": "± 1635",
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
          "id": "bc6c9e8789b39f85c208967163a067d3da6a1f18",
          "message": "feat: jobs split view, markdown descriptions, on-demand scoring, and linux/steam deck support (#486)\n\n* refactor(jobs): extract shared posting actions hook\n\nReuse one persist/track/open/save/tailor/copy implementation across PostingRow, the new split-view list item, and the detail pane. The viewed badge now also reflects an opened interaction.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* refactor(jobs): move the match band into a shared renderer module\n\nRelocate the band and scoreTier out of the jobs feature so autopilot can reuse them without a cross-feature import. Adds a coverage variant and a subtle mode for the compact list.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): track view mode and selection in the session store\n\nAdd viewMode (defaults to split), selectedId and detailCollapsed to the jobs slice so the split view survives remounts, re-sorts and live-scrape prepends.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): add linkedin-style master-detail split view\n\nTitle list on the left, full job description on the right, toggled from the page header and defaulting to split.\n\nCompact two-line rows, softened match bands, keyboard listbox navigation, top-job auto-select, and responsive single-pane collapse with back and expand controls below md.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(autopilot): expand cards on click with match labels and viewed badge\n\nClicking the card header expands or collapses found jobs; each found job shows a coverage-variant match label and a viewed badge.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(i18n): add jobs split-view and match band strings\n\nAdd the view-mode, master-detail, match-band, score-loading and copy-link-error keys for en and de.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: document the jobs split view and match band variants\n\nNote the split-view default on the jobs route, the contrast floor for small muted text, and the combined versus coverage match-band thresholds.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(scraping): resolve aggregator redirects to full job text\n\nFollow the aggregator redirect to its final URL (IP-guarded per hop) and re-dispatch the board scrapers there, so a click-through to a supported board yields full text; 429 or error keeps the snippet.\n\nAlso tightens the Workday and SmartRecruiters host gates to suffix match and rate-limits the extension-bridge resolve path.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): add scrape_update_description to re-score on full text\n\nAdd an in-place PostingsCache update-by-id plus a scrape_update_description command and contract so the renderer can write a resolved description back for re-scoring.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): type interaction sets as string for strict typecheck\n\nType the interaction Set as Set<string> so has() accepts string keys; these errors were masked because the filtered typecheck wrapper is a no-op and the esbuild test runner does not type-check.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): fetch, reformat and re-score the full description on open\n\nAggregator jobs with a short snippet fetch the full posting on open (longer text wins, with a manual retry), render it as paragraph/list/heading blocks, and persist it so the score uses the full text.\n\nAlso fixes the strict-typecheck errors in the touched test fixtures (timestamp not createdAt) and adds the updateDescription mock-client stub.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): recall previously-typed values in scrape and filter inputs\n\nAdd stable id/name to the scrape query and results filter inputs so the WebView2 form-history dropdown recalls previously-typed values, matching the autopilot name input.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: document full-description resolve, re-score, and reformat\n\nNote the resolve-on-open full-description path, the write-back and re-score flow, and the readable-block reformatting in the scraping and automation knowledge docs.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore: make claude hook commands cwd-proof\n\nReference the hooks via $CLAUDE_PROJECT_DIR (Claude Code substitutes it before the shell runs) so they resolve regardless of the session working directory.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(scraping): convert job descriptions to markdown via htmd\n\nAdd html_to_markdown (htmd) for board and aggregator description fields instead of strip_html, so the renderer receives structured markdown; falls back to html_to_text on error.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(match): strip markdown from scoring text and keep utf-8 intact\n\nCollapse markdown links to anchor text and drop bare URLs in the scoring blob so URL fragments don't pollute ATS keywords; operate on str slices to preserve non-ASCII (German).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): markdown job descriptions and on-demand match scoring\n\nRender descriptions with a shared @ajh/ui JobDescription (react-markdown) in the jobs detail pane and the applications job-ad tab (which resolves the full text on open).\n\nScore each job once on open via a reactive per-job query (no batch scoring, no score-sort, no list reorder), persist the resolved full text before scoring, and remove the dead batch path.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: document on-demand scoring and markdown rendering\n\nUpdate the scraping and automation knowledge docs from the removed match-batch invalidation to the per-job on-demand scoring flow.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(linux): steam deck wayland appimage rendering and cross-distro browser detection\n\nWayland AppImage: set the WebKit DMABUF/compositing env and preload the host libwayland-client (re-exec) so the webview renders on Steam Deck/Mesa instead of aborting with EGL_BAD_PARAMETER.\n\nBrowser detection: find Chrome/Chromium/Brave/Edge/Vivaldi across native, Snap and Flatpak installs, and also write the native-messaging manifest to the Flatpak per-app config dirs.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(match): preserve utf-8 when stripping markdown from the scoring blob\n\nOperate on str slices instead of byte-as-char in markdown_to_plain so German/accented job descriptions are not mojibaked before keyword extraction.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* style: apply rustfmt to the new scrape_url tests\n\nReflow the added scrape_url tests to rustfmt-canonical form so the pre-push cargo fmt gate passes.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(ui): use the alert component in onboarding and autopilot wizard steps\n\nReplace ad-hoc colored status banners with the @ajh/ui Alert across the onboarding steps (Adzuna key, AI/Ollama/cloud panels, browser-not-detected) and the autopilot StepTarget hint.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(linux): apply theme without view transitions on webkitgtk\n\nWebKitGTK crashes the web process inside document.startViewTransition; gate it off on Linux so appearance changes (theme/transparency/accent/contrast) apply directly instead of closing the app.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(onboarding): show the detected browser name and localize the ready state\n\nDerive the real browser label (Chrome/Chromium/Brave/Edge/Vivaldi) from the detected path and use i18n instead of hardcoded Chrome copy, now that non-Chrome and Flatpak browsers are detected.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: document linux steam deck support and the alert wizard convention\n\nNote the Wayland AppImage safeguard, cross-distro/Flatpak browser detection and native-messaging caveat, the WebKitGTK view-transition gate, and the Alert standard for wizard status.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): invalidate the interactions query prefix so viewed badges refetch\n\nusePersistJob invalidated a key with a trailing undefined that never matched the typed interactions queries; invalidate the two-element prefix so the viewed badge refetches on click.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(ui): render markdown image alt text instead of a live img in descriptions\n\nMap img in the JobDescription markdown renderer to its alt text so scraped image syntax can't emit a broken remote img.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: fix drift after removing the batch-scoring hook and text formatter\n\nPoint scraping-domain at html_to_markdown + JobDescription, drop removed useJobMatchScores frontend refs from API.md and adr-017, and reword the scrape_update_description comment.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* style: remove unused path import in the flatpak manifest test\n\nDead import flagged by clippy -D warnings in CI (incremental cache masked it locally).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(chrome): make linux flatpak detection hermetic with an injectable home\n\nExtract probe_flatpak(home) so the flatpak branch is tested with a temp home instead of asserting detect_*, which broke on CI runners shipping a real /usr/bin/google-chrome.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(match): keep underscored tokens and drop stale embedding on description change\n\nCodeRabbit: stop stripping underscores in markdown_to_plain (preserves OPENAI_API_KEY etc.) and invalidate the cached embedding when update_description changes the text.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(linux): scope webkit env to wayland appimage and restore ld_preload on failed re-exec\n\nCodeRabbit: only disable DMABUF/compositing in the AppImage+Wayland case (not all Linux), and roll back LD_PRELOAD and the guard env if re-exec cannot proceed.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(extension): register vivaldi native host and bound flatpak probes with a timeout\n\nCodeRabbit: add Vivaldi native-messaging manifest paths and wrap the flatpak version/info probes in a wait_timeout so a hung flatpak cannot block detection.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(scraping): make resolve host-gate tests hermetic and verify the url-skip branch\n\nCodeRabbit: assert the host gate without reaching a live send(), and exercise the final_url==url skip path.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): handle rejected mutations, sync description cache, and reset scores on resume switch\n\nCodeRabbit: catch rejected save/tailor mutations, sync the postings cache after updateDescription, reset scores on resume switch, guard browserPath, and localize onboarding labels.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(jobs): strengthen autopilot, view-toggle, tooltip, and markdown-image tests\n\nCodeRabbit: real button stub, a view-mode toggle interaction test, a focusable tooltip target, and a stricter empty-alt image assertion.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: enforce thin pointers and fix the alert example after review\n\nCodeRabbit: replace copied literals and thresholds with source pointers across api/architecture/deployment/knowledge docs, and correct the alert example to the real props.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(linux): resolve scope and visibility errors in the linux-gated test build\n\nFully-qualify the serial attribute in mod linux and make restore_preload_env pub(super); both only surfaced on the Linux runner since Windows cfg-excludes mod linux.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(jobs): wait for the observable outcome instead of pumping microtasks\n\nReplace the brittle repeated await Promise.resolve() with waitFor on the notify spy in the save-guard test.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): use a single active-descendant focus model in the jobs split view\n\nCodeRabbit: drop the roving tabindex on options (tabIndex -1) and keep the listbox container as the sole tab stop with aria-activedescendant.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-24T12:15:57+02:00",
          "tree_id": "32214f58173b5448fab40ef6d5ec71390895ac7b",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/bc6c9e8789b39f85c208967163a067d3da6a1f18"
        },
        "date": 1782297369272,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1929746,
            "range": "± 117155",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2590041,
            "range": "± 40338",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 305221,
            "range": "± 10639",
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
          "id": "be378fb5637673ed443dcc0da56a3cea8026c9d5",
          "message": "feat(jobs): linkedin-style jobs page, viewed dwell, show-more dedup (#499)\n\n* feat(jobs): linkedin-style jobs page, viewed dwell, show-more dedup\n\nRedesign the Jobs page toward the LinkedIn jobs UI and fix three issues:\n\n- remove the split-view collapse/expand toggle (drop detailCollapsed); keep\n  the mobile back button (now clears selectedId)\n- move the detail-pane save/tailor action cluster to the header top-right;\n  status badges stay on the left under the title\n- fix show-more duplicate jobs: PostingsCache::add now upserts by id\n  (preserving insertion order) and the JobsPage merge dedups by id\n- mark a job viewed only after a 5s dwell instead of instantly; the opened\n  (external-link) path stays instant\n- dim already-viewed rows in the split list and label them \"Viewed\"\n- widen the list pane, add a source-badge logo slot, a remote pill chip,\n  and an \"about the job\" section label\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): a11y contrast and linkedin-fidelity polish from review\n\nAddress critic findings on the jobs redesign:\n\n- raise dimmed viewed rows to legible tokens: viewed title uses\n  text-muted-foreground and the meta line no longer drops below the\n  contrast floor; \"about the job\" label uses text-muted-foreground\n- bump split-list row height to 68px (estimateSize kept in sync) and the\n  badge gap to gap-3 so the source badge no longer crowds the text\n- drop the redundant \"viewed\" tag from the detail header (the list already\n  dims and labels viewed jobs)\n- remove a dead viewed-ref reset and a redundant ml-auto; extract the\n  repeated status-tag class\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(jobs): cover dedup formula, viewed dim class, and dwell unmount\n\n- update the JobsPage merge pure-function replica to the new single-pass\n  dedup and add an intra-livePostings duplicate case\n- assert the text-muted-foreground viewed-dim class on the list title\n  (present when viewed and not selected, absent when selected)\n- add a dwell-timer unmount-cancel test (no viewed after unmount)\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* style(postings): rustfmt the cache upsert block\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(jobs): fix missing vitest symbol import in dwell-timer test\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore: wire native graphify-mcp server, drop node launcher\n\ngraphifyy now ships a graphify-mcp executable, so .mcp.json calls it directly instead of the scripts/graphify-mcp.mjs launcher.\n\nUpdate .mcp.json.example and DEVELOPMENT.md to match, and delete the obsolete launcher.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore: route agents to graphify mcp tools, grant mcp__graphify\n\nGrant mcp__graphify to all 24 agent tool allowlists so subagents call the MCP server instead of shelling out to the graphify CLI.\n\nSwitch the routing prose to prefer MCP tools when connected (CLI fallback): CLAUDE.md, the token-efficiency contract, the review-*/refactor-module commands, and docs/knowledge pointers.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): address coderabbit review on the jobs redesign\n\nResolve all six CodeRabbit threads on PR #499:\n\n- postings cache: invalidate the cached embedding when add() replaces an\n  existing id, mirroring update_description (a re-streamed posting with\n  changed text no longer reuses a stale vector); covered by a new test\n- JobsSplitView: defer the Back-button list focus to an effect that runs\n  after the list pane is visible, so keyboard arrow-nav resumes on narrow\n  screens (synchronous focus on a still-hidden aside was dropped)\n- dedup test: use two distinct objects sharing an id to prove id-based\n  (not reference-based) deduplication\n- PostingListItem test: assert the aria-hidden viewed marker node directly\n  so a visible-marker regression can't hide behind the sr-only summary\n- session-store test: assert an untouched field survives a setJobs patch\n- docs/DEVELOPMENT.md: point at .mcp.json.example (mcpServers.graphify)\n  instead of copying MCP command literals\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): show viewed/applied/saved in the list instead of match score\n\nReplace the per-row match score with the interaction status, and fix the\nunderlying bug that the list never reflected persisted interactions.\n\n- remove the MatchBand score from PostingListItem and RowMatchScore from\n  PostingRow; the score now lives only in the detail pane\n- join InteractionStore records onto each posting in scrape_list_postings\n  (via a unit-tested attach_interactions helper) so posting.interactions is\n  finally populated — previously the backend returned the raw cache with no\n  interactions and nothing merged them, so viewed/applied/saved never showed\n- usePersistJob now also invalidates the postings list, so the list refetches\n  and the badge appears after the 5s viewed dwell (and applied/saved)\n- tests: detail-pane asserts the score still renders; list asserts it does\n  not; backend asserts the interaction join by job id\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* refactor(postings): map joined interactions to the contract shape\n\nAddress CodeRabbit: attach_interactions built the interactions array via\njson!(InteractionRecord), coupling the scrape_list_postings IPC response to\nthe storage struct — a future storage-only field would leak and drift from\nthe shared JobInteraction contract. Map each record explicitly to the eight\ncontract fields via a small interaction_value() projection, and assert the\nexact key set in the test so growth can't silently leak. No behavior change.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ui(jobs): frame the detail-pane header with a thin border\n\nWrap the whole detail header (title, actions, meta, status badges) in a thin\nrounded border inset with m-3, replacing the bottom-only divider; trim the\nbody top padding so the header margin doesn't double-gap.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* perf(jobs): cache the fetched job description for the session\n\nThe on-demand description resolve inherited the default 5-minute gcTime, so\nre-opening a job after a while re-fetched its description. A description is\nimmutable within a session — set the resolve query's staleTime and gcTime to\nINFINITE (new QUERY_TIMES sentinel) so it is fetched once per URL and served\nfrom cache thereafter.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(jobs): drop stale score mocks from the posting-row test\n\nThe row no longer renders the match score, so remove the now-unused\nrow-match-score and score-to-level mocks; keep the providers mock (the actions\nhook still calls the row match score transitively) with a comment explaining why.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): show interaction badges in the list and polish the list ui\n\n- allPostings now prefers the backend posting copy (which carries the joined\n  interactions and the persisted full description) over the streamed copy, so\n  the viewed/applied/saved badges actually appear in the list\n- the detail pane no longer flashes a loading state when a snippet is already\n  shown; it renders the snippet immediately and resolves the full text silently\n- \"Show more\" keeps the existing list, scroll and selection (the skeleton only\n  shows on a fresh search with no results yet)\n- the selected row uses the sidebar nav active style (rounded inset pill); rows\n  are taller with larger text, and the viewed badge is shown in the detail pane\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(landing): drop .html from the agent-system download link\n\nMatch the site's clean-url convention (index/how-it-works/architecture-map\nalready link to /download); the agent-system page was the lone straggler.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): animate selection and detail entrance, surface-wrap the panes\n\n- selected row uses a shared-layout motion indicator that slides between jobs\n  (square, no radius), like the sidebar\n- the detail pane content fades/slides in on each job switch\n- preserve the user's selected job across re-scrapes/show-more/live prepends —\n  only auto-select when nothing valid is selected\n- wrap the two-pane area in a themed surface card (adapts light/dark)\n- the \"Show more\" button is now primary\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(jobs): cover selection preservation across re-scrapes and show-more\n\nAdd explicit coverage for the simplified auto-select effect: the user's\nselected job is preserved across show-more, live prepends and re-scrapes as\nlong as it stays in the list, and topId is auto-selected only when nothing\nvalid is selected (fresh search or selection filtered out); list mode no-op.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ui(jobs): premium company monograms, type scale, refined selection and header\n\n- new CompanyAvatar derives the monogram from the company name (varied per row,\n  not the always-\"AG\" source) with a deterministic accent color + hairline ring\n- adopt the apple type-scale tokens (caption-strong / body-strong / fine-print)\n  in the list rows and the detail header; lift all 9px status pills to the 12px\n  floor\n- refine the selected row: softer brand fill + a hairline left accent, and a\n  whisper brand-tinted hover instead of the flat grey fill\n- the detail header sits flush in the surface card with a single hairline\n  divider; symmetric body gutters and roomier row padding\n- guard the selection slide animation with reduced-motion\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): opt-in company logos behind a privacy setting\n\nAdd a \"Fetch company logos\" preference (default OFF, v3→v4 migration keeps\nexisting users off). When enabled, CompanyAvatar resolves a logo from Clearbit\n(name→logo via the autocomplete endpoint, cached for the session, fully\ndefensive — any failure falls back to the monogram). CSP gains exactly two\nhosts: img-src logo.clearbit.com and connect-src autocomplete.clearbit.com.\nWith the setting off (the default) no company data leaves the device.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): address review — logo fallback, avatar tokens, a11y, list font\n\n- CompanyAvatar falls back to the monogram when the logo image fails to load\n  (onError), not just when the URL is absent; avatar colors now use design\n  tokens (brand / brand-2 / action-run / action-edit / destructive / muted)\n  instead of raw tailwind palette classes\n- guard the detail-pane entrance animation with reduced motion; aria-hidden the\n  inline \"updating\" hint so it doesn't double-announce with the status region\n- add referrerPolicy=\"no-referrer\" to the shared Image; document the\n  useCompanyLogo renderer-fetch as an intentional opt-in exception\n- revert the list-row font to 13px/11px (the taller row height stays) — the\n  ask was more height, not larger text\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ui(layout): center page content with a max-width on large screens\n\nBump the PageShell default to max-w-6xl 2xl:max-w-7xl (the dashboard's\nultrawide width) and wrap the full-width jobs page in the same centered column\nso its two-pane no longer stretches edge-to-edge on ultrawide monitors. The\nfull-bleed tool/builder pages (settings, resume builder, analyze, ai-generate,\nautopilot, documents) keep their intentional full width.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ui(layout): center the remaining tool pages on large screens\n\nApply the same mx-auto max-w-6xl 2xl:max-w-7xl centering to the settings,\nai-generate, analyze, autopilot, resume-builder and documents pages so every\nroute stops stretching edge-to-edge on ultrawide. Each wrapper carries the\npage's existing flex/height/overflow chain so the two-pane splits, builder\nstages and independent scroll regions are preserved; modal/overlay layers\nstay outside the centered column.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): stop escaping markdown in plain-text job descriptions\n\nhtml_to_markdown ran every description through htmd, which escapes markdown\nspecial chars in text nodes — so plain-text/already-markdown descriptions\n(common from Adzuna, e.g. an SAP ad) had their ** emphasis escaped to \\*\\*,\nrendering as literal asterisks, and their newlines collapsed into one wall of\ntext. Only run htmd when the input actually contains HTML tags; otherwise pass\nthe trimmed text through so existing ** markers and paragraph breaks survive.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ui(jobs): let the job description use the full detail-pane width\n\nDrop max-w-prose from the detail-pane JobDescription so it fills the pane\ninstead of leaving the right half empty; the page-level centering already\nbounds the overall line length.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore(deps): bump pnpm/action-setup from 6.0.8 to 6.0.9\n\nBumps [pnpm/action-setup](https://github.com/pnpm/action-setup) from 6.0.8 to 6.0.9.\n- [Release notes](https://github.com/pnpm/action-setup/releases)\n- [Commits](https://github.com/pnpm/action-setup/compare/0e279bb959325dab635dd2c09392533439d90093...0ebf47130e4866e96fce0953f49152a61190b271)\n\n---\nupdated-dependencies:\n- dependency-name: pnpm/action-setup\n  dependency-version: 6.0.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\n\n* chore(deps): bump actions/labeler from 5.0.0 to 6.1.0\n\nBumps [actions/labeler](https://github.com/actions/labeler) from 5.0.0 to 6.1.0.\n- [Release notes](https://github.com/actions/labeler/releases)\n- [Commits](https://github.com/actions/labeler/compare/8558fd74291d67161a8a78ce36a881fa63b766a9...f27b608878404679385c85cfa523b85ccb86e213)\n\n---\nupdated-dependencies:\n- dependency-name: actions/labeler\n  dependency-version: 6.1.0\n  dependency-type: direct:production\n  update-type: version-update:semver-major\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\n\n* chore(deps): bump actions/checkout from 6.0.3 to 7.0.0\n\nBumps [actions/checkout](https://github.com/actions/checkout) from 6.0.3 to 7.0.0.\n- [Release notes](https://github.com/actions/checkout/releases)\n- [Changelog](https://github.com/actions/checkout/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/actions/checkout/compare/df4cb1c069e1874edd31b4311f1884172cec0e10...9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0)\n\n---\nupdated-dependencies:\n- dependency-name: actions/checkout\n  dependency-version: 7.0.0\n  dependency-type: direct:production\n  update-type: version-update:semver-major\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\n\n* chore(deps): bump softprops/action-gh-release from 3.0.0 to 3.0.1\n\nBumps [softprops/action-gh-release](https://github.com/softprops/action-gh-release) from 3.0.0 to 3.0.1.\n- [Release notes](https://github.com/softprops/action-gh-release/releases)\n- [Changelog](https://github.com/softprops/action-gh-release/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/softprops/action-gh-release/compare/b4309332981a82ec1c5618f44dd2e27cc8bfbfda...718ea10b132b3b2eba29c1007bb80653f286566b)\n\n---\nupdated-dependencies:\n- dependency-name: softprops/action-gh-release\n  dependency-version: 3.0.1\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\n\n* chore(deps): bump anthropics/claude-code-action from 1.0.150 to 1.0.156\n\nBumps [anthropics/claude-code-action](https://github.com/anthropics/claude-code-action) from 1.0.150 to 1.0.156.\n- [Release notes](https://github.com/anthropics/claude-code-action/releases)\n- [Commits](https://github.com/anthropics/claude-code-action/compare/9dd8b95a392eb34b6f5fb56cf5a64cb735912d4b...74eedf1a1892082d619c3edb66b9402da6520e7f)\n\n---\nupdated-dependencies:\n- dependency-name: anthropics/claude-code-action\n  dependency-version: 1.0.156\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\n\n* fix(jobs): address coderabbit — validate interaction type, bound caches\n\n- postings::interaction_value clamps the persisted interaction_type to the\n  JobInteraction union (unknown -> \"viewed\") so a corrupt on-disk value can't\n  break the cross-layer IPC contract; covered by a new test\n- give the resolved-description and company-logo react-query caches a finite\n  gcTime (TEN_MIN) instead of Infinity so per-url/per-company entries are\n  evicted once inactive (bounds memory); staleTime stays infinite so cached\n  entries still don't re-fetch while active\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(jobs): address coderabbit round 2\n\n- reset CompanyAvatar's logoFailed flag when the logo url changes so a new\n  company after a failed logo gets a fresh attempt (real bug, + rerender test)\n- avoid a bare unwrap on the HTML-tag regex (expect with a message) and\n  tighten the plain-text markdown tests to exact-output assertions\n- make the company-logo failure tests assert the real settled outcome instead\n  of a trivially-true predicate\n- extract the postings merge into a shared mergePostings helper used by both\n  JobsPage and its test (no more drifting inline replica)\n- dry the remote pill to statusTagCls, trim stale justFinished comments, and\n  run the preferences-store migrations in ascending version order\n- pin the claude-code-action annotation to the specific bumped version\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
          "timestamp": "2026-06-25T00:04:31+02:00",
          "tree_id": "0d89854d8beea32fec5e0a7b575a984cfeec3ab8",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/be378fb5637673ed443dcc0da56a3cea8026c9d5"
        },
        "date": 1782339812413,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1889802,
            "range": "± 66593",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2483269,
            "range": "± 51525",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 284074,
            "range": "± 12400",
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
          "id": "5bbc5c7a69746abacd488481f32bc0a222a33df9",
          "message": "feat: github project import, job-summary fixes, and answer rewrite (#500)\n\n* feat(resume): add github repo fetch ipc for project import\n\nAdds the github_import_repos IPC capability: fetch a user's public repos,\ndrop forks, sort by stars (top 30), return them to the renderer. First\nstep of the resume-builder GitHub project import (chunks B/C add AI bullets\nand the UI).\n\n- profile_import/github.rs: fetch_repos with a hardened SSRF guard\n  (host-gated parse, server-constructed api.github.com URL, username\n  validated against GitHub's rule); routed through scraping::http::fetch_text\n  for the 8MB body cap, per-host rate limiter and a 20s timeout.\n- scraping/http: opt-in FetchOptions.timeout (default None, backward-safe).\n- Full IPC wiring: shared contract, tauri-client namespace, service hook,\n  query key, mock-client stub.\n- 26 Rust unit tests (URL/username parsing, SSRF rejection, filter/sort,\n  serde shape); no network in tests.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(resume): import github projects into the resume builder\n\nAdds an 'Import from GitHub' action to the resume builder's Projects step:\nfetch the user's public repos (chunk A), let them multi-select, generate\nAI resume bullets per repo, and append them as project entries.\n\n- packages/prompts: github-projects prompt builder + lenient parser; repo\n  text fenced as untrusted (ADR-010 pattern) and the AI never sees/writes\n  URLs.\n- lib/generate: generateGitHubProjects streams via the shared pipeline (no\n  new IPC, every provider), matches AI bullets back to repos by de-slugged\n  name so a reordered response can't cross a bullet onto the wrong repo\n  link, and falls back to the raw repo description offline.\n- GitHubImportModal: username prefilled from the contact profile, fetch/\n  empty/error/generating states, accessible dialog + live regions, design\n  tokens; failure keeps the modal open with the selection preserved.\n- Tests: SSRF-rejection + status-mapping (Rust), name-match link guard +\n  fallback (generation), modal flows incl. async prefill + abort.\n- Removes an unused github query-key factory (the hook is a mutation).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs(resume): document github projects import\n\nThin-pointer docs for the GitHub import feature: github.importRepos in\ndocs/API.md, a knowledge pointer, and the README index row.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(resume): resolve pre-pr review on github import\n\nAddress the internal pre-PR gate: cargo fmt wrap on the new test asserts,\na clippy doc_lazy_continuation, the namespace error-unwrap throwing on an\nundefined result (vitest suite exit), the cancel-during-generation guard\n(abort no longer appends fallback entries), and typed test accessors so the\nmodal test passes real tsc.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(documents): show model picker and regenerate job summary on language change\n\nThe job-summary step now renders the shared ModelSelector (same preferences\nstore as StepModel, no second model state), and changing the output language\nre-runs the summary in the new language (guarded: only when a summary already\nexists, aborts any in-flight run, no loop).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(documents): rewrite application answers with ai\n\nAdds a Rewrite-with-AI affordance to each generated application answer,\nreusing RewritePopover with a new 'application-answer' RewriteDocType\n(prose voice, same grounding/no-fabrication contract). Accepted rewrites\npersist through the existing save path. Includes a scoped eslint test-file\noverride (matches the Storybook pattern) and en/de strings.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: list cli agents in tagline and open the readme toc by default\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore(agents): harden pr-reviewer with coderabbit-derived rules\n\nAdds rules distilled from CodeRabbit findings on the last reviewed PRs:\nstatic-init (LazyLock) panics, json!(struct) IPC contract drift, React\nQuery gcTime/over-broad invalidation, and tautological-assertion patterns\n(boolean-returning waitFor, same-object dedup, partial-patch asserts).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(documents): surface rewrite save errors and cover answer persistence\n\nacceptRewrite now closes the popover and notifies on a failed re-save\n(was silently swallowed). Adds useApplicationAnswers updateAnswer tests\n(no-op before first generate, full answer-set persisted, untouched answers\nsurvive) and removes a stale 'covered elsewhere' comment.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(documents): type vi.fn mocks so tsc passes uncached\n\nUse the vitest 3 function-type generic for updateAnswer/toggle/addCustom\nmocks and drop a non-tuple spread, so the real (uncached) tsc that CI and\npre-push run passes — the scoped typecheck had been returning a stale-cache\npass.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(shared): add github to expected ipc namespaces\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(prompts): replace polynomial regexes with linear strips\n\nResolves CodeQL #236 (js/polynomial-redos, HIGH): the <think> strip becomes\nan indexOf-based linear scan and the markdown bold/italic strips use a bounded\n[^*]+ class, so no lazy span between identical delimiters runs on model output.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(resume): surface github ipc failures and document ranking window\n\nimportRepos now throws on an unexpected/undefined result instead of masking it\nas an empty list; the generic namespaces test mocks a valid github envelope.\nDoc comments state the result is top-30-by-stars among the 100 most-recently-\nupdated repos (deliberate v1 scope).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(documents): guard job-summary mount and revert failed answer rewrite\n\nThe language-regen effect now skips its mount run so a restored summary isn't\nclobbered in the default locale. A failed answer-rewrite save reverts the\noptimistic local edit (revertAnswer, no re-save) and toasts. The answers ref\nsync moves to a useEffect so the setAnswers updater stays pure under StrictMode.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(resume): reset github import modal on reopen and complete plural keys\n\nThe always-mounted modal now clears repos/selection/fetch state when reopened\n(no stale list or duplicate appends). Adds the missing addSelected_one and\ndrops the redundant bare plural keys so i18next v4 resolves _one/_other.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: fix api heading level, drop test counts, correct sibling path\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(prompts): scan original string when stripping think blocks\n\ntoLowerCase() can change length (e.g. İ), so offsets from a lowercased copy\ndrift from the source and mis-cut the span. Match the ASCII <think>/</think>\ntags case-insensitively against original-string slices instead.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(resume): decouple modal reset from prefill and harden ipc envelope check\n\nThe modal now resets transient state only on an open transition (not on an\nasync prefill update that would wipe an in-progress fetch); the username seed\nis a separate, guarded effect. The github IPC client checks the result is a\nnon-null object before the 'in' check so a primitive response can't throw.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(documents): guard rewrite revert against stale save failures\n\nRevert only when the optimistic value is still current, so a later accepted\nrewrite isn't clobbered by an earlier save's stale rejection. Tests now mirror\nthe production optimistic prop update (the prior mock masked the real flow).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: make github import test pointers status-free\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(documents): make rewrite rollback guard timing-independent\n\nTrack the pending rewrite in a synchronously-set ref instead of the\nrender-synced answersRef, so a fast save rejection (before the optimistic\nre-render flushes) still reverts correctly while a superseding rewrite skips.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: drop gitignored scratch link from github import doc\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-25T05:54:51+02:00",
          "tree_id": "c89787d66a81e0372d106ef912cd6006c06f6a78",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/5bbc5c7a69746abacd488481f32bc0a222a33df9"
        },
        "date": 1782360211936,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1906363,
            "range": "± 64176",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2523928,
            "range": "± 54869",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 288777,
            "range": "± 9855",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "saeedkolivand1997@gmail.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "committer": {
            "email": "saeedkolivand1997@gmail.com",
            "name": "Saeed Kolivand",
            "username": "saeedkolivand"
          },
          "distinct": true,
          "id": "7702cf828aefd8dca5b92cbf84b439ef7b262a26",
          "message": "chore(branding): add product hunt launch assets\n\nGallery images (1270x760 @2x), teal thumbnail, maker first comment, and a\nreproducible headless-chrome generator under branding/marketing. Add a\nscoped eslint node-globals override for branding/**/*.mjs so the generator\nlints clean.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-26T01:04:41+02:00",
          "tree_id": "104ed507ee89cf6b40b994b110681009590fff80",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7702cf828aefd8dca5b92cbf84b439ef7b262a26"
        },
        "date": 1782429375253,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1878629,
            "range": "± 43122",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2506529,
            "range": "± 32227",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 289525,
            "range": "± 6001",
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
          "id": "f622c5c08421375dd11b5af79a5cac28122eb751",
          "message": "fix: prevent cover-letter extractor panic on multibyte job ads (#504)\n\nThe job-ad metadata extractor computed a byte offset by searching one\nstring and then sliced a different string with it, panicking on a\nnon-char-boundary when emoji/multibyte text shifted the two apart (the\nreported crash at extractor.rs inside a leading emoji during cover-letter\ngeneration).\n\nReplace the cross-string slicing with an ASCII case-insensitive search\nthat returns an offset valid in the searched string itself, so every\nslice stays on a char boundary and extracted company/role keep their\noriginal casing. Add regression tests whose inputs panic against the\npre-fix code.\n\nAlso install a best-effort panic hook that appends crash payload,\nlocation and backtrace to a local crashes.log in the app data dir\n(chaining the default hook), so future panics are diagnosable.\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-30T01:12:08+02:00",
          "tree_id": "347806f9d0bb14fdc2902366c6c9f4193b081051",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f622c5c08421375dd11b5af79a5cac28122eb751"
        },
        "date": 1782775838922,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1855172,
            "range": "± 41016",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2450229,
            "range": "± 29587",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 281011,
            "range": "± 3989",
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
          "id": "87f0b97c4860479723c8df4a6f31f34edd48b0cb",
          "message": "feat: export a redacted diagnostics bundle for crash reports (#509)\n\n* feat: export a redacted diagnostics bundle for crash reports\n\nZips a strict allowlist (system-info.txt, redacted crashes.log, redacted logs/) via a native save dialog, then reveals the file to attach to a GitHub issue.\n\nThe database, documents, API keys, and resume/job data are excluded by construction.\n\nBundled text runs through redact_token (now covering email and JSON-shaped secrets, skipping symlinks) so no PII reaches a public issue.\n\nAlso fixes the previously broken support_export_diagnostics command (name/dir mismatch). See ADR-027 for the privacy boundary.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: source diagnostics logs from app log dir and harden export\n\nLogs are written by tauri-plugin-log to app_log_dir(), not app_data_dir()/logs, so the bundle shipped without logs on Windows and macOS.\n\nSource logs from app_log_dir() and decode bytes lossily so a corrupt log byte can't abort the export.\n\nFrontend: a reveal failure no longer flips a successful export to an error; toasts are localized (en+de) and no longer surface raw error strings.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: address coderabbit review on diagnostics bundle\n\nReject save destinations that alias crashes.log or a log file (which would truncate the source before reading) and exclude the dest from the log scan.\n\nModel the export contract as a discriminated union so callers can type the failure path's error field.\n\nUse local date for the default filename, move save() inside the error boundary, and log only the error name (never a path) on failure.\n\nSync the en/de bundle summary to the real three-entry allowlist; keep the security-rules pointer thin; correct the README redaction wording.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-30T05:45:28+02:00",
          "tree_id": "f4438781499f93d971946fb453b0d942c96eee24",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/87f0b97c4860479723c8df4a6f31f34edd48b0cb"
        },
        "date": 1782791639498,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1939834,
            "range": "± 61031",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2552208,
            "range": "± 20419",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 288790,
            "range": "± 2257",
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
          "id": "075e24010ea48ba2105a40e8c96c5022ed63c63e",
          "message": "feat: add apify linkedin aggregator provider (opt-in) (#510)\n\n* wip: apify linkedin provider (paused for #509 coderabbit)\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore: apply cargo fmt to apify linkedin provider\n\nPre-format before push so pre-push cargo fmt --check passes.\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>\n\n* feat: add apify linkedin settings ui and paid-provider cost controls\n\nSettings: Apify token field (keychain) + opt-in 'Include LinkedIn (Apify)' toggle, optional actor-id override, and a cost/latency notice, backed by a use-scraping-settings plugin-store hook.\n\nCost safety: retries:0 on the billed non-idempotent run, mid-flight cancellation via tokio::select!, and platform-enforced maxItems/maxTotalChargeUsd caps; actor-id is grammar-validated.\n\nDocs: privacy disclosure for Apify plus the RapidAPI/JSearch correction, and ADR-028 for the additive-merge + cost-control model.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: pin apify cost invariants to the real code path\n\nExtract build_apify_endpoint + an APIFY_RETRIES constant so the cost-cap and retries:0 tests assert on production code, not local copies; fix canonical_url to require a dot boundary on linkedin.com.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: address coderabbit review on apify provider\n\nValidate Apify job URLs (https + linkedin.com host; numeric-only ids) before emitting, so a drifting actor can't inject non-LinkedIn URLs into JobPosting.\n\nSkip the paid LinkedIn run when the free providers already fill the requested count; otherwise cap the fetch to the remaining slots (min of cap and remaining).\n\nDedupe with host-only lowercasing so path/query case is preserved; assert retries:0 and the endpoint via the real builders; tighten the settings test and ADR-028 wording.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: reference apify default actor symbol in adr-028\n\nPoint at APIFY_DEFAULT_ACTOR instead of copying the actor-id literal so the ADR can't drift.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-06-30T08:16:06+02:00",
          "tree_id": "f8df146615edfa13630932f23d896e48cf1ad014",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/075e24010ea48ba2105a40e8c96c5022ed63c63e"
        },
        "date": 1782800688300,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1931882,
            "range": "± 19175",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2558340,
            "range": "± 36056",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 290524,
            "range": "± 5608",
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
          "id": "04e6e78aed29a22b3f1d370a91823bd89cdf1ef9",
          "message": "refactor: rename apps/tauri to apps/desktop and @ajh/tauri to @ajh/desktop (#512)\n\n* refactor: rename apps/tauri to apps/desktop and @ajh/tauri to @ajh/desktop\n\nRename the desktop app directory and its private @ajh package identity. All path and pnpm --filter references updated across configs, CI workflows, docs, renderer, and Rust source.\n\nAlso drop apps/desktop from the check:agent-system stale-architecture denylist (it was the Electron-era dir name, now the current Tauri app dir). Lockfile regenerated; no behavior change.\n\nVerified: turbo typecheck, eslint config resolution, check:agent-system, check:landing-drift, and a clean cargo build.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* refactor: fix over-replaced tauri-apps url and rename app label to app/desktop\n\nAddresses PR #512 review. The literal apps/tauri rename also hit the substring\ninside tauri-apps/tauri, corrupting the Tauri security-advisory URL in\ntauri-standards (HIGH). Reverted that URL back to tauri-apps/tauri.\n\nAlso renames the labeler.yml key app/tauri to app/desktop (matching the\napp/extension convention). The GitHub label plus stale rust and frontend label\ndescriptions were updated to apps/desktop out of band.\n\nRust crate name ajh-tauri is left as-is. It is internal only and the\nversion-sync script still matches it.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-01T02:12:18+02:00",
          "tree_id": "98f9b44e9da3c4ae85dbf3e0a9416df6162add38",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/04e6e78aed29a22b3f1d370a91823bd89cdf1ef9"
        },
        "date": 1782865913456,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1966911,
            "range": "± 66249",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2618202,
            "range": "± 59787",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 308460,
            "range": "± 6425",
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
          "id": "29fe2e5770ed74fbe21ae1e7dee00738e74dbded",
          "message": "feat: add pinpoint, rippling, breezy, bamboohr ats board scrapers (#513)\n\n* feat: add pinpoint, rippling, breezy, bamboohr ats board scrapers\n\nAdd four new company-scoped ATS job-board scrapers, taking the registry\nfrom 16 to 20 active boards. Each is a zero-auth public JSON endpoint keyed\nby company slug, cloning the existing greenhouse/personio company-scoped\npattern: DNS-label slug SSRF guard, per-company fanout with partial-failure\nisolation, and per-company URL-based dedup.\n\nEndpoints (re-verify live before relying on shape):\n- pinpoint:  https://{slug}.pinpointhq.com/postings.json\n- rippling:  https://api.rippling.com/platform/api/ats/v1/board/{slug}/jobs\n- breezy:    https://{slug}.breezy.hr/json\n- bamboohr:  https://{slug}.bamboohr.com/careers/list\n\nBambooHR job ids are tenant-local integers, so JobPosting.id is namespaced\nas bamboohr:{company}:{id} (mirrors personio::make_job_id) to prevent\ncross-tenant collisions overwriting results in PostingsCache. Pinpoint/breezy\nposting URLs (the dedup key) reject non-https and userinfo-bearing links.\n\nRegistered in SCRAPERS + BOARD_IDS + en/de board-name i18n; 95+ fixture\ntests per board (happy path, empty, malformed-row drop, slug guard, the\nworkLocation/id shape variants, and the cross-tenant id-collision regression).\nEndpoint reconnaissance ported from santifer/career-ops (MIT), attributed in\neach module header.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: list new ats boards in companies schema comment\n\nAdd pinpoint, rippling, breezy, bamboohr to the ScrapeBoardsRequestSchema\ncompanies comment (stale after 16→20). Comment-only; no logic change.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-01T18:31:04+02:00",
          "tree_id": "2120d421c4c7c7c6e7a0c33fb060371dd946a49b",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/29fe2e5770ed74fbe21ae1e7dee00738e74dbded"
        },
        "date": 1782924784792,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1886075,
            "range": "± 104375",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2531590,
            "range": "± 34478",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 288375,
            "range": "± 6140",
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
          "id": "32e1dd316215b4f44410df5de77e260922c4820a",
          "message": "feat: add the muse aggregator job board scraper (#529)\n\n* feat: add the muse aggregator job board scraper\n\nAdd The Muse (themuse) as a keyword aggregator scraper, taking the registry\nfrom 20 to 21 active boards. Zero-auth public JSON feed at\nwww.themuse.com/api/public/jobs (host-locked, fixed URL — no user input in\nthe request path). The Muse has no server-side keyword search, so query and\nlocation are applied client-side after a bounded page fetch.\n\nModeled on the arbeitnow board: 0-indexed pagination bounded by\ninput.pages.clamp(1, 5) and the response page_count, per-page cancellation,\nand partial-result degradation (page-0 error propagates, later-page errors\nlog and break keeping earlier results). A warn fires when page_count is\nabsent on a full first page, so an unverified-endpoint shape drift surfaces\nin logs instead of silently truncating.\n\nThe client-side filter is extracted to matches_filters() so search() and the\ntests exercise the same code path. 21 fixture tests cover parse mapping,\ncompany/location fallbacks, malformed-row drop, url-as-id, and the query and\nlocation filters. Endpoint reconnaissance ported from santifer/career-ops\n(MIT), attributed in the module header; marked unverified pending a live\nsmoke test.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: warn on themuse none-page break and emit none for empty location\n\nLog a warn when a later page returns None (non-2xx or shape mismatch)\ninstead of breaking silently, so endpoint drift surfaces in logs. Map an\nabsent/blank The Muse location to None rather than Some(\"\") to match the\nfleet convention (arbeitnow); the client-side location filter is unaffected.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: treat themuse page-0 cancellation as a clean stop, fix doc drift\n\nAddresses CodeRabbit review on PR #529: fetch_json returns\nAppError::Cancelled on an in-flight cancel, which previously bubbled\nas a page-0 board error instead of Ok(vec![]). Also corrects doc\ndrift introduced by the earlier board-count bump: docs/API.md listed\nthe non-canonical `weworkremotely` instead of BOARD_IDS' `wwr`, the\nscraping-domain doc double-counted `aggregator` in its board totals,\nunderstated the pagination bound (missing `input.pages`), and\nmisclassified The Muse's source pointer under company-scoped boards\ninstead of its own keyword-aggregator section.\n\nThe max_pages-vs-total_pages progress-denominator finding is\ndeliberately deferred (pre-existing behavior shared with Arbeitnow;\ntracked as a fleet-wide follow-up, not a one-board local fix).\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-01T20:27:42+02:00",
          "tree_id": "431e6de2e86d4a820c459c2491c62d20fb9927dd",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/32e1dd316215b4f44410df5de77e260922c4820a"
        },
        "date": 1782931009357,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1898801,
            "range": "± 51627",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2498243,
            "range": "± 13557",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 286082,
            "range": "± 9401",
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
          "id": "65bf53014ccffcef7611d56444d3496f413c23f7",
          "message": "feat: add job trust / ghost-job validator with badge (#530)\n\nScore every scraped job for scam/ghost-posting signals and surface a badge\non low-trust listings. A pure Rust validator (scraping/trust) starts each job\nat 100 and subtracts for a missing/invalid apply url, a url-shortener host, or\na company-name/domain mismatch (skipped for known ATS + aggregator hosts,\nincluding api.adzuna.com), then classifies High/Medium/Low. Enrich, never drop.\n\nAttached at the pipeline's single stream funnel and the url-resolve path, and\nin the Autopilot found-jobs projection (build_found_job), so every renderer\nsurface carries it; merge_found_jobs now also carries trust across a resurfaced\njob. Exposed via a trust field on the shared JobPosting/AutopilotFoundJob\ncontract (serde-flatten). The renderer TrustBadge shows only for Medium/Low\n(High = trusted, no badge), reuses the MatchBand warning/error color language,\nexposes the flags via a keyboard-reachable HoverPopover, and uses an opaque\nfill on selected rows to stay AA-contrast over the row gradient.\n\nPorted from santifer/career-ops (_trust-validator.mjs, MIT), attributed in the\nmodule header. The company/host match is an intentionally unanchored substring\nheuristic for V1 (documented, non-gating). Covered by 16 Rust tests (scoring,\nallowlist incl. adzuna, label-boundary anchoring, attach json shape, the real\nbuild_found_job + merge projections) and 12 TrustBadge tests (gating, resolved\ni18n labels, key-drift guard, interactive/strong paths).\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-01T22:28:41+02:00",
          "tree_id": "8b79349cb74a953ce01a72ed7f71fa0ca62d6084",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/65bf53014ccffcef7611d56444d3496f413c23f7"
        },
        "date": 1782938225609,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1890140,
            "range": "± 85847",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2500918,
            "range": "± 40460",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 285378,
            "range": "± 4030",
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
          "id": "75f633e5848c1921386748375e299b0e56052ba0",
          "message": "fix: harden rippling scraper against a single malformed row (#531)\n\n* fix: harden rippling scraper against a single malformed row\n\nRippling deserialized its jobs response as an atomic Vec<RpJob> where uuid\nis a required String, so one row missing uuid (or with an oddly typed\nworkLocation) failed the whole deserialize and silently returned zero jobs\nfor that company. Fetch the array as Vec<serde_json::Value> and deserialize\nper row via a new rows_to_jobs(), skipping bad rows instead of dropping the\nwhole batch. The uuid requirement and ats.rippling.com URL host-lock are\nunchanged, so a surviving row still passes the same guards.\n\nRippling-only: pinpoint/breezy/bamboohr rows are all-Option and already\ntolerate missing fields. All 5 new boards were live-verified today against\nreal tenants with zero endpoint/shape drift.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: warn when rippling drops malformed rows\n\nPer-row skipping silenced whole-batch schema drift: if every row failed\nto deserialize the array still parsed and search counted a successful\nfetch, so a total drift returned empty with only per-row debug logs.\nEmit a warn-level summary when any rows are skipped, restoring detection\nof the silent zero-jobs condition at batch scope. Addresses CodeRabbit.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-02T08:08:43+02:00",
          "tree_id": "5af1afb353068341a17012e11a373255a1808720",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/75f633e5848c1921386748375e299b0e56052ba0"
        },
        "date": 1782973029728,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1866480,
            "range": "± 70179",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2498426,
            "range": "± 21637",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 285178,
            "range": "± 2733",
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
          "id": "a2d3e0be5838dcc45e4215005a63baafd951b54a",
          "message": "ci: harden pipeline gates, review gate, and fleet token efficiency (#534)\n\n* ci: harden pipeline gates, deploy-key isolation, and the review gate\n\nCI: ci-ok now requires success from changes/dependencies (+ dependency-review\non prs) so an install or advisory failure can no longer false-green the only\nrequired check; the benchmark job's release deploy key is event-gated away\nfrom pr runs (pr leg checks out keyless with persist-credentials off);\nsemantic-release extra_plugins exact-pinned; two template-injection fixes\n(github.ref -> $GITHUB_REF); zizmor now blocking at high severity with a\nscoped, documented cache-poisoning suppression for the dispatch-only\nrelease workflow; dependabot gains a cargo ecosystem, minor+patch grouping,\nand a 2-day cooldown; pnpm minimumReleaseAge committed explicitly (1440).\n\nAgent system: review-gate fixes — **/ globs now match root files (root\nmanifests route to the security reviewer; root lockfile stays skiplisted),\nuntracked files are diffed into the review, empty reviewer output no longer\ncaches hunks as reviewed, the security reviewer gets a reserved critic slot\nand highRisk derives from matched globs, checklist slice 1600->5000, review\ncache capped at 2000 lines; review-routes pruned of dead globs and extended\n(translations/test-ids/tauri-client/scripts/.github owners; limits+commands\nsecurity globs); check-agent-system gains reverse drift checks (glob\nprefixes must exist; referenced agents must have files); stale dir\nreferences cleaned from four agent prompts; CLAUDE.md pre-pr gate points at\n/review-security.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* refactor: token-efficiency pass for the agent fleet\n\n- handoff template: mandatory <=2K 'current state' header every stage reads;\n  append-only log below read only by project-steward (token-efficiency +\n  author-contract updated to match)\n- author contract: hard codegraph-first rule before any raw grep/read\n- dedupe the 'strict enforcement' boilerplate from 16 agent files into one\n  canonical token-efficiency section; agents keep only a pointer + their\n  domain-specific high examples (net -11 lines; also removes a stale\n  pnpm -F validation contradiction from frontend-author)\n- claude.md: critic count scales with risk (gate-only / one critic / full\n  trio incl. security)\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* build: ignore quick-xml rustsec advisories (no transitive fix path)\n\nrustsec-2026-0194 (quadratic duplicate-attribute check) and rustsec-2026-0195\n(unbounded nsreader namespace allocation) — both DoS-class, published as a\nbatch against quick-xml <0.41. All three locked versions are transitive pins\n(docx-rs, feed-rs + tauri-winrt-notification, citationberg/typst) and the fix\nis a semver-major none of them accept yet. Exposure bounded: own generated\ncontent everywhere except board RSS, which the shared fetch layer caps at\n8 MB. Remove when upstreams bump to >=0.41.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-02T11:06:17+02:00",
          "tree_id": "12ceba19776ab93c73b1ae5ff47e210c8e18d201",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/a2d3e0be5838dcc45e4215005a63baafd951b54a"
        },
        "date": 1782984332783,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1891094,
            "range": "± 62106",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2566328,
            "range": "± 65361",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 293085,
            "range": "± 6343",
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
          "id": "47111d9a085da96c74c64ce1366f0bbd2dc812ac",
          "message": "feat: scraping hardening + workable and comeet boards (21 to 23) (#535)\n\n* refactor: dedupe board helpers, harden redirect ssrf, fix muse progress + trust stop-words\n\n- DRY: extract normalize_companies + is_https_url into scraping/boards/common.rs; drop 9 copies\n- SSRF: centralize a redirect-target guard at the net/http.rs base_builder chokepoint\n  (cap 10 hops, reject non-http(s) + private/loopback/link-local IP-literal targets via\n  crate::net::ssrf::is_safe_public_host); get_guarded's Policy::none path unaffected\n- The Muse: progress denominator = total_pages.min(max_pages) so it reaches 1.0 on short feeds\n- trust: skip stop-words (the/inc/llc/ltd/corp/gmbh) in company_matches_host per-word match\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat: add workable and comeet job boards\n\n- workable: zero-auth company-scoped board on the live-verified widget API\n  (apply.workable.com/api/v1/widget/accounts/{slug}?details=true), per-row\n  lenient deserialize, host-locked urls, /j/{shortcode} fallback, 21 tests\n- comeet: credentialed board (company-uid + api token via os keyring, apify\n  pattern, no new ipc commands), unconfigured -> empty, response shape\n  flagged for live-cred verification, 18 tests\n- redact reqwest transport errors at the fetch chokepoint (without_url) so\n  query-string secrets (comeet token, adzuna app_key) never reach\n  BoardScrapeSummary.error / logs, with a regression test\n- make the redirect-ssrf wiremock test target a reachable route so it fails\n  if the policy is ever unwired\n- registry 21 -> 23 boards; BOARD_IDS, allowlist, en/de i18n, docs updated\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ui: add comeet credential fields to aggregator keys settings\n\nTwo AggregatorKeyField rows (company uid + api token) after the apify\nsection, same divider pattern; no enable toggle — the board activates when\nboth creds are present. Extends the existing test suite (field labels,\nsave wiring, count asserts 4->6) and rescopes the actor-id save test off\nits fragile trailing-index button lookup.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: drive comeet credential-gate tests via block_on to satisfy strict clippy\n\nThe pre-push gate's clippy variant (--all-targets -D warnings) denies\nawait_holding_lock: the three credential-gate tests held the std keyring\nMutexGuard across search().await. Convert them to sync #[test] fns that\nhold the guard while a current-thread runtime block_on drives the async\nbody — same serialization, no allow attribute.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* build: ignore quick-xml rustsec advisories (no transitive fix path)\n\nrustsec-2026-0194 (quadratic duplicate-attribute check) and rustsec-2026-0195\n(unbounded nsreader namespace allocation) — both DoS-class, published as a\nbatch against quick-xml <0.41. All three locked versions are transitive pins\n(docx-rs, feed-rs + tauri-winrt-notification, citationberg/typst) and the fix\nis a semver-major none of them accept yet. Exposure bounded: own generated\ncontent everywhere except board RSS, which the shared fetch layer caps at\n8 MB. Remove when upstreams bump to >=0.41.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: collapse case-variant workable slugs and descope positional test lookups\n\n- workable: lowercase companies before normalize_companies so case-only\n  variants dedupe to one outbound request (+2 tests)\n- comeet settings test: scope the save assertion to the field's own label\n  row instead of trailing array indices (same shape as the actor-id fix)\n\nAddresses the two medium findings from the claude review on #535.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: address coderabbit review on scraping boards\n\n- reject userinfo in workable job urls (spoof@apply.workable.com passed the\n  host-lock); the /j/{shortcode} fallback flows through the same guard (+2 tests)\n- extract the byte-identical dns-label slug validator shared by bamboohr/\n  breezy/pinpoint into boards/common.rs; fix the common.rs doc claim that all\n  validators differ by design\n- engine catalog test asserts workable/comeet ids directly, not just the count\n- move matches_filters (themuse + comeet) into boards/common.rs\n- deny.toml: correct the quick-xml note (patched in 0.39.4; still no\n  semver-compatible path for the 0.36/0.37/0.38 transitive pins)\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-02T11:51:00+02:00",
          "tree_id": "bd380ca3838d151c93907cc810fd8f02237dc5dd",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/47111d9a085da96c74c64ce1366f0bbd2dc812ac"
        },
        "date": 1782987001452,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1888700,
            "range": "± 74866",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2501033,
            "range": "± 48749",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 283252,
            "range": "± 2349",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "75a6be748d71e918fc8d88f572c923c36ed71e3d",
          "message": "chore: bump lopdf from 0.42.0 to 0.43.0 in /apps/desktop/src-tauri (#536)\n\nBumps [lopdf](https://github.com/J-F-Liu/lopdf) from 0.42.0 to 0.43.0.\n- [Release notes](https://github.com/J-F-Liu/lopdf/releases)\n- [Changelog](https://github.com/J-F-Liu/lopdf/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/J-F-Liu/lopdf/compare/v0.42.0...v0.43.0)\n\n---\nupdated-dependencies:\n- dependency-name: lopdf\n  dependency-version: 0.43.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>\nCo-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>\nCo-authored-by: Saeed Kolivand <51081940+saeedkolivand@users.noreply.github.com>",
          "timestamp": "2026-07-02T13:11:56+02:00",
          "tree_id": "841e37a40c3e50d11e141a54e83713ded6201d0a",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/75a6be748d71e918fc8d88f572c923c36ed71e3d"
        },
        "date": 1782991910330,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1894495,
            "range": "± 58273",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2609903,
            "range": "± 42893",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287803,
            "range": "± 13359",
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
          "id": "af02e53167532450ba5c5260bfaed2da2056461a",
          "message": "fix: surface newly found autopilot jobs at the top of the list (#545)\n\nmerge_found_jobs appended never-seen postings to the end of the\ncumulative found-jobs list, so new finds rendered last in AutopilotCard.\nBuild the merged Vec new-first: genuinely-new URLs (in the already\nscore-sorted incoming order) followed by the refreshed existing rows.\nExisting rows keep their prior order, found_at, and is_new-cleared state;\nno frontend/schema/IPC change since the renderer shows the Vec verbatim.\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-04T06:04:30+02:00",
          "tree_id": "eafaf40654f9b54c421bbb3ebe8750cf71fcda2f",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/af02e53167532450ba5c5260bfaed2da2056461a"
        },
        "date": 1783138426151,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1914287,
            "range": "± 55279",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2557420,
            "range": "± 23509",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 297274,
            "range": "± 3014",
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
          "id": "43a78def8fe8623626718ca5b34edc682d348e2d",
          "message": "fix: stop project links seeding the contact profile on import (#547)\n\n* fix: stop project links seeding the contact profile on import\n\nclassify_contact_links seeded the header from the resume's flat link\npool with host-only rules, so a GitHub repo URL became the candidate's\ngithub, a live-demo URL became website, and every remaining http(s) link\nwas dumped into extra_links — forcing users to delete project links from\nContact Profile settings after every import.\n\nGate seeding by URL shape, mirroring links.ts (isProfileShaped/\nclassifyLinks): only a profile-shaped platform profile (github.com/<user>,\nnot /<user>/<repo>) or a bare-root personal domain seeds the profile;\ndeep-path repo/demo/article/project links are dropped. LinkedIn keeps its\nstricter /in/ gate. Pure string parsing (no regex), doc-comments point to\nlinks.ts as the canonical rule so the two can't drift.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: assert promoted github profile is not duplicated in extras\n\nMakes the no-double-count invariant explicit in the classifier test\n(addresses a claude-review nit on #547).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-04T07:19:58+02:00",
          "tree_id": "c5342a74ccd6c6208f28aa568645bd59aecbe497",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/43a78def8fe8623626718ca5b34edc682d348e2d"
        },
        "date": 1783143543980,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1950304,
            "range": "± 51591",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2572365,
            "range": "± 94183",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 307597,
            "range": "± 26811",
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
          "id": "4bed556c96a6d2a71b77d9ab7157732739d2e9f5",
          "message": "feat: research the market salary range for salary-expectation answers (#549)\n\n* feat: research the market salary range for salary-expectation answers\n\nGround the salary application answer in a web-researched market range,\nnot just the candidate's saved expectation, so the paste-ready number is\nmarket-aware.\n\nAdd AiProvider::research_salary (reusing each provider's native web search\nvia an extracted web_search_complete transport, so the company-brief path\nis unchanged and new providers stay zero-change), a SalaryResearch\nenricher that parses+validates the provider output into {min,max,currency}\nand caches it (7-day TTL, case-folded key), and an ai_lookup_salary IPC\ncommand guarded by the same rate-limit + daily-cost budget as ai_generate.\n\nOnly the validated integers + a shape-checked currency ever reach the\nprompt (no raw web text) — the injection boundary. A fenced <salary_context>\nblock feeds the answer; the paste-ready Number is the applicant's own\nexpectation floored at the market minimum (never underselling), the range\nmidpoint when no expectation is set, and omitted (non-committal, never\ninvented) when no source is grounded. Degrades to the C1 saved-expectation\nbehavior whenever the lookup is unavailable, fails, or times out.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: don't floor the salary number across currencies\n\nAddress a claude-review finding: the market range from <salary_context>\nis in the location's local currency, but the applicant's stated\nexpectation is free text and may be another currency. Only apply the\nanti-lowball floor/midpoint when the expectation is in the same currency\nas the market range; otherwise use the applicant's own figure and\ncurrency (C1 behavior) and mention the market range as context only,\nnever reconciled across currencies.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-04T10:19:30+02:00",
          "tree_id": "16100eb4af025af7789fd0637072ccbabbd5a711",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/4bed556c96a6d2a71b77d9ab7157732739d2e9f5"
        },
        "date": 1783153700138,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1885986,
            "range": "± 45297",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2502087,
            "range": "± 26513",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 281337,
            "range": "± 1719",
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
          "id": "7da5c92f275c4dc8fe16df8ab7b4c691d54605ff",
          "message": "fix: rate-limit the ai_research_company web-search command (#550)\n\n* fix: rate-limit the ai_research_company web-search command\n\nai_research_company fired a billable provider web search with no rate,\nconcurrency, or daily-budget guard, unlike ai_lookup_salary and\nai_generate — a looping or compromised renderer could drive unbounded\npaid-API spend. Apply the same guard, sharing the \"ai_research\" limiter\nbucket (previously wired to ai_lookup_salary only) and the per-provider\ndaily ceiling; degrade to the existing empty {company,brief} shape when\nthrottled.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: mark ai_research_company as guarded in the limiter module\n\nFollow the code: this PR guards ai_research_company, so the limits module\ndocs that listed it as an out-of-scope follow-up gap are now stale. Add it\nto the guarded-commands list and drop the \"known gap\" wording.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-04T10:44:30+02:00",
          "tree_id": "a84794b7b348f3e062790565b8003fefce774e45",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7da5c92f275c4dc8fe16df8ab7b4c691d54605ff"
        },
        "date": 1783155188241,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1913778,
            "range": "± 73274",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2509033,
            "range": "± 70625",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287506,
            "range": "± 5466",
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
          "id": "b3ed856690e934f2344e4ef562b47f9ad39344e7",
          "message": "feat: ground salary answers in a job's scraped salary range (#551)\n\n* feat: ground salary answers in a job's scraped salary range\n\nPrefer a posting's own scraped salary over the web lookup for the salary\napplication answer (precedence: scraped > web research > saved\nexpectation). Scraped salary is deterministic and free, so it avoids a\npaid web search when the posting already carries the numbers.\n\nBackend: persist salary end-to-end. Adzuna's salaryMin/max gain a derived\nISO-4217 currency (country -> currency map; unknown -> none -> web\nfallback). FoundJob and the Application record carry salary_min/max/\ncurrency; a new nullable, append-only migration (#6) adds the columns\n(NULL = unknown, never 0). Both apply paths (autopilot + manual Jobs)\nthread it through.\n\nRenderer: a complete, validated scraped range (positive, finite,\nmin<=max, ISO-shaped currency) builds the SalaryRange and skips the web\nlookup; anything malformed falls through to the web lookup so the answer\nnever loses its range. buildScrapedSalaryRange is a strict superset of the\nprompt-layer buildSalaryRangeBlock guard, rounded to integers to match the\nweb path. The <salary_context> block is now source-neutral.\n\nAlso add a test seam for the previously-untested provider transports:\nextract OpenAI's web-search HTTP body (wiremock-tested) behind a pure gate\npredicate, and inject a SalarySearcher trait + KvCache into\nSalaryResearch::enrich so its parse/degradation/timeout/cache paths are\nunit-tested (no AppHandle/network). Dev-only tokio test-util feature; no\nnew dependency.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: drop the dead salary_research architecture allowlist entry\n\nThe enrich dependency-injection refactor removed salary_research's\nAppHandle/try_state coupling, so its debt-allowlist entry is now dead and\ntrips the no-dead-entries architecture check. Remove it — the module is\nnow cleanly tauri-free at layer 2.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: round the scraped salary before validating, and uppercase currency\n\nAddress claude-review findings on buildScrapedSalaryRange: validate the\nrounded values (not the raw ones) so a sub-0.5 min that rounds to 0 falls\nback to the web lookup instead of blanking the range, keeping the guard a\nstrict superset of the prompt-layer check. Also uppercase the currency so\nit renders consistently. Tests cover the sub-0.5 fall-through and the\nmixed-case currency normalization.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-04T12:46:29+02:00",
          "tree_id": "680ef391698cd9970fa4dfd371877497a11627c7",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b3ed856690e934f2344e4ef562b47f9ad39344e7"
        },
        "date": 1783162543741,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1909986,
            "range": "± 49628",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2547085,
            "range": "± 16678",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 293175,
            "range": "± 23999",
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
          "id": "a6face033944df702c9aaa6cbb008630b1fea44a",
          "message": "feat: add agentic tool-calling foundation (provider channel + budgeted loop) (#552)\n\n* feat: add agentic tool-calling foundation (provider channel + budgeted loop)\n\nBackend-only Phase 1 of the human-in-the-loop assistant. Not yet wired to a\nTauri command (Phase 2 adds agent_run), so no user-facing behavior changes.\n\nai_provider: new non-streaming chat_with_tools trait method whose default\ndelegates to complete, so CLI agents and non-tool models degrade to\nsingle-shot. Native overrides for Anthropic, OpenAI(-compatible), Gemini,\nOllama, and Ollama Cloud, each gated by capabilities(model).supports_tools,\nwith pure unit-tested per-vendor tool-call parsers.\n\nagent/: per-flow tool registry (Read/Write kinds, no global tool set) and a\nbudgeted while-loop controller (step + token caps, cancel between turns).\nWrite tools are denied and logged until the Phase 3 confirm gate.\n\nsecurity: untrusted job text and tool results are fenced as data, never the\ntrusted system prompt; tool handlers take provider/model/base_url only from\ntrusted context, never model args (lethal-trifecta exfil guard). The\ntranscript fold preserves user/assistant wire-role alternation (Anthropic and\nGemini 400 guard); truncated (Length) turns stop without executing tool calls.\n\nReviewed by ai-provider-expert, rust-backend-architect, and\ntauri-security-reviewer (2 HIGH + 3 MEDIUM resolved). build + clippy +\ntest-compile green.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: guard truncated tool calls and stop gracefully on budget limits\n\nAddress CodeRabbit review on the Phase-1 agent loop (still backend-only).\n\n- controller: a mid-run provider budget/rate-limit error now returns a graceful\n  StoppedReason::Budgeted with partial progress, instead of aborting and\n  discarding the accumulated steps and text.\n- controller: a length-truncated final answer with no tool calls now reports\n  Truncated instead of Done.\n- gemini: finishReason MALFORMED_FUNCTION_CALL maps to Length so truncated\n  tool-call JSON is never executed; an empty/blocked turn surfaces a provider\n  error instead of a blank Done.\n- ollama: done_reason \"length\" is checked before tool-calls presence, so a\n  truncated call maps to Length, not ToolUse.\n- docs: add events and agent to the L3 heading list; cargo fmt.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-04T23:02:34+02:00",
          "tree_id": "dd70ee7fb0928e407d82365867abc9247120f462",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/a6face033944df702c9aaa6cbb008630b1fea44a"
        },
        "date": 1783199482479,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1874264,
            "range": "± 37581",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2498566,
            "range": "± 13806",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 291877,
            "range": "± 2091",
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
          "id": "0fbd7c82946ad7cc7547d2b061c26f9755a874a4",
          "message": "feat: add prep-application agentic assistant flow with streaming panel (#555)\n\n* feat: add prep-application agentic assistant flow with streaming panel\n\nPhase 2 of the human-in-the-loop assistant. A new `agent_run` Tauri command\n(the repo's 5-step IPC flow) runs the Phase-1 agent loop over a job + résumé\nwith a READ-ONLY tool whitelist (research company, match résumé, draft cover\nletter, suggest interview questions) and streams `agent:step` events to a\nPrepApplicationPanel in the Job Detail pane. The terminal step is a display-only\nPROPOSED status update — no write executes; Phase 3 adds the confirm gate.\n\nBackend:\n- commands/agent.rs — `agent_run`: limiter + job tracker, cancel wired via the\n  scraper token registry (Stop actually interrupts; Cancelled maps to job_cancel,\n  not job_complete), and it requires a tool-capable model (fails early otherwise).\n- agent/flows.rs — the fixed, trusted PREP_APPLICATION_SYSTEM prompt.\n- agent/tools.rs — a trusted ToolContext carries provider/model/base_url (only\n  from the validated request, never model/tool args — severs the exfil leg);\n  two new draft tools; prep_application_tools() whitelist (zero Write tools).\n- agent/controller.rs — AgentStepKind{Turn,Proposal} and a jobId on every step.\n- Untrusted job/résumé/company-brief text is fenced as data.\n\nFrontend:\n- PrepApplicationPanel + agent-run.machine (distinct cancelled state, working\n  retry, interrupted-row status) + use-agent service hook. Stop pinned in the\n  ModalShell footer; step-milestone aria-live (not token-by-token); en/de i18n.\n\nShared: AgentRunRequest + AgentContract + AgentStepEvent (carries jobId).\nAlso: use-focus-trap re-queries focusable elements at keydown so the trap\nsurvives dynamic modal content (fixes a latent a11y escape this panel exposed).\n\nReviewed by ai-provider-expert, rust-backend-architect, tauri-security-reviewer,\nfrontend-reviewer, ui-ux-expert; all HIGH/MEDIUM resolved. cargo build + clippy\n(-D warnings) + fmt clean; typecheck 12/12 and 3052 TS tests pass. Rust tests\nrun in CI (this host can't launch the test binaries).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: address prep-application review findings (cancel, research, re-run)\n\nAdvisory findings from the @claude PR review on #555 (no blocking issues).\n\n- mid-request cancellation: race env.turn() and each read-tool call against\n  cancel.cancelled() via tokio::select! (biased), so Stop interrupts an\n  in-flight provider round-trip immediately instead of only between turns.\n- research_company: load the run's own posting server-side by ctx.job_id\n  (threaded into ToolContext from the validated request), taking no\n  model-supplied text — consistent with the other id-loaded tools.\n- estimate_tokens: count chars, not bytes, so multi-byte résumé/job text\n  doesn't trip the token budget earlier than ASCII of the same length.\n- panel: show a \"Prep again\" re-run affordance from the done state too\n  (previously only error/cancelled offered a retry).\n- docs: correct the agentic-feature PR numbers in ARCHITECTURE_STATUS.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-05T01:45:45+02:00",
          "tree_id": "f36ed7e70bf32af96317da79c72befb90c2f88b5",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/0fbd7c82946ad7cc7547d2b061c26f9755a874a4"
        },
        "date": 1783209292579,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1896806,
            "range": "± 47993",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2519002,
            "range": "± 25635",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 291710,
            "range": "± 2524",
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
          "id": "ce0558f31c5ce39d20f2bdafed4f8a98092b08d1",
          "message": "feat: gate agent write actions behind human-in-the-loop confirmation (#556)\n\n* feat: gate agent write actions behind human-in-the-loop confirmation\n\nPhase 3 of the agentic assistant — the safety core. Agent Write actions now\nSUSPEND the run for explicit user approval; nothing is persisted or applied\nwithout an approve/edit click.\n\nBackend:\n- agent/gate.rs — AgentGate: Mutex<HashMap<(jobId,callId), oneshot::Sender>>;\n  register/resolve/remove, no lock held across an await.\n- agent/controller.rs — a Write tool call now emits an agent:step\n  kind=confirm_request and awaits the user's Decision, raced (tokio::select!,\n  biased) against cancel / a 300s timeout / the decision — cancel, timeout,\n  deny, and a dropped channel all default to NOT acting, with gate.remove on\n  every branch. Edited args are re-validated by two fail-closed layers (a\n  routing/egress key denylist + a tool-schema whitelist); routing and identity\n  stay in the trusted ToolContext. callId is keyed {step}-{idx}-{tool}.\n- commands/agent.rs — agent_confirm (renderer-only IPC, never a model-callable\n  tool; map_decision fail-closed). agent/tools.rs — save_cover_letter, the\n  first gated Write tool (wraps ai_generations_save; target from ctx.job_id).\n\nFrontend:\n- AgentConfirm — approve / deny / edit UI; args rendered as untrusted data,\n  editable content-only; brand accent; focus-managed (no focus-trap escape);\n  loading, error, and no-longer-available states. agent-run.machine gains a\n  confirming state (Stop still works while suspended).\n\nReviewed by tauri-security-reviewer (gating authority — PASS), rust-backend-\narchitect, frontend-reviewer, and ui-ux-expert; all HIGH/MEDIUM resolved.\ncargo build + clippy (-D warnings) + fmt clean; typecheck 12/12; 3076 TS tests\npass. Rust tests run in CI.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: split confirm-gate module under the loc cap and address review nits\n\nFixes the CI R8 (oversized-module) failure plus @claude review advisories.\n\n- split agent/controller.rs (was 1676 LOC, over the 1400 R8 hard cap): moved\n  the confirm-gate execution + validation (resolve_write, run_write_raced,\n  validate_edited_args, is_routing_egress_key, clamp_json_strings,\n  WriteResolution, CONFIRM_TIMEOUT/ARGS_DISPLAY_CAP) and their tests into\n  agent/gate.rs. Behavior-preserving; controller.rs now 985, gate.rs 1080,\n  both under the cap.\n- panel: clear a stale confirm card when a non-confirm step resumes the run,\n  which happens on the 300s server-side confirm timeout (deny-and-resume).\n- validate_edited_args: reject routing/egress keys at ANY nesting depth\n  (recursive scan), so a future object-typed gated-tool arg can't smuggle a\n  nested provider/base_url/job_id.\n- docs: correct the callId format to {step}-{idx}-{tool} in the three doc\n  comments that still said {step}-{tool}.\n\ncargo build + clippy (-D warnings) + fmt clean; typecheck 12/12; 3078 TS tests\npass. Rust tests run in CI.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-05T04:12:18+02:00",
          "tree_id": "bcc94bbd3a8f6a4df9948c2352a0817e8e4b8808",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ce0558f31c5ce39d20f2bdafed4f8a98092b08d1"
        },
        "date": 1783218090527,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1907110,
            "range": "± 64384",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2617163,
            "range": "± 46531",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 293620,
            "range": "± 7195",
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
          "id": "7d2d759968d44e3ff1773a6f570607e918fc0d3d",
          "message": "feat: add opt-in autopilot ai notes for top matches (headless, notify-only) (#557)\n\n* feat: add opt-in autopilot ai notes for top matches (headless, notify-only)\n\nPhase 4 (final) of the agentic assistant — optional autonomy. When an autopilot\nopts into \"AI notes\", each scheduled run attaches a short LLM-reasoned note (why\na top match fits + a tailoring tip) to its top few NEW matches. Headless,\nread-only, notify-only — nothing is applied or submitted.\n\nBackend:\n- the autopilot record gains an opt-in `assistant` flag + a provider snapshot\n  (assistant_provider/model/base_url, captured by the renderer at opt-in — no\n  keys, which stay in the keychain); FoundJob gains assistant_notes.\n- autopilot_helpers::generate_assistant_notes: a plain Completer::complete()\n  per top-N NEW match behind a NoteEnv trait (unit-tested via a fake env).\n  Read-only by construction (no tools / no Write / no confirm — there is no\n  live user headless). Triple-bounded: ASSISTANT_NOTES_MAX=3, a\n  charge_provider_daily short-circuit, cancellable (tokio::select! around the\n  in-flight call), a 45s step timeout, and a prior-URL skip so steady-state\n  re-runs make zero calls. Resume/job text is fenced as untrusted data.\n- the new-jobs notification body reflects \"(N with AI notes)\".\n\nFrontend:\n- an \"AI notes\" Switch in the autopilot wizard that snapshots the active\n  provider; a provider hint + a legible (>=/70) disclosure caption; the note\n  surfaces on found-job rows as a labeled, line-clamped, plain-text block.\n\nReviewed by tauri-security-reviewer (PASS), performance-profiler,\nscraping-applier-expert, frontend-reviewer, and ui-ux-expert; all HIGH/MEDIUM\nresolved. cargo build + clippy (-D warnings) + fmt clean; typecheck 12/12;\nautopilot suite green. Rust tests run in CI.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: resolve r7 layering and address review findings for autopilot ai notes\n\nFixes the RED CI Architecture Boundary Test (R7) plus the @claude and\nCodeRabbit review findings on the Phase 4 autopilot AI notes PR.\n\nR7 layering + CodeRabbit (AppHandle coupling in L2):\n- move provider/limiter resolution UP to the L3 caller (commands/autopilot.rs,\n  which already holds the AppHandle) and pass a resolved Option<&Completer> +\n  Arc<Limiter> down into generate_assistant_notes. autopilot_helpers (L2) is now\n  AppHandle-free for the notes path and no longer reaches up into crate::commands\n  (dissolves the L2->L3 commands/ProviderId edge). The gate on autopilot.assistant\n  also skips a resolve+log for the common notes-off run.\n- allowlist the remaining L2->L3 agent edge in tests/architecture.rs: headless\n  notes reuse the agent layer's pure prompt-safety primitives (fenced/JOB_CAP/\n  RESUME_CAP) + the AUTOPILOT_NOTE_SYSTEM literal; read-only string construction,\n  invokes no controller. TODO(arch): relocate those primitives to an L0/L1\n  prompt-utils to clear the edge. Documented in docs/architecture-rules.md.\n\nCodeRabbit (correctness):\n- clear the assistant_provider/model/base_url snapshot when AI notes are toggled\n  off on update, so a disabled record can't linger with a stale, reusable egress\n  target (also shrinks the MEDIUM-4 renderer-provenance window).\n\n@claude review (UX + diagnostics):\n- guard the AI-notes Switch when no provider is configured: disable it with an\n  explanatory caption, never snapshot an empty model, and drop the dangling\n  \"Notes will use <provider> - \" hint separator.\n- restore the resolve-error reason in the \"no usable provider\" skip log so\n  \"notes never run\" is debuggable again.\n- note_user_msg no longer emits a bare \" at \" for an empty title+company.\n\ncargo fmt + clippy (-D warnings) + build clean; cargo test --test architecture\n11 passed (R7 + no-dead-entries green); gen:ipc:check clean; TS typecheck +\n1926 renderer tests pass. Reviewed by rust-backend-architect, tauri-security-\nreviewer (PASS), and frontend-reviewer.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-06T17:08:47+02:00",
          "tree_id": "72a4239bec5f09955e7c02686120c66aa4b1e589",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7d2d759968d44e3ff1773a6f570607e918fc0d3d"
        },
        "date": 1783351048454,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1887511,
            "range": "± 53085",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2468803,
            "range": "± 62535",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 280864,
            "range": "± 2543",
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
          "id": "5cf2534655bd11f609b1da9c7944180160708796",
          "message": "fix: prevent prep-this-application runs from getting stuck at pending (#558)\n\n* fix: prevent prep-this-application runs from getting stuck at pending\n\nThe agentic \"Prep this application\" flow could hang on a permanent spinner\nand never complete. Two independent causes, both fixed.\n\n1. Renderer race (the permanent hang). agent_run minted the job id, started\n   the job, then ran provider/model/resume/job validation synchronously — each\n   failure emitted the terminal job.failed jobs:event BEFORE the {jobId} return.\n   The panel only learns the job id (and starts filtering events on it) from\n   that return, so a fast validation failure's terminal event was dropped and\n   the state machine sat in `planning` forever.\n   - backend: move every fail-able validation into the spawned task so all\n     terminal events (success or failure) fire asynchronously, exactly like the\n     happy path. job_start + cancel-token registration stay synchronous before\n     the spawn (a fast jobs_cancel must not be a no-op); the limiter-rejection\n     is the sole synchronous fail (before the token exists) and the renderer\n     reconciles it. New fail_run helper (job_fail + unregister) at each site.\n   - renderer: PrepApplicationPanel now reconciles against the job's actual\n     status via useJob(runJobId) once the id is known — if the job is already\n     terminal while the machine is still busy, it drives COMPLETE/ERROR/CANCEL\n     (shared finishRun helper, fires at most once, never overrides a terminated\n     machine). Belt-and-suspenders for any residual event-before-subscribe gap.\n\n2. No per-step wall-clock timeout (long stalls). The controller raced each\n   provider turn / read-tool call only against cancel — a hung or misconfigured\n   OpenAI-compatible base_url could block for minutes with no terminal event.\n   - add AGENT_STEP_TIMEOUT (360s, above the 300s Ollama HTTP timeout) wrapping\n     both step selects, plus StoppedReason::Timeout mapped to job_fail (never a\n     silent success), so the loop always terminates and always emits a terminal\n     jobs:event. The confirm-gate suspension keeps its own CONFIRM_TIMEOUT.\n\nReviewed by rust-backend-architect (token lifecycle + timeout/cancel\ncomposition correct) and frontend-reviewer (exact payload parity + idempotency).\ncargo fmt + clippy (-D warnings) + build clean; cargo test --test architecture\n11 passed; two new FakeEnv step-timeout tests; typecheck + PrepApplicationPanel\nsuite (19) green. Rust tests run in CI.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: cover completed and cancelled reconciliation branches\n\nAddresses the @claude review LOW on PR #558: the useJob reconciliation\nfallback was only tested for status 'failed'. The completed branch reads\njob.result (vs the live path's event.data), so add completed + cancelled\ncases asserting the proposal renders / cancelled state shows and the busy\nspinner clears. 21 PrepApplicationPanel tests pass.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-06T18:23:53+02:00",
          "tree_id": "3391e6a23e05778dbc2a68434a47be4659608ae7",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/5cf2534655bd11f609b1da9c7944180160708796"
        },
        "date": 1783355595530,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1932562,
            "range": "± 68327",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2572055,
            "range": "± 21803",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 291357,
            "range": "± 986",
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
          "id": "f7765e0fee720fbb78f16f77d7f157ce911bea2d",
          "message": "fix: ground salary-lookup currency in the job's country (#560)\n\n* fix: ground salary-lookup currency in the job's country\n\nThe web-lookup salary path let the model freely choose the currency\n(SALARY_SYSTEM said \"use the local currency for that location\" and the user\nprompt dropped the location clause entirely when it was blank), so a job in\nGermany with a weak/empty location string rendered USD or a hallucinated CAD\nrange instead of EUR.\n\nGround the currency in the job's validated ISO country end-to-end:\n- add countryToCurrency(iso2) in @ajh/prompts (ISO-3166 alpha-2 -> ISO-4217,\n  covering the Eurozone incl. HR/BG plus the common markets), beside\n  countryToMarket.\n- thread the job country + resolved expected currency from the answer call\n  site through lookupSalaryRange -> the lookupSalary IPC contract ->\n  ai_lookup_salary -> SalaryResearch::enrich -> the shared research.rs prompt\n  builders, which now pin the currency authoritatively (\"the role is based in\n  {country}; report the range in {currency}\") across all provider fan-out\n  builders + the Ollama synth path. When the country is unknown the builders\n  fall back byte-for-byte to the prior unconstrained wording.\n- defense-in-depth: reconcile_expected_currency DROPS a result whose currency\n  still doesn't match the expected one (returns None -> graceful no-range),\n  rather than relabeling numbers it can't convert. A stale wrong-currency cache\n  entry falls through to a fresh grounded fetch. The scraped-salary range\n  (PR #551) still takes precedence.\n\nReviewed by ai-provider-expert (provider abstraction + contract parity intact)\nand job-match-expert (all currency mappings correct; the relabel->drop backstop\nwas its MEDIUM, now fixed). gen:ipc:check clean; typecheck 12/12; 365 @ajh/prompts\ntests + 3111 renderer tests; cargo fmt + clippy (-D warnings) + build +\narchitecture (11) green. Rust unit tests run in CI.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: address review findings on salary currency grounding\n\nFollow-ups from the @claude review of PR #560:\n- include the expected currency in the salary cache key, so two postings that\n  share role/company/location but differ in country (e.g. two Remote listings)\n  no longer collide, and an unknown-currency read can't return a currency a\n  different known-country job cached. reconcile_expected_currency stays as the\n  self-heal for a stale wrong value under the same key.\n- gate the Ollama synth/search-query country interpolation on a resolved\n  currency (mirroring the native path), so the two paths agree on how much they\n  trust the raw country string.\n- extend COUNTRY_TO_CURRENCY with HU->HUF, RO->RON and the euro microstates\n  SM/VA/AD->EUR (previously fell back to unconstrained, never wrong).\n\n@ajh/prompts 365 tests + new cache-key/Ollama-gating unit tests; typecheck +\neslint clean; cargo fmt + clippy (-D warnings) + build + architecture (11)\ngreen. Rust unit tests run in CI.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-06T21:19:17+02:00",
          "tree_id": "bc389bc6eef5ec8aed2b546cde811640e0770efa",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f7765e0fee720fbb78f16f77d7f157ce911bea2d"
        },
        "date": 1783366076331,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1880890,
            "range": "± 136048",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2503696,
            "range": "± 19557",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 292485,
            "range": "± 8814",
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
          "id": "125afd46a3d63296b416aae343bb3f1a71228e94",
          "message": "feat: draft a tailored resume in the prep-this-application flow (#561)\n\n* feat: draft a tailored resume in the prep-this-application flow\n\nThe agentic \"Prep this application\" flow only drafted a cover letter, never a\nresume (user-reported). It now also drafts a tailored resume and offers to save\nit, mirroring the existing cover-letter tool pair exactly.\n\n- draft_resume (Read): generates an ATS-tailored resume from the fenced resume\n  + job posting, driven by a new trusted RESUME_SYSTEM literal (a compact\n  backend port of @ajh/prompts buildResumeSystemPrompt keeping the honesty /\n  keep-every-role / keyword-weaving spine). Draft-only — no persistence, no\n  confirm gate (same class as draft_cover_letter).\n- save_resume (Write, GATED): persists the drafted resume via the existing\n  ai_generations_save(resume_text). Like save_cover_letter it SUSPENDS the run\n  for explicit user confirmation and never auto-persists; its schema is\n  content-only (resumeText), with company/title/url/board resolved server-side\n  from the trusted ToolContext, never from model args.\n- PREP_APPLICATION_SYSTEM sequences the new step; ARGS_DISPLAY_CAP widened to\n  max(COVER_LETTER_CAP, SAVED_RESUME_CAP=40k) so the confirm UI shows the full\n  resume for review/edit; MAX_AGENT_STEPS raised 8 -> 12 to fit the flow's seven\n  tool turns plus planning/summary with headroom.\n- renderer: a resume checklist row in PrepApplicationPanel + a draft_resume ->\n  DRAFT mapping in the agent-run machine; en/de copy updated so the promised\n  behavior (resume + cover letter + questions) matches reality.\n\nReviewed by tauri-security-reviewer (PASS — gated Write verified end-to-end,\nfenced untrusted data, trusted routing, display cap >= persist cap),\nresume-export-expert (honesty spine intact), and ai-provider-expert (tool wiring\ncorrect; the step-budget MEDIUM is fixed here). cargo fmt + clippy (-D warnings)\n+ build + architecture (11, R8 ok) clean; typecheck 12/12; desktop suite (1941)\n+ agent-run machine/panel (35) green; gen:ipc:check unchanged. Rust tests run in CI.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: raise the agent token budget for the resume-augmented prep flow\n\nThe @claude review of #561 noted that after raising MAX_AGENT_STEPS, the token\nbudget became the tighter backstop: the prep flow now echoes a full drafted\nresume twice through the accumulator (the draft_resume tool result + the\nsave_resume args turn) on top of the cover letter, match, and research. A large\nresume (SAVED_RESUME_CAP=40k chars ~= 10k tokens, twice) could trip MAX_AGENT_TOKENS\nand stop the run before the final save/summary. Raise MAX_AGENT_TOKENS 60k -> 120k\n(clear headroom over the two-resume-echo worst case) and document the driver;\nstill the cost backstop.\n\ncargo fmt + clippy (-D warnings) + build + architecture (11) green.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-06T22:46:08+02:00",
          "tree_id": "b99ff90875fcb5eaf08d20c600a1f4684ba66ed1",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/125afd46a3d63296b416aae343bb3f1a71228e94"
        },
        "date": 1783371345474,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1926228,
            "range": "± 84462",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2633083,
            "range": "± 41667",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 306619,
            "range": "± 6725",
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
          "id": "4ceeb66b221a34d6803c975bf2637cc28a92733f",
          "message": "feat: add optional per-question web search to application answers (#562)\n\n* feat: add optional per-question web search to application answers\n\nApplication-question answers were generated with no live web grounding\n(user-reported: \"Application questions now can search internet to provide\nbetter answers\"). Adds an opt-in per-question web search that reuses the\nexisting provider-native web-search infrastructure.\n\n- new AiProvider::research_answer facet mirroring research/research_salary\n  (default Ok(\"\"); native providers delegate to their existing\n  web_search_complete transport, Ollama family to the Ollama Web Search API) —\n  zero new per-provider HTTP. Completer::research_answer wrapper.\n- new ai_research_answer command mirroring ai_research_company: shared\n  \"ai_research\" limiter bucket, Completer::resolve, capability pre-check that\n  returns \"\" WITHOUT charging when the provider can't web-search (avoids\n  wasting the daily budget per question on the fan-out), otherwise\n  charge_provider_daily; forwarded question/role/company are length-capped;\n  returns \"\" on any failure. Full 5-step IPC flow (+ mock-client parity).\n- the untrusted result is fenced as <web_search_notes> in\n  buildApplicationAnswerPrompt (reference-only, ignore embedded instructions,\n  never writes the answer), never reaching the system prompt; a literal\n  closing tag in the note is neutralized to prevent fence-breakout (applied to\n  the company-research block too). The research prompts forbid the model from\n  writing the answer itself.\n- renderer: a \"Search the web for better answers\" @ajh/ui Switch in the\n  Application Questions modal (disabled while generating), wired through\n  useApplicationAnswers so each selected question searches first; on\n  \"\"/unsupported/failure the answer still generates unchanged. en/de copy added.\n\nReviewed by ai-provider-expert (provider abstraction + transport reuse +\nlimiter parity intact), tauri-security-reviewer (PASS — untrusted result fenced\nas data, no arbitrary egress, capability-gated, cost-bounded), and\nfrontend-reviewer (graceful degradation, toggle gating). gen:ipc:check clean;\ntypecheck 12/12; @ajh/prompts (371) + desktop suite (1951); cargo fmt + clippy\n(-D warnings) + build + architecture (11) green. Rust tests run in CI.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix: bound web-search fan-out and harden ai_research_answer\n\nAddresses the review of PR #562 (CodeRabbit MAJOR + advisories):\n- bound the per-question web-search fan-out (WEB_SEARCH_MAX_PER_RUN=8) so a\n  many-question form can't exhaust the shared (day, provider) daily budget and\n  starve company/salary research; questions past the cap still generate with an\n  empty webSearchNotes. A dedicated budget bucket is noted as the fuller fix.\n- use a question-specific truncation cap (700) for the search query instead of\n  the 200-char salary cap that cut long questions mid-sentence (role/company\n  keep 200); the final answer prompt still uses the full question.\n- log the web-search failure (tracing::debug) before degrading to empty, so a\n  silently-non-searching provider is diagnosable.\n- extract research_answer_core behind an AnswerSearcher trait (mirroring\n  SalarySearcher) to unit-test the branching: non-searchable provider returns\n  empty WITHOUT charging the daily budget; success charges once; failure\n  degrades; question truncation is char-boundary-safe.\n- strengthen the renderer failure-path test (loop continues + empty notes after\n  a search rejection) + a fan-out-cap test.\n\ngen:ipc:check clean; typecheck 12/12; desktop (1952) + @ajh/prompts (371);\ncargo fmt + clippy (-D warnings) + build + architecture (11) green. Rust tests\nrun in CI.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore: bump crossbeam-epoch to 0.9.20 for rustsec-2026-0204\n\nAn advisory published 2026-07-07 flags an invalid pointer dereference in\ncrossbeam-epoch below 0.9.20 (transitive via rayon-core -> exr -> image).\nLockfile-only bump per the advisory (cargo update -p crossbeam-epoch);\nunblocks the Cargo Deny CI gate. Unrelated to this PR's feature changes.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-07T00:49:13+02:00",
          "tree_id": "65fd54284e87ad50c7431df212657a12e703cb3a",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/4ceeb66b221a34d6803c975bf2637cc28a92733f"
        },
        "date": 1783379346756,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1904180,
            "range": "± 75672",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2549880,
            "range": "± 47722",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 294972,
            "range": "± 7906",
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
          "id": "169f7ae8e386b47c58b9546f5cf0986f1d063424",
          "message": "feat: humanize generated application text so it reads as authentic human writing (#563)\n\nText from the résumé / cover-letter / application-answer generators read as\nobviously AI-written (an AI detector confidently flagged it). The shared\nnatural-voice module was almost entirely a negative ban list; this adds the\npositive \"write like a specific human\" guidance the 2026 humanization research\nconverges on, grounded in the existing honesty spine (humanize only real\ncontent, never invent).\n\n- natural-voice: new HUMANIZE_PROSE (cadence/burstiness — mix short and long\n  sentences; concrete specifics from the candidate's real résumé; cut clichés,\n  hedging preambles and stock transitions; tone-gated controlled imperfection;\n  authentic first-person voice) and HUMANIZE_LEXICAL (résumé/ATS-safe tier:\n  real metrics/tools + bullet-shape variety while every bullet still opens with\n  a strong action verb — no contractions/fragments/em-dashes). Composed\n  alongside the untouched ANTI_AI_TELL_* bans across all 7 generation surfaces\n  at every provider depth.\n- wire the previously-dead Output Tone setting (professional/casual/formal/\n  creative) into the résumé/cover-letter/answer generators via a toneDirective\n  helper (renderer reads outputTone from the store and threads it into the\n  builders — no IPC/contract change). A lexical tone variant keeps the résumé\n  ATS-safe (never injects contractions), and TONE_PRECEDENCE + ATS_PRECEDENCE\n  keep keyword/CAR rules above tone. creative is explicitly bounded.\n- backend: the agent \"Prep this application\" RESUME_SYSTEM/COVER_LETTER_SYSTEM\n  literals get the compact always-on humanization to match the renderer path.\n\nExcludes deceptive artifacts (unicode/homoglyph/watermark tricks) — those\ncorrupt ATS parsing and misrepresent the candidate; authenticity is the goal.\n\nReviewed by ai-provider-expert (composition complete, honesty spine intact —\nno new fabrication surface), resume-export-expert (ATS keyword-matching + CAR\npreserved; LEXICAL/PROSE split correct), and frontend-reviewer (tone wiring,\nno IPC change). gen:ipc:check unchanged; typecheck 12/12; @ajh/prompts (421) +\ndesktop suite (1956); cargo fmt + clippy (-D warnings) + build + architecture\n(11) green. Rust tests run in CI.\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-07T02:33:26+02:00",
          "tree_id": "5b2b25a1c9bddce6d77b0039f3261d34f1c9ee73",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/169f7ae8e386b47c58b9546f5cf0986f1d063424"
        },
        "date": 1783384943448,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1880642,
            "range": "± 181719",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2527177,
            "range": "± 51763",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 289283,
            "range": "± 3685",
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
          "id": "eb7023079ff0c344ec292c676427eb3496e13b83",
          "message": "feat: resolve full-history audit findings (fixes, cleanups, docs) (#564)\n\n* docs: add full-history audit report, triage worklist, and adrs 0005-0006\n\nAdd AUDIT_REPORT.md (full-history logic audit) and docs/AUDIT_TRIAGE.md\n(the prioritized develop/modify worklist from the grill session).\n\nAdd ADR 0005 (network egress / privacy boundary) and ADR 0006 (Support\nFAQ-only, diagnostics dashboard removed), plus 5 new CONTEXT.md glossary\nterms.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(i18n): add 5 missing keys rendered raw by live components\n\nFive t() keys referenced by mounted components were absent from both en\nand de, so users saw the raw dotted key string: settings.resume.defaultSet\nand defaultSetFailed (ResumePreferences), onboarding.browser.checkFailed\nand checkFailedDesc (BrowserErrorState), and settings.cancel\n(CloudProviderConfig). Add all five to en + de.\n\nResolves audit findings i18n-001, p2-m-i18n-001, p2-b5-translations-seam-001.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(settings): add export-diagnostics action, remove dead support dashboard\n\nThe Support page is intentionally FAQ-only (ADR 0006). Delete the 22\nunrendered diagnostics/health/recovery/knowledge-base components, their\n13 unused support IPC contract methods + client/mock/hook entries, the\ndead SUPPORT_TABS constants, and ~15 orphaned support.* i18n namespaces\nfrom en + de.\n\nSalvage the one capability with a real backend: add an \"Export\ndiagnostics\" action to Settings > About, wired through the\nuseExportDiagnostics service hook to the existing support_export_diagnostics\ncommand. export_diagnostics is preserved end-to-end.\n\nResolves audit findings renderer-feat-3-001, p2-b1-ipc-chain-001,\np2-b1-ipc-chain-002, and a large share of the orphaned-i18n-key findings.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(match): default an omitted semantic-scoring flag to keyword-only\n\nsemantic_enabled_bit(None) returned 1 (semantic ON), so a caller that\nomits the flag — notably the agent match_resume tool — silently ran\nembeddings, disagreeing with the renderer's keyword-only default and the\nscores the user sees. Map only Some(true) to 1; Some(false) and None both\ndefault to 0. Update the pinning test.\n\nResolves audit finding p2-contra-renderer-features-002.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(privacy): wipe chromium board-login profiles on factory reset\n\nprivacy_reset_app disconnected boards (status flag + cookies snapshot)\nbut never deleted browser-state/<board>/profile, so a \"fresh-install\"\nfactory reset left live authenticated board sessions on disk. Add a\nbest-effort remove_dir_all of browser-state (exists-guarded, tolerates\nWindows file locks, never errors the reset). Mid-session disconnect is\nunchanged.\n\nResolves audit finding rust-data-001; aligns with ADR 0005.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(credentials): return bare bool and add a real os-keyring probe\n\ncredentials_available returned {available:bool} but the TS contract, mock,\nand consumer expect a bare boolean, so the \"no OS encryption\" warning's\n=== false gate was dead. Return a bare bool. Also replace the hardcoded\nis_available()==true with a read-only, memoized keyring probe (reads a\nsentinel slot; NoEntry/Ok => available, any other error => unavailable),\nso the warning fires on platforms without secure storage.\n\nResolves audit finding ipc-a-001.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* refactor: remove dead command-palette plumbing and match_resume_batch\n\nTwo fully-dead IPC capabilities, no live consumers:\n\n- Cmd+K command-palette plumbing (Q2b): the shortcut:command-palette event,\n  its shortcuts events/contract/tauri-client namespace, useCommandPaletteShortcut,\n  and the mock entry. The removed hybrid-search surface was its only user;\n  useKeyboardShortcuts deliberately implements no command palette.\n\n- match_resume_batch (Q3): the batch command + MATCH_BATCH_MAX + its tests,\n  the resumeBatch contract/client/mock, the MatchResumeBatchRequest schema +\n  its generator MODULES entry (regenerated ipc_contracts/matching.rs), and the\n  boundary tests. On-demand per-job scoring superseded it.\n\nResolves audit findings for the orphaned command-palette and match_resume_batch.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* refactor(prompts): remove the unconsumed structured-output helper\n\nThe structured-output helper had no consumer — native structured output was\nnever wired, and analysis/metadata parsing goes through validateAndRepair /\nvalidateMetadata for every provider. Remove the helper, its exclusive spec\ntype + analysis/metadata schemas + private consts, its test block, and the\nREADME row. Keep resolveProfile and the structuredOutput boolean.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(export): set pdf document title, author, and language\n\nExported PDFs had no document title and no language tag. Add a shared\ndocument_meta_preamble that sets the PDF title (candidate name + doc kind),\nauthor, and reasserts the locale language, injected into the assembled\nTypst source before page content. Name + language are read from data.json\nat Typst runtime (injection-safe); only the doc-kind label is baked in. A\ncheap screen-reader win short of full PDF/UA.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ci: fail the release when the generated notes body is empty\n\nconventional-changelog-conventionalcommits v10 silently empties the\nsemantic-release notes body and shipped a blank changelog in v0.119.0. Add a\nguard that fails a published release whose notes body is empty or whitespace.\nNotes are read via env (not inline) to avoid shell injection from the body.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): show live scrape progress while scanning boards\n\nThe Rust backend emits scrape:progress (a 0..1 boards-done/total fraction)\nduring an interactive scrape, but no renderer consumed it. Add a\nuseScrapeProgress service hook subscribing via a new scrape.onProgress port\nmethod, and render an @ajh/ui ProgressBar + \"Scanning N%\" label in the Jobs\nresults area while a fresh scrape has no results yet. Event-only port method\n(no request channel, like autopilot.onStep); no Rust or event-contract\nchange. Live results still stream via the existing job.stream channel.\n\nResolves audit finding p2-b7-events-deeplink-002.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: reconcile documentation with the audit fixes and pre-existing drift\n\nSync docs to code after the audit-triage changes and fix long-standing drift\n(each claim verified against source):\n\n- privacy: replace \"only outbound calls are AI provider + web search\" with the\n  personal-data guarantee + enumerated egress classes (ADR 0005)\n- semver: BREAKING CHANGE -> minor while 0.x (matches the .releaserc guard)\n- PDF: drop the PDF/UA tagged-output claim; note the new title/author/language\n  metadata; mark full PDF/UA-1 a future goal\n- drift: board count 16 -> 23; fonts -> Carlito/Inter/Source Serif 4/Manrope\n  (no Noto); storage -> per-domain SQLite (no app.db/LanceDB); accent -> teal\n  (user-customizable); release trigger -> manual dispatch\n- stale refs from this branch: remove match_resume_batch, structuredOutputFor,\n  and command-palette references; /resumes -> /documents\n- document the extension connection-phase model\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* feat(jobs): add a 15-minute posted-date filter preset\n\nThe finest posted-date preset was 30m though a 15m filter was requested. Add\n\"15m\" as the finest option in both DATE_FILTER_OPTIONS mirrors (Rust + shared\nschema, regenerated), the ScrapeFilters UI, and en/de labels. Crucially, wire\nthe cutoff through every backend that maps the preset to a query: LinkedIn\nguest search gets the exact window (f_TPR=r900), and the Adzuna/JSearch/Apify\naggregators collapse to their finest window (1 day / today / 24h), matching\nhow 30m already behaves. Without these arms \"15m\" silently fell through to\nno filter (LinkedIn) or month.\n\nResolves audit finding scraping-b-002.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(privacy): abort factory reset when the app data dir cannot resolve\n\nGuard the destructive browser-state wipe: privacy_reset_app fell back to the\nprocess CWD if app_data_dir() failed, so remove_dir_all would target a\nCWD-relative path. Bail with a failure result before any reset runs. Unreachable\non desktop, but hardens a destructive operation (security-review follow-up).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(privacy): report a partial factory reset when board sessions remain\n\nThe factory reset wiped the stores but returned success:true even when the\nbest-effort browser-state removal failed, so the user was told the reset was\nclean while authenticated board sessions could remain on disk. Track the wipe\noutcome and return { success:false, browserStateRetained:true } on failure; the\nSettings reset action now warns the user instead of silently over-reporting.\n\nAddresses CodeRabbit review on #564.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(credentials): assert the keyring probe directly; document invariants\n\nAssert probe_keyring_available() directly instead of the memoized is_available()\nwrapper, so the test no longer depends on OnceLock init ordering. Add doc notes\nthat is_available() is process-sticky (memoized, won't re-probe mid-session) and\nthat document_meta_preamble's doc_kind is baked unescaped into Typst source and\nmust be a static literal.\n\nAddresses @claude review on #564.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* docs: reframe the privacy guarantee as storage/telemetry, not \"never leaves\"\n\nThe guarantee claimed personal data \"never leaves your device\" while the same\nADR permits AI-provider egress — generating a résumé/cover letter sends the\nrésumé + job text to the provider. Reframe consistently (README, SECURITY,\nCONTEXT, ADR 0005) as: no off-device storage, no telemetry, no app-operated\nbackend, with the configured AI provider (and user-invoked scraping/search/\ngeocode/updater) as explicit egress exceptions. Also sanitize literal OS paths\nin DEVELOPMENT.md and note the structured-output override in the prompts README.\n\nAddresses CodeRabbit review on #564.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* ci: block a blank changelog pre-publish via a semantic-release plugin\n\nThe empty-notes guard ran as a post-publish workflow step, so it only reddened\nthe run after a blank changelog already shipped. Replace it with a local\nsemantic-release plugin (scripts/guard-empty-release-notes.cjs) whose prepare\nhook throws on empty notes — ordered after release-notes-generator and before\npublish, so a blank release aborts before the changelog is written, committed,\nor published. Pure JS reading the context (no shell, no injection); covered by a\nunit test in a new scripts vitest project. Removes the superseded release.yml step.\n\nAddresses CodeRabbit + @claude review on #564.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test: make the scrape-progress mock observable\n\nThe scrape.onProgress mock returned a no-op unsubscribe and never emitted, so\ncreateMockClient() couldn't exercise the progress path. Back it with an in-memory\nhandler set + an emitScrapeProgress helper so renderer tests can drive it.\n\nAddresses CodeRabbit review on #564.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(privacy): type the factory-reset command with its real result shape\n\nprivacy_reset_app returns a JSON object, but the contract still declared the\ncommand as returning void, forcing a void->object cast in the consumer that\nfailed CI typecheck (TS2352). Add PrivacyResetResult { success, error?,\nbrowserStateRetained? } and type the command to return it, so the cast is gone\nand the seam is honest. Mock returns the shape.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-08T15:21:59+02:00",
          "tree_id": "e59ecd7308bedbcbbc2e6b96b550015a5e622223",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/eb7023079ff0c344ec292c676427eb3496e13b83"
        },
        "date": 1783517496339,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1961224,
            "range": "± 56080",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2612122,
            "range": "± 62079",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 299490,
            "range": "± 3642",
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
          "id": "eec1b72ada2b1274ed080f0f6e0e73e13f64f455",
          "message": "chore(limits): raise the per-provider daily request ceiling to 4000 (#566)\n\nDouble the runaway-cost backstop from 2,000 to 4,000 accepted AI requests per\nprovider per UTC day, for headroom on heavy autopilot/generation days. Still a\ngenerous safety cap that only a pathological loop trips — not a token/cost budget.\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-08T16:12:24+02:00",
          "tree_id": "ee152afd72af0d2b7d056888fa2b5434072d259c",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/eec1b72ada2b1274ed080f0f6e0e73e13f64f455"
        },
        "date": 1783520494970,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1889110,
            "range": "± 56405",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2535750,
            "range": "± 40411",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 297643,
            "range": "± 6548",
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
          "id": "46f7825c9f1eae6f96892ba91d22ead315c40864",
          "message": "fix: floor aggregator sub-day date filters to 3 days (#569)\n\n* fix(scraping): floor aggregator sub-day date filters to 3 days\n\nAn autopilot on the Aggregator board with a \"Last 24h\" (or any sub-day) date\nfilter returned zero results while the same interactive search worked: Adzuna and\nJSearch have only whole-day recency granularity, and mapping every sub-day token\nto max_days_old=1 / \"today\" zeroed out on a quiet day. Floor the sub-day tier to\n3 days (Adzuna max_days_old=3, JSearch \"3days\") and rely on the existing\nfreshest-first date sort. The paid LinkedIn (Apify f_TPR r-values) path keeps its\nexact sub-day window — it has real seconds granularity.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(scraping): sort jsearch aggregator results newest-first\n\nThe 3-day sub-day floor relies on freshest-first ordering, true for Adzuna\n(&sort_by=date) but not JSearch, which defaults to relevance — so a \"Last 24h\"\nfilter could surface a 3-day-old JSearch job above today's. Append &sort_by=date\nto the JSearch request so the freshness guarantee holds, and document the\nintentional cross-provider recency skew (Apify/LinkedIn keeps a strict <=24h).\n\nAddresses @claude review on #569.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-08T17:30:36+02:00",
          "tree_id": "b829f59d9666e329c30d27c221960d952334b7b1",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/46f7825c9f1eae6f96892ba91d22ead315c40864"
        },
        "date": 1783525814931,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1855461,
            "range": "± 26126",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2418296,
            "range": "± 54802",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 261951,
            "range": "± 3739",
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
          "id": "676f71c6c8100e7e438d0f82b14829720796da27",
          "message": "feat: fix gemini/codex cli providers on windows and add antigravity (#582)\n\n* feat: fix gemini/codex cli providers on windows and add antigravity\n\ngemini/codex install as npm .cmd shims (no .exe), which Windows CreateProcess\nwon't launch, so they were detected \"not installed\" and failed to spawn. Add a\nPATH x PATHEXT resolver in platform/process.rs; run resolved .cmd/.bat shims via\ncmd.exe /C with separate argv. Also: pass codex --skip-git-repo-check (it runs\nin a temp cwd) and filter gemini's credential/telemetry stdout noise out of the\ngenerated text.\n\nAdd a new antigravity CLI backend (agy), mirroring the plain-text gemini path.\nIt is flagged UNVERIFIED — agy isn't installed here, so its flags/stdin behavior\nneed confirming against a real install.\n\nSecurity (reviewed): deliver the prompt over stdin for codex/gemini/antigravity\n(like claude), never as argv. Passing the untrusted prompt (which inlines scraped\njob-description text) as an argv element through cmd.exe was a CVE-2024-24576\ncommand-injection/RCE vector — Rust's batch-arg escaping doesn't engage when the\nprogram is cmd.exe. With stdin delivery, argv carries only static trusted flags.\nAntigravity ships without --yes (no auto-approving tool actions on an untrusted\nprompt).\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(ai-provider): prevent cli-agent pipe deadlock and harden model/effort argv\n\nWrite the prompt to the CLI child's stdin on a detached task so stdout is\ndrained concurrently, instead of awaiting the full stdin write first — a prompt\nlarger than the OS pipe buffer (~64KB; realistic for a cover letter) otherwise\ndeadlocks both pipes and hangs until the 5-minute timeout. Also validate\nmodel/effort against an allowlist before they enter argv (defense-in-depth for\nthe cmd.exe wrapper), add antigravity to the web-search capability test, and\nsuppress a stray leading blank line in the streamed output.\n\nAddresses @claude review on #582.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-08T19:09:41+02:00",
          "tree_id": "f0b0bf8132264a3917c7539fc462600778ae6342",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/676f71c6c8100e7e438d0f82b14829720796da27"
        },
        "date": 1783532208927,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1972600,
            "range": "± 56502",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2660602,
            "range": "± 80772",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287300,
            "range": "± 7384",
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
          "id": "f2ec062620fa186a0060372412c7aad6b1b61c70",
          "message": "chore: batch safe dep + tooling bumps (pnpm, harden-runner, upload-sarif, setup-uv) (#583)\n\n* chore: bump pnpm to 11.10.0\n\n* chore: bump harden-runner 2.20.0, codeql upload-sarif 4.37.0, setup-uv 8.3.2\n\n* ci: sync pnpm/action-setup pins to 11.10.0 to match packageManager",
          "timestamp": "2026-07-08T20:26:37+02:00",
          "tree_id": "e52328b267023757c0d76e391ffc9c90a7a3c71d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f2ec062620fa186a0060372412c7aad6b1b61c70"
        },
        "date": 1783536470387,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1943505,
            "range": "± 63130",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2704591,
            "range": "± 68048",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 285344,
            "range": "± 23151",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}