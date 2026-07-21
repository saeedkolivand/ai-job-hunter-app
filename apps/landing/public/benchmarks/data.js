window.BENCHMARK_DATA = {
  "lastUpdate": 1784629158290,
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
          "id": "abc24e1a2789818d025dcbdcbc60875c80a8ca21",
          "message": "fix(deps): migrate rustcrypto stack to cipher 0.5 and digest 0.11 (#585)\n\nUnblocks held Dependabot bumps #579 (cbc 0.1.2->0.2.1) and #577\n(sha1 0.10.6->0.11.0) by moving the whole Chromium cookie-decrypt crypto\nstack forward together in one branch (they share Cargo.toml + import.rs).\n\ncipher 0.5 family (Unix v10/v11 CBC path): aes 0.8->0.9, cbc 0.1->0.2 (add\nblock-padding feature). API: BlockDecryptMut -> BlockModeDecrypt and\ndecrypt_padded_vec_mut -> decrypt_padded_vec per the cipher 0.5 rework.\n\ndigest 0.11 family (Unix key derivation): sha1 0.10->0.11, hmac 0.12->0.13,\npbkdf2 0.12->0.13. The pbkdf2_hmac::<Sha1> call is unchanged; all three\nalready resolved to digest-0.11 versions in the tree via zip/lopdf.\n\nBehaviour is byte-identical: same algorithms and parameters (PBKDF2-HMAC-SHA1\nsalt saltysalt, 1003/1 iterations, AES-128-CBC, 16-space IV, PKCS7). Verified\nagainst RFC 6070 PBKDF2 vectors plus a CBC round-trip in a standalone crate,\nsince the cfg(not(windows)) path cannot compile on the Windows dev host.\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-08T21:38:54+02:00",
          "tree_id": "0d581c404cd20221f80aeefe1e67cfb93d2d60aa",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/abc24e1a2789818d025dcbdcbc60875c80a8ca21"
        },
        "date": 1783540716035,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1906807,
            "range": "± 68377",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2510493,
            "range": "± 44032",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 283304,
            "range": "± 12258",
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
          "id": "794f38e712729fd1ba1c5379db844babb1a34c55",
          "message": "fix(deps): migrate typst family to 0.15 (#586)\n\n* fix(deps): migrate typst family to 0.15\n\nBump typst, typst-pdf, typst-svg, typst-render from =0.14.2 to =0.15.0 in\nlockstep so the held Dependabot bump builds; a lone typst bump breaks against\nthe 0.14.2 siblings. Add typst-layout =0.15.0 because PagedDocument moved out of\ntypst-library into typst-layout in 0.15.\n\nMigrate the export engine to the 0.15 API:\n- FileId::new now takes a single RootedPath and VirtualPath::new is fallible;\n  build ids via RootedPath::new(VirtualRoot::Project, VirtualPath::new(..)).\n- World::today offset is Option<Duration> (was Option<i64>).\n- VirtualPath::as_rootless_path is deprecated; use get_without_slash.\n- PagedDocument::pages is now a method, not a public field.\n- typst_svg::svg takes &SvgOptions and typst_render::render takes &RenderOptions\n  (pixel_per_pt moved onto RenderOptions). Defaults preserve prior behaviour.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(deps): tolerate typst-pdf 0.15 page-dict whitespace change in test scan\n\nCI caught 3 export-render test failures on the typst 0.15 bump: every_\ntemplate_renders_a_valid_pdf, atelier_multipage_sidebar_renders_once, and\nportrait_multipage_sidebar_renders_once all failed the byte-count assertion\n(count_pdf_pages returning 0) before ever reaching the sidebar-content checks.\n\nRoot cause confirmed empirically (standalone probe against a trivial 1- and\n2-page document, then reverted): typst-pdf 0.15's krilla/pdf-writer backend\nserialises the page dict's type entry with no space (was one space in the\npinned 0.14.2 backend) — pdf-writer/krilla internals, unrelated to any typst\nlanguage layout behaviour. This is not a layout/sidebar regression: the\nAtelier/Portrait \"sidebar renders once\" guard is plain typst markup\n(context + counter(page), a stable introspection primitive) untouched by the\nRust-host crate bump, and the single-column smoke test that already exercises\npdf-extract text extraction against a 0.15-rendered pdf passed in the same\nCI run.\n\ncount_pdf_pages now tolerates zero-or-more spaces in the dict entry so it\nmatches both writer formattings and won't silently regress to zero again on\nthe next pdf-writer whitespace tweak.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-09T07:55:35+02:00",
          "tree_id": "5722754dd178e94bc5dfc719b03daef97dad3424",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/794f38e712729fd1ba1c5379db844babb1a34c55"
        },
        "date": 1783577855050,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1891411,
            "range": "± 55785",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2550152,
            "range": "± 32634",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 290264,
            "range": "± 5017",
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
          "id": "8fb0522a57d6ea3ed6b56653f3122c96d7566269",
          "message": "fix: autopilot aggregator zero-jobs + move export diagnostics to developer settings (#587)\n\n* fix: stop autopilot aggregator silently zeroing on a guessed market\n\nAutopilot targets are commonly saved with a location but no geocode-picked\ncountry_code (buildDefaults/JobsPage only ever seeded location). The\naggregator board then defaulted the missing country to \"de\" (a GUESS) and,\nwhen Adzuna returned Ok(empty) for that wrong market, treated it as a\ngenuine zero and never consulted JSearch - the autopilot aggregator\nzero-jobs bug.\n\n- scraping/boards/aggregator: primary_chain now treats an empty result from\n  a GUESSED country + a real location as untrustworthy and falls through to\n  JSearch or the existing diagnostic, exactly like the unsupported-country\n  guard. A guessed country with no location (the keyless German default)\n  is unaffected.\n- job_preferences: add an optional countryCode column/field so a saved\n  preferred location can carry its real country.\n- commands/autopilot: derive country_code at save time from the location\n  via the existing geocode service when the user didn't pick one.\n- autopilot wizard defaults + JobsPage prefill now seed countryCode\n  alongside location.\n\nNo migration needed for already-saved autopilots: the aggregator guard\nabove covers a legacy country_code=None record on its very next run without\nany stored-data change.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore: restore typst 0.15 + benchmark data reverted by a criss-cross merge\n\nThe two local syncs with origin/fix/autopilot-aggregator-zero-jobs (after\nGitHub's \"Update branch\") produced a criss-cross merge whose auto-selected\nbase predated PR #586 (typst family 0.14.2 -> 0.15.0), silently reverting\nCargo.toml/Cargo.lock/typst_engine and the benchmark data snapshot back to\npre-#586 state. None of these files are part of this PR's fix; restore them\nbyte-for-byte from origin's tip.\n\n* fix: address code review findings on the aggregator zero-jobs fix\n\n- privacy: the guessed-market-empty fallback no longer interpolates the raw\n  user-entered location into the log warning or the persisted diagnostic\n  Error (both in scraping/boards/aggregator/mod.rs's primary_chain) - only\n  the guessed country code and the JSearch remedy remain. Test updated to\n  assert the generic \"supplied location\" phrasing + assert the raw location\n  string is absent, instead of asserting it's present.\n- correctness: country_code_from_suggestions (commands/autopilot.rs) now\n  scans for the first suggestion that actually carries a countryCode\n  instead of only inspecting the first entry, so a leading hit with an\n  absent/null countryCode no longer blocks a usable later one. New test\n  covers both the missing-key and explicit-null cases.\n\n* fix: move export diagnostics from about card to developer settings\n\nThe export-diagnostics control was misplaced under the \"Fund the hunt\"\ndonations card. Move the caption, button, and handler into\nDeveloperPreferences, re-namespace its 4 i18n keys from\nsettings.about.exportDiagnostics* to settings.developer.exportDiagnostics*,\nand update the settings search index + related tests to match.\n\n* fix(autopilot): validate derived country code and cap save-time geocode\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* chore: change app/tauri to app/desktop in coderabbit\n\n* test(settings): cover export-diagnostics error paths in developer settings\n\nAdd tests for the two handleExportDiagnostics error branches (mutation\nresolves success: false, and mutateAsync rejects) and strengthen the\nexisting success test to assert the destination path passed to\nexportDiagnostics and revealItemInDir. Also fixes the beforeEach not\nclearing revealItemInDir's call history between tests, which was\nmasking the new assertions.\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-09T09:53:24+02:00",
          "tree_id": "c33a214ae885c93c2c059956f0fddb83bb26a616",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/8fb0522a57d6ea3ed6b56653f3122c96d7566269"
        },
        "date": 1783584464739,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1931985,
            "range": "± 81169",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2625582,
            "range": "± 35154",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 305679,
            "range": "± 7183",
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
          "id": "360dc7ebd734aab72e3b0286649f2f1956c0c663",
          "message": "fix: aggregator low-count filtering + resume experience translation (#588)\n\n* fix(scraping): clean adzuna location filter and broaden sparse results\n\nAggregator (Adzuna) autopilots returned almost no jobs for German\nsearches (Köln -> 4, germany -> 1) while LinkedIn returned 49.\n\n- adzuna_where(): send only the first comma-segment as Adzuna's `where`\n  (e.g. \"Köln, Deutschland\" -> \"Köln\"). The country is already the market\n  path segment, so a trailing \", Country\" only over-narrows the geocode.\n- broaden-on-near-empty: when an explicit country is set and a non-empty\n  `where` returns fewer than ADZUNA_BROADEN_FLOOR (3) hits, retry once\n  country-wide (where=\"\"), same query/date window. Gated on\n  !country_guessed so the guessed-market -> JSearch fallback (which keys\n  off Adzuna returning empty) is preserved for foreign locations.\n- broaden log emits counts only (no raw location; PII).\n\nDate-filter buckets, sort_by=date, and the 50-result page are unchanged\nso \"24h\"/\"week\" labels stay honest; Adzuna's thin fresh-DE feed still\nreturns modest date-filtered counts by design.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* fix(prompts): translate resume experience bullets into target language\n\nA target-language resume translated the professional summary but left\nthe work-experience bullets in the source language. The target-language\ndirective existed only once globally, while the experience-section\ninstructions said \"reorder/condense EXISTING bullets\" with no language\ndirective, so the model preserved source-language bullets.\n\n- Add a per-section directive to the Work Experience block to write every\n  bullet in the target language, translating source-language text.\n- Add a system-prompt CORE RULE covering summary + every experience/skills\n  bullet. Employer/company names, titles, and dates stay factual.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* style: apply rustfmt to aggregator broaden retry\n\nThe broaden-retry block was committed without running `cargo fmt`, so CI's\n`cargo fmt --all -- --check` gate failed (mod.rs:340/354/435 line-wrapping).\nNo logic change — rustfmt output only.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n* test(scraping): unit-test the adzuna broaden decision via should_broaden\n\nAddresses the claude[bot] review test-gap finding on #588: the broaden\ngate (!country_guessed && where non-empty && count < floor) had no test —\nno HTTP-mock infra reaches AdzunaProvider::search, so the country_guessed\nsuppression that protects the guessed-market -> JSearch fallback was\nunverified. Extract it into a pure `should_broaden` predicate and unit-test\nall four cases (incl. the guessed-market regression guard). Behavior\nunchanged — same boolean expression, now named and callable from tests.\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
          "timestamp": "2026-07-09T13:09:28+02:00",
          "tree_id": "a1483e819caebc93b7aa79b59020d216be6a76de",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/360dc7ebd734aab72e3b0286649f2f1956c0c663"
        },
        "date": 1783595980297,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1912734,
            "range": "± 68543",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2667895,
            "range": "± 26291",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 314191,
            "range": "± 6472",
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
          "id": "0eda637072557b64c13e35dc5db54f828ab12785",
          "message": "feat: merge modern into classic and add per-export document accent (templates pr 1/6) (#590)\n\n* feat(export): merge modern into classic, render classic parametrically, add document accent\n\nRemoves the modern template (color-twin of classic): saved modern ids\ndeserialize to classic permanently. Deletes the bespoke classic.typ —\nclassic now renders through the shared parametric single_column.typ\n(reviewed small spacing drift accepted). Adds a per-export document\naccent override: ExportRequest.accent flows through the documented\nrender seam for pdf, and a single-source validated color override for\ncover letters and docx, so pdf and docx accent behavior cannot drift.\nPreview svg regeneration for classic is pending ci (host cannot execute\ncargo tests); modern previews deleted.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(ui): add document accent picker and update gallery for modern removal\n\nNew AccentPicker (template-default chip, seven curated swatches,\nvalidated custom hex) surfaced in the generate wizard, generation card,\nand tailor flow; accent state lives beside templateId per export and\nnever touches app theming (adr 0004 stays the app-ui accent). Roving\ntabindex keeps one tabbable element when a custom value is active and\nclearing the custom field resets to template default. Modern removed\nfrom the shared union, registry, captions, defaults, and tests; en and\nde strings added.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* test(export): remove redundant closure flagged by ci clippy\n\nci runs clippy 1.97 with -D warnings; the closure around generate_pdf\nin validate tests is redundant there. mechanical one-line fix.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* chore: ignore impeccable plugin cache dir\n\nmachine-local hook cache with absolute paths; never committed (path\nprivacy).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-10T12:24:25+02:00",
          "tree_id": "04d2c2b9b5ea3e5be343d48fb9138a5d241ef50e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/0eda637072557b64c13e35dc5db54f828ab12785"
        },
        "date": 1783680338865,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2218776,
            "range": "± 60900",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2784277,
            "range": "± 57816",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 325370,
            "range": "± 9146",
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
          "id": "256ee72fb933ccb95a9ca05d0804094a93a25905",
          "message": "feat: template tiers with grouped gallery and lebenslauf ats toggle fix (templates pr 2/6) (#591)\n\n* feat(export): add template tier metadata distinguishing ats and design templates\n\nTemplateTier { Ats, Design } on the template registry — render-neutral\nmetadata mirrored by the frontend. ats: classic, swiss minimal,\nacademic, meridian, throughline; design (photo or multi-column):\natelier, portrait, lebenslauf. Adds a tier pin test and a lebenslauf\nats-mode-drops-photo render test.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(ui): group template gallery by tier and fix lebenslauf ats toggle\n\nGallery splits into ATS-Safe and Design sections with tier badges; the\nats-toggle gate and all four reset call sites switch from\ntwo-column-only to design-tier, so lebenslauf (single column + photo)\nfinally surfaces the toggle everywhere and keeps the user's choice.\nTier-aware hint copy (collapse vs photo removal, inclusive for\nphoto-bearing two-column templates) in en and de.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(ui): apply design-tier reset to recommendation apply and tier-aware toggle tooltip\n\nReview follow-ups: the fifth ats reset site (template recommendation\napply) now gates on design tier like the other four, so applying a\nlebenslauf recommendation keeps the user's photo-drop choice; the\ntailor-flow toggle tooltip picks the collapse or photo copy per\ntemplate instead of the generic hint.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-10T14:13:14+02:00",
          "tree_id": "87b4b236cddaf5f85de277588331141858c6d90e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/256ee72fb933ccb95a9ca05d0804094a93a25905"
        },
        "date": 1783686126756,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2103768,
            "range": "± 31400",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2632421,
            "range": "± 46440",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 305949,
            "range": "± 13809",
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
          "id": "1172aff7a72ebea50954e612846f88cc113b9137",
          "message": "feat: cadence and regent single-column templates (templates pr 3/6) (#592)\n\n* feat(export): add cadence and regent single-column templates with tracking and underline knobs\n\nTwo ats-tier templates on the parametric renderer: cadence (inter,\nblue-grey accent, letter-spaced all-caps ruled headings, underlined\nlinks, 28pt name) and regent (source serif 4, deep burgundy accent,\nsmall-caps serif headings, 26pt name). Adds backward-compatible\nheading_tracking and link_underline style knobs (defaults preserve\nexisting output) and threads the previously-dead rule_thickness into\nthe renderer. Regent uses real typst smallcaps (extraction-safe);\nvisually inert until a smcp-capable source serif 4 ships (follow-up).\nCount pins 8 to 10; showcase generator grid widened (latent panic fix).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(ui): register cadence and regent in the template gallery\n\nRegistry records, captions, and shared-contract union entries mirroring\nthe rust configs; count pins 8 to 10; missing preview svgs exercise the\nicon fallback pending ci regeneration.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs(export): clarify rule thickness zero means default not suppression\n\nReview follow-up: rule presence is owned by section_style; 0.0 falls\nback to the house 0.5pt.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-10T15:28:44+02:00",
          "tree_id": "0373dd0e35e19c11a3bf77dac47319812a60da2f",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/1172aff7a72ebea50954e612846f88cc113b9137"
        },
        "date": 1783690709582,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2149557,
            "range": "± 70123",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2574204,
            "range": "± 51292",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 293654,
            "range": "± 9814",
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
          "id": "4a5d18c955c270d0bf272fb33acfc612b0c0b6ff",
          "message": "feat: aria and saffron photo templates with per-template placement (templates pr 4/6) (#593)\n\n* feat(export): add aria and saffron photo templates with per-template section placement\n\nTwo design-tier two-column photo templates: aria (right untinted\nsidebar, rectangular top-right photo, manrope 30pt name, slate accent,\ntracked hairline headings, name-only fallback) and saffron (left tinted\nsidebar, ringed circular photo, source serif 4 small-caps + inter,\nterracotta accent, monogram fallback). placement_for becomes\ntemplate-aware — aria keeps education in the main column and saffron\nkeeps certifications there; existing templates byte-identical, pdf and\ndocx share the same id-keyed placement. Both templates linearize and\ndrop the photo under ats mode. Count pins 10 to 12.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(ui): register aria and saffron in the template gallery\n\nRegistry records, captions, shared-contract union, and two-column set\nentries mirroring the rust configs; count pins 10 to 12; missing\npreview svgs exercise the icon fallback pending ci regeneration.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(export): grapheme-safe monogram initials and correct ats reading-order test\n\nThe no-photo monogram in saffron and portrait sliced names at byte\noffsets, panicking on multi-byte leading characters (a name like\nüber odegaard aborted the export); initials now take the first grapheme\ncluster, verified against a reproduced panic. The aria ats reading-order\ntest wrongly projected the visual placement override onto ats mode —\nlinearize orders by the fixed ats order and ignores placement; both\ntwo-column ats tests now assert the full canonical chain.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-10T16:20:07+02:00",
          "tree_id": "d4cdd803faf7a23a65479a8e35c558254d2e46b6",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/4a5d18c955c270d0bf272fb33acfc612b0c0b6ff"
        },
        "date": 1783693738345,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2162367,
            "range": "± 37054",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2536008,
            "range": "± 40576",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 289302,
            "range": "± 11844",
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
          "id": "1adb050e7827640f636470e2efcc624bf2581b8e",
          "message": "feat: selectable letter layouts inheriting the resume template palette (templates pr 5/6) (#594)\n\n* feat(export): add letter layouts with independent selection\n\nNew letter layout concept: layout owns arrangement only while palette\nand fonts keep inheriting from the resume template. Three layouts:\nclassic (existing letter, byte-identical default), refined (large name,\nright-aligned contact, always-on reference line from the subject,\nsignature space), and banded (angled pale accent band on page one,\nsmall-caps name, footer rule). Market conventions still own semantics —\ndin date placement, betreff handling, and salutations hold across all\nlayouts; band geometry verified against measured header extents at a4\nand us letter. Wire field letterLayoutId defaults to classic.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(ui): add letter layout picker across cover flows\n\nLetterLayoutPicker (radiogroup, three text options) surfaces in the\nwizard for cover and combined targets, the done panel cover tab, and\nthe tailor flow cover tab, all sharing per-export state threaded to\npreview and export identically.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(ui): make letter layout handler optional for resume-only hosts\n\nThe resume builder renders the done panel without a cover tab; the\npicker gates on both the cover tab and a provided handler.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(export): honor the selected letter layout in docx cover exports\n\nThe docx cover path ignored letter_layout, silently exporting the\nclassic arrangement after a banded or refined pdf preview. Refined maps\nfaithfully (right-aligned contact, bottom-border rule, reference line\nwith the same caption-suppression rule); banded approximates the angled\nband with accent-tinted paragraph shading and an uppercased name per\nthe docx small-caps precedent, documented inline. Classic remains\nbyte-identical. Assertions verified against real produced document xml.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(export): parseable caption-suppression expression and longer band fixture\n\nTypst terminates a top-level let at end of line when the expression\nlooks complete, so the multi-line or-expression failed to parse and\nbroke every refined compile; split into single-line bindings with the\nsame logic. The banded page-one-only test fixture gains paragraphs so\nit genuinely reflows to two pages under the corrected band geometry.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-10T18:25:56+02:00",
          "tree_id": "0215819c7846a30a9c6a0f40a33ea6d41c489148",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/1adb050e7827640f636470e2efcc624bf2581b8e"
        },
        "date": 1783701337651,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2152280,
            "range": "± 80372",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2561107,
            "range": "± 50441",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287861,
            "range": "± 5258",
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
          "id": "f060c8de1f15070bd40a0a8959712ed345a526cb",
          "message": "fix: classic cover letters render the recipient address block again (#596)\n\n* fix(export): render the recipient block in classic cover letters\n\nThe classic letter gated recipient emission on order tokens\n(after-date, empty) while the shared conventions fixture supplies\nplacement tokens (left, top-right) verbatim — the vocabularies never\noverlapped, so the inside address was silently dropped for all sixteen\nmarkets since the template's first commit, violating din 5008 for\ndach letters. The template now emits the block for anything except\nbefore-date; render-verified in us and de with position-aware\nregression assertions, and the refined and banded layouts' recipient\nrendering is pinned.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* refactor(export): remove unreachable before-date recipient branch\n\nThe classic letter's before-date recipient branch was dead from day\none (no fixture market supplies order tokens, and LetterOpts has no\npublic injection path), and its top-right-date combo dropped the\nrecipient the same way the after-date bug did. Delete the recip-pos\nbinding and both branches; emit-recipient-block() now runs\nunconditionally at a single point, matching refined and banded.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-10T20:08:32+02:00",
          "tree_id": "2e1e8cec2ad8c4bef9bc34fac0bacc3e033a151a",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/f060c8de1f15070bd40a0a8959712ed345a526cb"
        },
        "date": 1783707463480,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2145113,
            "range": "± 15718",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2552362,
            "range": "± 73532",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 306285,
            "range": "± 16490",
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
          "id": "4245370dc79fe7d0b6f9107c7b8b109bf4bccdba",
          "message": "fix: represent board fetch failures instead of silent zero results (#597)\n\n* fix: represent board fetch failures instead of silent zero results\n\nfetch_json returns AppResult<T> — non-2xx errs with its status code, schema drift errs as parse failure; every board propagates into BoardScrapeSummary.error.\nATS boards err when all company fetches fail (closes the wrong-slug silent zero); germantechjobs/wwr/berlinstartupjobs err on non-200; adzuna/jsearch errors carry the provider name.\nShared all-fail + pagination policy fns in boards/common.rs with unit tests. Trust program PR A (audit 2026-07-10).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: keep cancellation out of board error summaries\n\narbeitnow/arbeitsagentur gain themuse's cancelled-break guard so a user stop is not recorded as a board failure; ycombinator deleted-item skips log at debug; stale fetch_json doc refs updated.\nIncludes the scraping-domain knowledge doc sync (fetch_json error contract, aggregator provider file refs) and persisted lessons.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-10T22:15:57+02:00",
          "tree_id": "aa1c1da4d8ed99710190e6ff0410a33767dc2434",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/4245370dc79fe7d0b6f9107c7b8b109bf4bccdba"
        },
        "date": 1783715110615,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2125377,
            "range": "± 27667",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2540112,
            "range": "± 22571",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 286368,
            "range": "± 9205",
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
          "id": "dcd50817f698a04d314647e85f114c699ec27d41",
          "message": "feat: record honest autopilot run status with per-board summaries (#598)\n\n* feat: record honest autopilot run status with per-board summaries\n\nRuns persist BoardScrapeSummary per board and derive Failed / CompletedWithErrors / Completed from them instead of always recording Completed with 0 found.\nPaginated boards report partial-harvest truncation via a ScrapeContext side-channel into BoardScrapeSummary.truncated; the run payload carries status so the renderer branches honestly.\nRenderer maps failed to the error banner and completedWithErrors to an amber badge (en+de); autopilot store load is per-record tolerant so an unknown future status cannot wipe the file.\nTrust program PR B (audit 2026-07-10).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: clear stale run summaries on cancel and surface truncation in diagnostics\n\nCancel path uses set_run_status_clearing_summaries so a cancelled run never keeps a prior run's per-board data.\nscrape_diagnostics also reports truncated boards; the renderer clears the stale failure banner when a new run starts.\nIncludes docs/knowledge sync and a persisted lesson.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T00:15:43+02:00",
          "tree_id": "5ab4c8a1aba67ec0e3057e6e5a2a14c257849c82",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/dcd50817f698a04d314647e85f114c699ec27d41"
        },
        "date": 1783722288785,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2221359,
            "range": "± 26000",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2703967,
            "range": "± 35902",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 283368,
            "range": "± 11163",
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
          "id": "b5e9c387282e6c41bfb21e6a46ea85e0c4ce0c3b",
          "message": "feat: make location broadening and guessed markets visible (#600)\n\n* feat: make location broadening and guessed markets visible\n\nThe aggregator reports city-to-country broadening and guessed-market fallbacks as per-board notes (machine tokens, country code only) surfaced as informational chips.\nThe autopilot wizard shows the resolved country inline once a location suggestion is picked, so the save-time backfill is a legacy fallback only.\nGeo fields were already part of the replace-vs-append search signature; the missing radius test now pins it. Trust program PR D (audit 2026-07-10).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* test: pin note chips against the all-green collapse\n\nA note-bearing board renders per-board chips instead of collapsing into the all-ok summary; docs/knowledge synced and lessons persisted.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: correct the note mutual-exclusivity condition\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T03:46:01+02:00",
          "tree_id": "e6e646d68ff65b907a8f3432f541ac560161a2bc",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b5e9c387282e6c41bfb21e6a46ea85e0c4ce0c3b"
        },
        "date": 1783734885022,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2157839,
            "range": "± 82517",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2545705,
            "range": "± 59272",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 281815,
            "range": "± 3486",
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
          "id": "0b13fc3bb44286b8941ca84baba8563b04c21b64",
          "message": "feat: collapse cross-source duplicate jobs behind one canonical key (#601)\n\n* feat: collapse cross-source duplicate jobs behind one canonical key\n\nOne shared canonical job key (normalize_job_url, else title+company) drives an engine dedup pass, the autopilot merge, and the renderer mergePostings mirror.\nThe same job no longer lists 2-3 times or re-fires notifications.\nSurvivors merge field-level: incumbent identity kept, longer description adopted (byte length both sides), salary/trust/extra filled from the loser, interactions unioned.\nRust and TS key fns are a documented lockstep pair with verbatim shared test fixtures as the drift guard. Trust program PR E (audit 2026-07-10).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: keep the selected job selected when a duplicate collapses into it\n\nmergePostings reports absorbed ids so the selection reconciler re-points at the surviving row instead of jumping to the top of the list.\nLockstep comments added on both sides of the extra-field fill list; docs/knowledge synced and lessons persisted.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* test: pin enrichment fill-list and non-ascii key parity\n\nGuard test asserts every extra enrichment survives a renderer collapse; a shared non-ascii fixture pins rust/ts lowercasing parity on the fallback key.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T05:26:56+02:00",
          "tree_id": "db7ff23e02af068eca08ea41973a4031a2915288",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/0b13fc3bb44286b8941ca84baba8563b04c21b64"
        },
        "date": 1783740980996,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2195411,
            "range": "± 50584",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2690000,
            "range": "± 100156",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 305015,
            "range": "± 16005",
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
          "id": "7abc20e2f80596c4b5247b93dfe8ae458abf550c",
          "message": "feat: canonical location model with honest per-board location handling (#602)\n\n* feat: canonical location model with honest per-board location handling\n\nLocationSpec derives once from the geo fields the renderer already sends; boards declare supports_location (verified catalog: aggregator, linkedin, arbeitsagentur only).\nThe engine centrally filters non-supporting boards with a conservative predicate: diacritic folding plus a curated exonym table, remote and unknown locations always kept.\nFiltered items never count toward the item cap, so boards keep paginating until enough matching rows arrive.\nEvery non-supporting board notes location-filtered:<n> (including zero) so a board that ignored the location can never read clean; the picker warns before the scrape.\nTrust program PR F (audit 2026-07-10) — the one sanctioned refactor.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* test: cover the location filter note at both picker call sites\n\nReal render tests for ScrapeForm and StepTarget pin the hint's presence when a location is set; docs/knowledge synced and lessons persisted.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: pin the on_item stream contract and warn on poisoned kept mutex\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T07:32:46+02:00",
          "tree_id": "3c51d0bbc253b1f344916fc45ae77319749f82ed",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7abc20e2f80596c4b5247b93dfe8ae458abf550c"
        },
        "date": 1783748625322,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1872865,
            "range": "± 31994",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2320226,
            "range": "± 51163",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 225168,
            "range": "± 3669",
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
          "id": "8b1a16715b0e41117ef397f87cc11e4a6c00f347",
          "message": "fix: live-verified board hygiene with honest failure reasons (#603)\n\n* fix: live-verified board hygiene with honest failure reasons\n\nLive verification found breezy functionally dead (location.state drifted to an object, zeroing every tenant) — fixed with a tolerant per-row parse.\nThemuse, bamboohr, and rippling shapes confirmed with dated notes; pinpoint documented unverifiable. Comeet hidden from the picker while staying dispatchable for saved targets.\nLinkedIn detects the zero-cards soft block as a board error (page 0 always pads cards, so zero is never a genuine empty) and gains country-biased cached geoId resolution.\nRuns where every company slug is invalid now say so instead of returning silent zero, with the all-rejected check guarded against mid-list cancellation.\nTrust program PR G (audit 2026-07-10).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: guard total row drift and centralize ats search finish\n\nA response whose rows all fail to parse now records a fetch failure instead of a silent empty success; the all-invalid message points at the jobs search form.\nShared ats_finish_search puts the cancellation-wins invariant in one tested place across the seven slug boards. Docs synced, lessons persisted.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: anchor geoid country bias to the trailing display-name segment\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T09:09:03+02:00",
          "tree_id": "528af3ab8e28d754c9535be709180a8661a68396",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/8b1a16715b0e41117ef397f87cc11e4a6c00f347"
        },
        "date": 1783754319257,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2233311,
            "range": "± 58842",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2651079,
            "range": "± 44735",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 288298,
            "range": "± 4674",
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
          "id": "75ba0f5f75e960a2d1b5783c8e414ba246e9470f",
          "message": "feat: scheduler retry with honest scores and partial-visibility notes (#604)\n\n* feat: scheduler retry with honest scores and partial-visibility notes\n\nFailed or interrupted autopilot occurrences get one bounded in-process retry with a post-backoff re-check so a recovered slot never double-scrapes.\nA concurrent-run guard dedups double invokes; store write failures are logged loudly.\nAggregator snippet scores are flagged provisional and render muted with an accessible hint until the detail view rescored on full text.\nPersonio joins the shared all-fail semantics; rippling and workable gain the total-drift guard; partial slug rejections and dropped rows surface as chips.\nBoard-native notes win over the location-filtered fill; jobs sorting is deterministic with undated postings in a trailing band.\nTrust program PR H — final PR (audit 2026-07-10).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: pending retries respect pause and provisional flags follow scores\n\nThe failed-arm retry re-reads the record and skips unless still schedulable, so pausing or archiving during the backoff wins.\nmerge_found_jobs carries score_provisional with score in both resurface directions; a manual run blocked by the in-flight guard surfaces an already-running message instead of silent success.\nDocs synced with the program completion note and named fast-follows; lessons persisted.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: type the already-running skip and mute provisional scores everywhere\n\nThe run contract types the skipped payload the guard already emits (closes the CI typecheck failure).\nMatchBand gains a muted prop so a provisional High score renders muted like every other tier, with the test mock deriving from the real tier logic.\nBreezy's rows-dropped count now includes empty-title and invalid-url format drops while excluding duplicate-url hygiene; the aggregator board id is a shared constant; doc inaccuracies corrected.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T11:09:40+02:00",
          "tree_id": "7cf061f96ffcaae1ac36127f89bcd8492e78e88f",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/75ba0f5f75e960a2d1b5783c8e414ba246e9470f"
        },
        "date": 1783761523001,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2162732,
            "range": "± 54501",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2596138,
            "range": "± 39173",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 296939,
            "range": "± 5848",
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
          "id": "d6102dd54b2a8b52b810e7ce6d8cef22905e317a",
          "message": "feat: add detector-resistant sampling params for prose generation (#615)\n\nplumbs topP/frequencyPenalty/presencePenalty/repeatPenalty (optional, zod range-validated) through\nAiGenerateRequest into all four providers; prose surfaces only — resume/analysis/rewrite untouched\nto protect ats keyword repetition.\n\n- provider mapping never serializes null; openai gates all params by supports_temperature (o-series parity)\n- anthropic sends top_p only and never alongside extended thinking; gemini uses generationConfig fields\n- ollama maps options.top_p + repeat_penalty; frequencyPenalty never remapped across semantics\n- per-provider body construction extracted into pure unit-tested build_chat_stream_body fns\n- renderer resolveSampling(): cover 0.8 large/0.58 small (topP 0.9 small tier), answers 0.5 without\n  presencePenalty (grounded surface), email+referral 0.7, interview 0.5\n- shared prose set: topP 0.95, frequencyPenalty 0.3, repeatPenalty 1.15\n- basis: RAID (ACL 2024) — repetition penalty + random sampling drop detector accuracy up to 38 points\n- audited by ai-provider-expert: no high/critical; both medium value-tuning findings applied\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T18:27:33+02:00",
          "tree_id": "9974d5179d9afc625746a2038e618ef9ff2c321d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/d6102dd54b2a8b52b810e7ce6d8cef22905e317a"
        },
        "date": 1783788209387,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1704226,
            "range": "± 13558",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2056048,
            "range": "± 11649",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 231861,
            "range": "± 3082",
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
          "id": "bac6c0a6992f7b099e66073993319dde59bd69bf",
          "message": "fix: fence the scraped job ad as untrusted input in all prompt builders (#617)\n\n* fix: fence the scraped job ad as untrusted input in all prompt builders\n\nthe scraped job ad is the primary attacker-controlled input yet was interpolated raw\ninto <job_ad> fences across 9 builders — no neutralization, no treat-as-data directive —\nwhile company-research/web-search/style-reference content was already fenced. a malicious\nposting with a forged </job_ad> + injected instructions could skew the ats score or bias\nresume/cover/answer generation.\n\n- new exported buildJobAdBlock(jobAd, maxChars) in emphasis.ts: slice (caller budget) →\n  neutralizeFenceTag → fenced block + JOB_AD_UNTRUSTED_NOTE ignore-instructions directive\n- all raw job-ad interpolations routed through it: analyze (brief/task/full), resume,\n  cover-letter, application-questions, application-email, interview-questions, metadata,\n  job-ad-summary. all three analyzer depths now structurally identical (real fence + note)\n- neutralizeFenceTag widened to whitespace-tolerant + forged-opening-tag, case-insensitive,\n  regex-escaped tagName, bounded \\s* (no redos) — hardens all four fences at once\n- benign job ads byte-identical; exact-keyword ats matching unaffected\n- tauri-security-reviewer: no high/critical; both medium findings (whitespace bypass +\n  forgeable full-depth header) fixed in this diff\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* build: delay-load comctl32 on windows so cargo test binaries load\n\nThe lib statically imports comctl32!TaskDialogIndirect (via the wry/muda/rfd dialog path), a ComCtl32 v6-only export. Cargo test\nbinaries get no Common-Controls 6.0.0.0 manifest, so on windows-msvc the loader binds System32 comctl32 v5.82 and the exe aborts at\nload with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139) before main. CI never sees it because tests run on linux/macos.\n\nThe library's own unit-test harness (where nearly all tests live) is a lib target: Cargo has no link-arg category that reaches it\nexcept the crate-wide rustc-link-arg, and a /MANIFEST:EMBED there collides with tauri-build's bin manifest resource (CVT1100 duplicate\nMANIFEST). So instead of manifesting the harness, delay-load comctl32: the v6-only import is no longer bound at process start (it\nresolves on first call, which the tests never make). Bins and dialogs still resolve v6 at runtime via tauri-build's bin manifest, so\nruntime behavior is unchanged.\n\nGated on the build-script target env (CARGO_CFG_TARGET_OS=windows, CARGO_CFG_TARGET_ENV=msvc), not #[cfg(windows)] which reflects the host.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: update cargo test comctl32 fix status and preview-regen notes\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T20:52:33+02:00",
          "tree_id": "af5e417ce474bc55b4afd1230be7eaa58c52fc2f",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/bac6c0a6992f7b099e66073993319dde59bd69bf"
        },
        "date": 1783796517847,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2159161,
            "range": "± 113836",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2591124,
            "range": "± 45017",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 296237,
            "range": "± 4818",
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
          "id": "7e4c72f943fcabb3be6154b234c9fd4401c6fc73",
          "message": "feat: add jooble as a byo-key aggregator fallback provider (#618)\n\n* feat: add jooble as a byo-key aggregator fallback provider\n\njooble (jooble.org/api) adds ~67-country coverage vs adzuna's ~19 markets, patching the\nunsupported-country zero-jobs case. wired as a third fallback tier: fires only when both\nadzuna and jsearch come up unconfigured/empty/err, so it never hammers the undocumented\nquota on searches the primary chain already answers.\n\n- new JoobleProvider in aggregator/providers.rs (JobProvider trait), modeled on jsearch;\n  key read from ai:jooble-key keyring slot, degrades read error to none (optional-key)\n- primary_chain third tier; jsearch ok(empty) short-circuits before jooble, symmetric with\n  adzuna's empty-means-stop rule (regression-tested both edges)\n- routes through fetch_json (failure-representable, 429/503 backoff, per-host rate limit,\n  cancellation); errors prefixed jooble:; external_id prefixed jooble- for dedup\n- security: jooble puts the api key in the URL PATH, so added FetchOptions.redact_path +\n  safe_log_url() to strip the path from any non-2xx/schema-drift log line (default false =\n  byte-identical for the 20+ existing callers). key percent-encoded; params in POST body\n- provider-slots contract + settings AggregatorKeyField (en+de) for the key\n- reviewed by scraping-applier-expert + tauri-security-reviewer: no high/critical\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: surface jooble configured-but-failed instead of a silent empty result\n\nthe jooble tier's Err arm only logged and fell through untracked, so a jooble-only setup\nwhose jooble call failed returned Ok(empty) ('no jobs') instead of a diagnostic error —\nthe exact silent-empty honesty bug the trust program (#597-#604) eliminated. now tracked in\njooble_configured_failed and surfaced last in primary_chain (after adzuna/jsearch configured-\nfailed, before the keyless-empty Ok), carrying jooble's own prefixed message. regression test\njooble_only_configured_failure_surfaces_error added; empty-vs-error edges stay distinct.\n\ncaught by the pre-push review gate (rust-backend-architect/scraping-applier-expert lens) after\nboth domain+security reviews missed it.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: combine all configured-failed aggregator providers into one diagnostic\n\nthe total-failure path returned only the first configured-failed provider in fixed order\n(adzuna→jsearch→jooble), so an adzuna+jooble both-failing setup (no jsearch) showed only\nadzuna's error plus a now-stale 'add a JSearch key' nudge, dropping jooble's failure. now\ncollects every configured-and-failed provider and joins their self-prefixed messages; the\nremedy nudge is generalized to 'add a JSearch or Jooble key' and omitted on multi-provider\nfailures. empty-vs-error distinction and the salvage path unchanged.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T22:49:28+02:00",
          "tree_id": "f4fcd0810dcf34b9030a1e8ed53197573c5fdc58",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7e4c72f943fcabb3be6154b234c9fd4401c6fc73"
        },
        "date": 1783803509394,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2139039,
            "range": "± 108565",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2640614,
            "range": "± 54919",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287702,
            "range": "± 4611",
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
          "id": "c6b8b85d5037c005772a305691c2f6bfb79b6353",
          "message": "fix: parse jooble timezone-less timestamps instead of dropping posted_at (#619)\n\nverified against the live jooble api: 'updated' arrives as '2026-05-15T00:00:00.0000000'\nwith no timezone offset, which parse_from_rfc3339 rejects — so posted_at was silently None\nfor every real jooble job (no dates, broken date filtering). now tries rfc3339 first, then\nfalls back to a naive datetime assumed utc (tolerant of the 7-digit-fraction no-tz form).\nadzuna/jsearch parsing untouched. 3 tests cover the real no-tz shape, a tz-bearing shape, and\na malformed string.\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T23:26:39+02:00",
          "tree_id": "500dc3a46d6751eaa340cbcf10017beb0e147343",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c6b8b85d5037c005772a305691c2f6bfb79b6353"
        },
        "date": 1783805742682,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2159974,
            "range": "± 18605",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2631885,
            "range": "± 32000",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 302236,
            "range": "± 13285",
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
          "id": "ab54a8061ff2908698beb181f058b0299088b7d0",
          "message": "feat: bundle a verified company-to-ats-slug seed directory (data only) (#620)\n\nadds a curated, live-verified (2026-07-11) seed of 59 companies to (ats, slug) across\ngreenhouse/lever/ashby/smartrecruiters/recruitee/personio/workable, 23 of them dach, so the\nexisting ats-direct scrapers can be exercised without the user hand-typing company slugs.\n\n- new scraping/boards/ats_seed module: AtsSeedEntry { company, ats, slug, tld, dach } +\n  static SEED table + all() / by_ats() lookups. data only — NOT in SCRAPERS, no wiring yet\n- ats ids cross-checked against the live Scraper::id() registry in tests (not a hardcoded copy)\n- encodes the verified quirks as data: personio .de/.com tld per company, ashby slug casing\n- doc note flags lever/smartrecruiters slug churn to periodic re-verify\n\nwiring into the engine + autopilot follows in a separate pr.\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-11T23:57:13+02:00",
          "tree_id": "b37209601477b5588ce6a90f5682d42f2bd695a5",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ab54a8061ff2908698beb181f058b0299088b7d0"
        },
        "date": 1783807578058,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2114329,
            "range": "± 11457",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2527593,
            "range": "± 118633",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 289864,
            "range": "± 3449",
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
          "id": "0dd532b51af0e1c62206a56b8a49c8ccd6dcfcf3",
          "message": "feat: route the ats seed into company-scoped boards (engine-side) — DO NOT MERGE until disclosure decision (#621)\n\n* feat: route the ats seed into company-scoped boards (engine-side)\n\nfinishes p2 sourcing depth: the engine now auto-populates a company-scoped ats board's\ncompanies from ats_seed::by_ats(scraper.id()) when the board is selected, requires a company,\nand the run supplied none. so the 7 seeded ats-direct boards (greenhouse/lever/ashby/\nsmartrecruiters/recruitee/personio/workable) finally get exercised — autopilot, which always\npassed an empty companies list, previously skipped every one as needs-company.\n\n- option (b), engine-local: no BoardSearchInput contract change, no scraper changes, no\n  renderer changes. run_boards gains one borrowed &HashMap override param; stays seed-agnostic\n- keyed on scraper.id(); explicit user companies always win (override only when list is empty);\n  unseeded requires-company boards still skip\n- fetch volume bounded per-board (sum of selected boards' slugs, capped by max_boards_per_batch),\n  not 59xN; rotted slugs 404 harmlessly and surface CompletedWithErrors, no silent-empty\n- verified all 7 boards consume seeds as-is (personio self-probes .de/.com, workable v1 GET,\n  ashby/smartrecruiters preserve casing); doc comments corrected\n- reviewed by scraping-applier-expert: no correctness blockers\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat: disclose curated ats-seed companies in the board pickers\n\ncompanion to the engine ats-seed routing: since selecting a company-scoped ats board now\nsilently queries a curated company set, both board pickers (autopilot wizard + manual jobs\nsearch) now disclose which companies. addresses the review's honesty concern.\n\n- backend: ScraperCatalogEntry/BoardCatalogEntry gain seededCompanies (serde-renamed camelcase),\n  populated from ats_seed::by_ats(scraper.id()); empty for non-seeded boards\n- frontend: shared components/scrape/SeededCompaniesNote (beside LocationFilterNote), rendered by\n  both StepTarget and ScrapeForm; shows first 5 names + \"+N more\" (full list on hover), role=note,\n  i18n en+de with pluralized suffix. only renders for selected boards with a seed\n- reviewed by frontend-reviewer: no high/critical\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-12T01:34:19+02:00",
          "tree_id": "f7f01b724acf3ca7d4d3aacb83da93ac357d2738",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/0dd532b51af0e1c62206a56b8a49c8ccd6dcfcf3"
        },
        "date": 1783813406877,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2118534,
            "range": "± 77996",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2538844,
            "range": "± 44958",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 289450,
            "range": "± 5627",
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
          "id": "9f93f42db49beea10f9c5a18930bbb3e400b12ae",
          "message": "feat: ai spend visibility — real per-provider token + estimated cost tracking (#624)\n\n* feat: real per-provider ai token + estimated-cost tracking (backend)\n\ncaptures the REAL input/output token counts each provider reports (openai include_usage,\nanthropic message_start/delta, gemini usageMetadata, ollama eval counts; cli agents report\nhonest zero; ollama-cloud kept paid), persists per-call to a new spend sqlite store, and\nconverts to an ESTIMATED cost via a static prefix-matched rate table (tokens exact, $ is a\nlist-price estimate — byo-key has no billing api). ai_spend_summary ipc + useSpendSummary hook.\nrecorded at the two shared chokepoints (stream finish + Completer::complete) so autopilot/\nagent/pipeline get tracking with zero call-site changes. settings panel follows.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: record agent + embedding spend, honest coverage, estimate edge-cases\n\nai-provider-expert review of the spend backend: (HIGH) agent tool-calling turns were unrecorded\nwhile the doc claimed coverage — the biggest token consumer was invisible; now AgentTurn carries\nUsage (per-provider turn parsers populate it, single_shot_turn uses complete_with_usage) and\nCompleter::chat_with_tools records it. (MED) embeddings now tracked via embed_with_usage\n(embed_text chokepoint: manual embed, match-score, reembed-all); research/web-search explicitly\nEXCLUDED + documented (per-search pricing, not token-rate); models/ prefix stripped in\nestimate_cost; openai-compatible at a localhost base_url is $0 (real local calls no longer show a\nfake cost); stream latest-usage-wins + record-once now tested; now_ms bound once.\n\ncovered: stream, complete, agent tools, embed. excluded (documented): research/web-search.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat: ai spend settings panel (today estimate + per-provider breakdown)\n\nsettings → ai 'ai spend' panel consuming useSpendSummary: today's estimated $ + exact token\ntotals + per-provider rows (local/cli read 'local — free'), with the honest disclaimer that\ntoken counts are exact but the dollar figure is a list-price estimate, not an actual charge.\nloading/empty/error states, en+de, search-indexed. also fixed two pre-existing branch issues:\nAiSpendSummary barrel re-export (tsc portability) + the search-anchor manifest count.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: keep the spend store tauri-free + register it as an l1 domain module\n\nthe pre-push architecture test caught the spend store violating the L0-L3 layer rules:\nunclassified module, rusqlite outside the R3 store allowlist, and tauri::AppHandle imported\ninto a data-layer store (R2). classified spend as L1 (domain, like ai_generations), added\nspend/mod.rs to R3_ALLOW, and moved the record_usage AppHandle->try_state->store.record hop\nup to the command layer (commands/ai_provider). spend/mod.rs is now tauri-free; behavior\nunchanged. architecture test 11/11.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: log dropped spend rows + live-refresh the panel (coderabbit)\n\ncoderabbit #624 (all trivial/minor): (1) spend record() now logs an insert failure instead\nof silently swallowing it — still non-propagating so a store error can't break an ai call, but\na dropped spend row is no longer invisible; (2) useSpendSummary polls (refetchInterval 30s +\n20s stale, while the settings panel is mounted) so totals refresh live as a generation finishes;\n(3) DataStore::import negative-path tests (non-array + bad-row → err, store untouched); (4)\nsub-cent (<$0.01) format test. skipped: cli-agent zero-row pruning (premature).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-12T09:07:11+02:00",
          "tree_id": "1dd764540507f0b2a009cfe3416dbd60430df966",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9f93f42db49beea10f9c5a18930bbb3e400b12ae"
        },
        "date": 1783840573041,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2107903,
            "range": "± 84454",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2531110,
            "range": "± 84679",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 289848,
            "range": "± 2607",
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
          "id": "49ba3e60e8bdd80be68ece395234a6f5e062ab87",
          "message": "feat: assisted autofill for application forms from contact profile (#625)\n\n* feat: assisted autofill for application forms from contact profile\n\nAdd a user-initiated \"Fill this form\" action to the MV3 extension that fills empty\nform fields on the current page from the user's Contact Profile (name, email, phone,\nlocation, linkedin, github, website). Mirror of Extension import: writes the user's own\ndata out instead of reading a job in.\n\nDesign (ADR-0009, grill-locked):\n- User-gestured, no broad host_permissions: activeTab + executeScript on click, so it\n  works on any site without standing access and keeps the AMO data-collection [\"none\"] stance.\n- Generic tiered matcher (autocomplete -> label -> name/id -> placeholder); fills empty,\n  unambiguous fields only; skips a denylist of sensitive/ambiguous keys.\n- Never submits: the extension fills, the human reviews and clicks submit.\n- PII travels over the existing authenticated loopback bridge via a new profile.get ->\n  profile.result message, fetched fresh at fill time, never persisted in chrome.storage.\n- Opt-in, default OFF, enforced desktop-side: the app refuses profile.get when the toggle\n  is off, so disabling it actually stops PII from leaving the device.\n- Transparent + honest limits: shows what it filled; no resume FILE upload (browser-forbidden),\n  complex custom ATS (Workday shadow DOM) fill partially at best. Disclosed in README + privacy page.\n\nisHidden walks getComputedStyle for every ancestor so CSS-class honeypot fields are skipped;\nthe bare-name fallback excludes education fields; the pairing-token threat model notes that a\nharvested token also reads the Contact Profile while autofill is on.\n\n* fix: catch opacity and off-screen honeypots in autofill field detection\n\nExtend the hidden-field check to also skip fields hidden via opacity:0, off-screen\nabsolute/fixed positioning (left/top <= -9999px), and zero-size (0x0), on top of the\nexisting display:none / visibility:hidden / CSS-class checks. All heuristics stay\ncomputed-style only (no getBoundingClientRect/offsetWidth, which jsdom zeroes out) so a\nfilled invisible honeypot can no longer flag the user as a bot. Adds coverage for the two\nnew honeypot shapes (with a normal-sibling false-positive guard), the autofill-toggle\nfailure path, and the popup fill-button click flow. Addresses CodeRabbit review on #625.\n\n* test: cover autofill background orchestration and refresh docs and store copy\n\nAdd background.test.ts (6 cases) exercising the fill dispatcher's real paths: not-paired\nshort-circuit, desktop refusal (opt-in off + transport reject), no-active-tab, malformed\ninjected result, and the success path — closing the AI-review-flagged coverage gap. Soften\nthe hidden-field doc comment to state actual coverage honestly (clip-based and single-\ndimension-zero .sr-only shapes are deliberately not treated as hidden, since those can be\nlegitimate screen-reader fields). Refresh the store description to disclose the new\nfill-form capability alongside job import. Addresses the @claude review on #625.",
          "timestamp": "2026-07-13T17:26:35+02:00",
          "tree_id": "eff3d2171f26cde8c2010bb9abb2d7c5693f4bad",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/49ba3e60e8bdd80be68ece395234a6f5e062ab87"
        },
        "date": 1783957009443,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2149582,
            "range": "± 26134",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2605621,
            "range": "± 55409",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 295630,
            "range": "± 17214",
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
          "id": "548f84196f4ee6e493e04059cab19faa223897c4",
          "message": "feat: mutual hmac handshake for the extension bridge (protocol v2) (#627)\n\n* feat: mutual hmac handshake for the extension bridge (protocol v2)\n\nReplace plaintext-token-per-frame auth with a mutual HMAC-SHA256 challenge-response so\nthe pairing token is used only as an HMAC key and is never transmitted. Handshake:\nhello{protocol,clientNonce} -> challenge{serverNonce} -> auth{proof} -> auth.ok{serverProof},\nthen session-authorized frames carry no token. Both sides prove knowledge of the token over\nper-connection nonces; the extension sends zero PII until it has verified the server's proof,\nso a loopback port-squatter that does not know the token harvests no reusable secret and no\ndata. Domain-separated proofs (no reflection), fresh CSPRNG nonces (no replay), constant-time\nverification both sides, and a cross-impl known-answer vector pinning the Rust and Web-Crypto\ncanonicalizations together. Force cutover: legacy plaintext frames get update.required, and a\nnew outdated phase surfaces the version mismatch on both sides. The send path is gated on the\nauthenticated session (connected phase), never on transport liveness. See ADR-0010.\n\n* ci: allowlist the hmac known-answer test vector in gitleaks\n\nThe bridge handshake ships a deterministic cross-impl known-answer vector whose fixed,\nobviously-fake pairing token trips gitleaks' generic high-entropy token rule. Add a\nnarrow value-scoped allowlist (extends the default ruleset; excludes no rule or path) so\nthe fixture passes while every other secret is still caught.\n\n* build: drop the duplicate direct hmac dep breaking all-features clippy\n\nThe new hmac 0.12 (handshake) and the pre-existing Unix-only direct hmac 0.13 (Chromium\ncookie pbkdf2) both landed in the extern prelude as `hmac`, so `use hmac::{Hmac, Mac}`\nwas ambiguous under `cargo clippy --all-features` (E0464), which CI runs but the local\n`--all-targets` clippy does not. The direct hmac 0.13 entry was unused — the cookie path\ncalls pbkdf2::pbkdf2_hmac and pbkdf2 pulls hmac 0.13 transitively via its own hmac\nfeature — so removing the direct dep clears the collision with no behavior change.\n\n* docs: align bridge comments and the stray-token test with protocol v2\n\nUpdate the extension_bridge doc comments left describing the removed v1 per-frame-token\nmodel (handle_connection, native_host, auth, the top-of-file security model, regenerate_token,\nand a README line) to the v2 mutual-handshake + session-auth reality, and fix the token-free\nenvelope test to actually parse a token-bearing frame and assert the stray token is stripped\n(was a no-op destructure with a mismatched name). Comment/test only. Addresses the CodeRabbit\n+ AI-review-gate findings on #627.",
          "timestamp": "2026-07-13T21:28:38+02:00",
          "tree_id": "cb21f74510d3af14959f6eea0af6e8fc2c894e9d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/548f84196f4ee6e493e04059cab19faa223897c4"
        },
        "date": 1783972091673,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2127282,
            "range": "± 92910",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2528489,
            "range": "± 102539",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 285200,
            "range": "± 1954",
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
          "id": "3e2f32549a4b8eefdf30a11411240c942ea9e0b7",
          "message": "fix: derive multi-board batch cap from the scraper registry so selecting all boards works (#629)\n\n* fix: derive multi-board batch cap from the scraper registry so selecting all boards works\n\nSelecting a 7th board in the autopilot wizard (arbeit now onward in catalog order) failed\nvalidation with a misleading generic error: the fixed MAX_BOARDS_PER_BATCH = 6 cap predates\nthe board-catalog expansion to 23 scrapers.\n\nThe engine cap is now registry-derived (boards::all().len()), so it scales with the catalog\nwhile keeping the CWE-770 request-amplification defense (dedup + registry-size truncation).\nShared Zod schemas mirror it (BOARD_IDS.length for the enum-typed scrape request, a generous\nsanity bound for the relaxed autopilot boards) and the wizard schema drops its .max(6).\nDocs updated (PATTERNS.md, anti-abuse-limits.md).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: address review findings on the board-cap pr\n\n- landing/how-it-works.html: drop the stale 'up to 6 boards per run' claim (now catalog-bound)\n- renderer schema test now validates the full BOARD_IDS catalog instead of 20 arbitrary ids\n- docs: thin pointers to max_boards_per_batch() instead of copied formulas; scaling claim\n  qualified by the shared-schema bounds\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-13T23:56:49+02:00",
          "tree_id": "5b560bd549335fac29139499da2ecb480c35e0e9",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/3e2f32549a4b8eefdf30a11411240c942ea9e0b7"
        },
        "date": 1783980333839,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1637350,
            "range": "± 49838",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 1994850,
            "range": "± 117509",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 199339,
            "range": "± 12765",
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
          "id": "63994473b965ebeeb351474862737e769441dc38",
          "message": "feat: add applied.check bridge verb with adaptive popup status (#631)\n\n* feat: add applied.check bridge verb with adaptive popup status\n\nImplements the reserved applied.check protocol verb end-to-end: the\npopup auto-checks the active tab URL on entering the connected phase\n(fire-and-forget, soft-fail silent) and the desktop answers with a pure\nread-only store lookup — canonical url, normalize, find by job url. The\npopup renders an already-in-pipeline status line and relabels the\nimport button. No protocol bump, no manifest changes, no consent gate\nneeded (device-local metadata over the authenticated loopback bridge).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: reset applied status line and import label on phase change\n\nClear the appliedCheck status line and import button label whenever the\nconnection leaves connected, and synchronously before each fresh check, so\nstale text from a prior page never flashes after a reconnect.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: include year in applied date when not current year\n\nformatShortDate omitted the year unconditionally, so an applied date from a\nprior year read ambiguously (e.g. \"Jun 12\"); add year:'numeric' when the\ndate's year differs from today's.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: guard applied auto-check against stale in-flight responses\n\nA disconnect->reconnect re-enters connected and fires a fresh\nappliedCheck while a previous one may still be in flight; without a\ngeneration guard the stale response (or its catch) could resolve\nafter the newer check and overwrite its rendered state. Add a\nmodule-level generation counter that bails before any DOM mutation\nin both the success and catch paths when a newer check has since\nstarted.\n\nAlso documents the consent-gate boundary (read-only own-metadata\nlookups need no desktop opt-in vs. fresh-PII/billable verbs that do)\nin extension-protocol-constants.ts, and notes the wire-error\nsentinel-text discipline on applied_result_reply's Err arm in the\nbridge (comment-only, no behavior change).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-14T06:34:20+02:00",
          "tree_id": "595d022d7a5219d09f16225e19c3a195a703d556",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/63994473b965ebeeb351474862737e769441dc38"
        },
        "date": 1784004245300,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2060526,
            "range": "± 70065",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2423789,
            "range": "± 74433",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 225259,
            "range": "± 7935",
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
          "id": "4580e112e58075ad5614654024d0bb18aa343848",
          "message": "feat: add status.update bridge verb for one-click mark as applied (#632)\n\n* feat: add status.update bridge verb for one-click mark as applied\n\nThe popup shows a mark-as-applied button when the checked page maps to\na saved application. The desktop enforces the transition with an atomic\ncompare-and-set (transition_status_if: update guarded on current status\nplus status event in one transaction) so only saved to applied can ever\nbe written, even under concurrent writers. Errors are user-facing fixed\nsentinels; untracked pages keep using import with the applied checkbox.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: preserve first applied timestamp in status transition cas\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: require explicit saved status and discriminate status result contract\n\nCodeRabbit fixes: resolveShowMarkAppliedButton now requires an explicit\n`status === 'saved'` (the CAS precondition), no longer defaulting a missing\nstatus to true. ExtensionStatusUpdateResult becomes a discriminated union\n(`ok:true` requires applicationId + status:'applied'; `ok:false` requires\nerror) mirrored in the zod schema and the extension's hand-written guard;\nthe Rust status_update.rs replies already satisfied the union.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: roll back status transition when event insert fails\n\nappend_event_conn now returns AppResult<()> and every call site\npropagates with `?`, so a failed status-event insert rolls back the\nwhole transaction instead of committing an orphan status flip.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-14T09:58:22+02:00",
          "tree_id": "33c34e90fa3c72c9805ab61632937f581e7123c7",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/4580e112e58075ad5614654024d0bb18aa343848"
        },
        "date": 1784016453897,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2224767,
            "range": "± 31687",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2737129,
            "range": "± 105296",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 298332,
            "range": "± 4545",
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
          "id": "27a5d732ae9625a6b50dce537d90005ff4db2ecc",
          "message": "feat: prefer extension job-root hint in generic page parsing (#633)\n\n* feat: prefer extension job-root hint in generic page parsing\n\nScan-mode imports stamp data-ajh-job-root on the likely job node; the\ngeneric-fallback parser now merges that subtree per field (title and\ndescription each override only when non-empty) over the whole-document\nheuristics, with script/style stripped locally and the no-hint path\npinned byte-identical by test. Fewer partial imports on cluttered pages.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: scope hint fallback and preserve section headings in job root parsing\n\nSkip the whole-document main_content_text last resort once the extension\nhint supplied a real title (thin-hint decoy risk), and strip only the\ntitle h1 (not every h1) from the hinted subtree's description source so\nlegitimate section headings survive. Adds precedence/regression pin tests.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* perf: skip job root reparse when no hint attribute present\n\nAdd an early substring check before the full-document reparse, skipping the no-hint path (every server-fetch resolve call).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-14T11:30:31+02:00",
          "tree_id": "25d9ad9143435dbd3c30fdffa013584ee8bce2f6",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/27a5d732ae9625a6b50dce537d90005ff4db2ecc"
        },
        "date": 1784021953018,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2115739,
            "range": "± 21638",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2512577,
            "range": "± 20314",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 282082,
            "range": "± 8091",
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
          "id": "d267ad87bd65cd7099697025d3c3f32a138d20fd",
          "message": "feat: fill portfolio and custom link fields from contact profile extra links (#634)\n\n* feat: fill portfolio and custom link fields from contact profile extra links\n\nThe autofill profile now carries the contact profile's extra links\n(cleaned desktop-side: http(s) allowlist, trimmed, capped at ten) and\nthe matcher fills link-labeled fields via conservative whole-word token\nmatching. Generic labels never match, multi-match skips as ambiguous,\nonly text/url inputs qualify, and the named-key fallthrough is scoped\nto the website key alone. Rides the existing autofill opt-in.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: token normalize generic link label denylist\n\nThe GENERIC_LINK_LABELS check compared an extra link's normalized-but-not-\ntokenized label against the raw denylist, so punctuation/hyphen variants\n(\"Website!\", \"Web-Site\") bypassed it while still token-matching a bare\nmatching field. Compare tokenized-vs-tokenized instead.\n\nAlso adds coverage for one link filling two same-labelled fields, label-side\ndiacritic symmetry, and suppresses the \"no matchable fields\" overlay line\nwhen fields were skipped as ambiguous instead (the skipped-note already\nexplains the outcome).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: sort denylist tokens and drop invalid link entries gracefully\n\nOrder-insensitive generic-link denylist comparison (sort tokens on both\nsides so e.g. \"Site Web\" can't bypass the denylisted \"web site\"), and\nfilter out malformed extraLinks entries instead of rejecting the whole\nprofile payload on one bad entry.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-14T13:01:51+02:00",
          "tree_id": "bdb91108d23b17150744e378490bdd12ca049ddf",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/d267ad87bd65cd7099697025d3c3f32a138d20fd"
        },
        "date": 1784027436097,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2117890,
            "range": "± 57776",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2534310,
            "range": "± 21795",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 289650,
            "range": "± 4936",
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
          "id": "c25c2b813ab2fa78badcda3562b236c5c74e7d2a",
          "message": "feat: capture filled application answers from the extension (#636)\n\n* feat: capture filled application answers from the extension\n\nNew answers.save verb: an explicit popup gesture collects filled,\nvisible, labeled form fields (identity fields and sensitive signals\nexcluded, select placeholders ignored) and appends them to the matched\napplication via the new merge_answers store method — single\ntransaction, normalized-question dedup, existing answers always win,\nper-field and per-application caps at the store boundary. Gated on the\nautofill opt-in; injected capture bundle stays a classic script with a\npackaging-time import-free assertion.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: merge answers by question on upsert instead of replacing\n\nupsert_internal's meta-merge path replaced Application.answers wholesale\nwhenever meta.answers was non-empty, so ai_generations_save silently wiped\nout any answers the extension's separate answers.save capture had appended\nonto the same application. Merge by normalized question instead: incoming\ntext wins for matching questions (needed for in-app answer edits), and\nexisting answers for untouched questions are preserved.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: surface skipped answer count and gate capture on pairing\n\nPopup now shows the desktop's skipped/dedup count on the answers-save\nconfirmation, with a distinct \"already recorded\" message when nothing\nnew was saved. Background now short-circuits on a missing pairing\ntoken before injecting the page-answer collector, mirroring the fill\nflow's not-paired gate.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: cap answers on creation and exclude autocomplete identity fields\n\nRoute upsert_internal's new-row branch through merge_answers_by_question\n(against an empty existing list) so MAX_TOTAL_ANSWERS and dedup apply on\napplication creation, not just merge. Extend isCapturable to also consult\nan input's autocomplete token via the shared Tier-1 mapping so a field\nautofill would treat as identity (e.g. autocomplete=\"name\") is excluded\nfrom answers capture even under a non-identity-looking label.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: swap capped answer replacements and harden classic script guard\n\nSame-question answer replacements in merge_answers_by_question now always\napply as a swap regardless of MAX_TOTAL_ANSWERS, so a legacy over-cap row\nno longer silently drops the replaced question; only brand-new questions\nare still subject to the cap. The extension packaging guard now strips\nstrings/comments and scans for import/export as tokens anywhere in the\nfile, instead of a line-anchored regex, so minified mid-line ES module\nsyntax can no longer slip past the classic-script assertion.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: look up existing application inside the upsert transaction\n\nupsert_internal released its lookup lock before re-acquiring it to write,\nletting a concurrent merge_answers commit land in the gap and be silently\noverwritten by the upsert's stale pre-gap snapshot.\n\nAlso extend the extension's AMBIGUOUS denylist with national-id, driver's\nlicense, bank/IBAN, and visa-status tokens.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-14T17:20:27+02:00",
          "tree_id": "b7b8c5f03f52650f79b4b31bcb213b2528fccd96",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c25c2b813ab2fa78badcda3562b236c5c74e7d2a"
        },
        "date": 1784042953636,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2140867,
            "range": "± 20898",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2534100,
            "range": "± 22803",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 289801,
            "range": "± 11826",
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
          "id": "d888929429b677e7d25c06ecdc26aabfec883fc7",
          "message": "feat: suggest saved answers for application form questions (#637)\n\n* feat: suggest saved answers for application form questions\n\nNew answers.suggest verb: the popup scans empty labeled fields and the\ndesktop matches them against all stored application answers with a pure\nlocal token-jaccard matcher (punctuation-stripping tokenizer, threshold\n0.4, one suggestion per question, capped). Suggestions render with copy\nand a fail-safe single-field fill; salary-like and duplicate-labeled\nquestions are copy-only. Gated on the autofill opt-in; the settings\ndisclosure now spells out the answers flow in both locales.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: harden salary detection and fill correlation against page variance\n\nRe-tokenize on non-alphanumeric boundaries before the salary-keyword\nsubstring check so hyphen/slash questions (\"Day-rate\", \"day/rate\") are\nno longer missed and wrongly offered a Fill button. Thread the\nscan-time same-question occurrence count through the answer-fill\nrequest/message/injected-arg chain so the fill-time re-scan refuses\nwhen a same-labelled field was inserted/removed since the scan,\ninstead of silently filling whatever now sits at that index. Add a\nnear-miss (0.375 < 0.4) negative regression pair for the suggest\nmatcher.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: match salary tokens exactly and pretokenize suggestion candidates\n\nSingle-word salary keywords now require a whole-token match instead of a\nsubstring one (fixes \"paid\" false-positiving inside \"unpaid\"); candidate and\nquestion tokenization is cached once instead of redone per jaccard pair.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: surface source question and extend salary rule to matched answers\n\nCloses the stopword footgun where an unrelated question (e.g. \"What is\nyour current location?\") can still cross the matcher's threshold on\nfiller words shared with a stored question (e.g. \"What is your current\nsalary?\"). Rather than stopword-filter the tokenizer or retune\nMIN_SCORE (both risk breaking the short-paraphrase matches it was tuned\nagainst), the popup now always shows the matched candidate's original\nquestion (\"answered as: ...\") so a cross-question match is visually\nself-evident, and the salary Copy-only guard checks BOTH the scanned\ninput question and the matched candidate's own question.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-14T20:31:16+02:00",
          "tree_id": "c1614c010084440310d12953b3fda5744beb97db",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/d888929429b677e7d25c06ecdc26aabfec883fc7"
        },
        "date": 1784055038508,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2243374,
            "range": "± 41123",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2687158,
            "range": "± 46550",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 251473,
            "range": "± 1642",
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
          "id": "c4f4b2c93ea19da2a7358bedbefdd59c2bfdb91b",
          "message": "test: pin xing and stepstone list urls as already canonical (#639)\n\n* test: pin xing and stepstone list urls as already canonical\n\nLive verification (public sessions) found both boards navigate to\npath-canonical urls at selection time, so no canonicalizer arms are\nneeded; regression tests guard the captured shapes and the tracking\nquery drop. Glassdoor stays open (cloudflare-blocked site-wide) and\nstepstone keeps a residual todo for its unprobed login-gated inline\nview, since this resolver's real caller is the authenticated import.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* test: carry captured tracking params in canonical url guards\n\nXing/StepStone detail-URL guards now carry the actually-captured\ntracking query params (ijt/rltr), matching the doc comment above them.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-14T21:23:53+02:00",
          "tree_id": "915175fa31fe291cfff28a33f43e3b9d263bfe5e",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/c4f4b2c93ea19da2a7358bedbefdd59c2bfdb91b"
        },
        "date": 1784057579949,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2143167,
            "range": "± 48912",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2580419,
            "range": "± 40964",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 303289,
            "range": "± 9153",
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
          "id": "abf8f2bdbe5b161ceb35530319107922fe25e842",
          "message": "feat: source the settings changelog from the bundled changelog file (#640)\n\n* feat: source the settings changelog from the bundled changelog file\n\nupdater_changelog previously fetched api.github.com/.../releases on every\nSettings changelog view. It now parses the repo's own CHANGELOG.md, bundled\ninto the binary at compile time via include_str! (Cargo tracks it for\nrebuilds like any other source dependency, no build.rs needed).\n\nThe changelog works fully offline now and removes a per-release GitHub API\ncall the app was making beyond the single on-launch version check that\ndocs/adr/0005-network-egress-privacy-boundary.md already accounts for.\n\nRelease-ordering check: the release job's semantic-release run commits\nCHANGELOG.md and tags vX.Y.Z in one step (.releaserc.json); the separate\nbuild-installers job later checks out that exact tag (.github/workflows/release.yml),\nso the shipped binary's bundled changelog always includes its own version's\nentry with no lag.\n\nThe IPC response shape (ChangelogResult/ChangelogRelease) is unchanged, so\nthe renderer (update-section, useChangelog) needed no changes.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: render changelog dates as local calendar dates\n\nDate-only publishedAt parsed as UTC midnight, showing the previous day west of UTC.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-14T22:26:49+02:00",
          "tree_id": "5e923fd0b1b8aee6b0b99e8417a386e0b741b6c8",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/abf8f2bdbe5b161ceb35530319107922fe25e842"
        },
        "date": 1784061350405,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2139936,
            "range": "± 22016",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2590873,
            "range": "± 76866",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287621,
            "range": "± 5254",
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
          "id": "bd87ac06333cfb34e49fca2735ba9b5ec352cc33",
          "message": "feat: add match.live check fit scoring and import match scores (#641)\n\n* feat: add match.live check fit scoring and import match scores\n\nThe last reserved verb: an explicit popup check-fit click scores the\ncaptured page keyword-only against the default resume via a purpose\nbuilt score entry that structurally cannot embed or translate (cli\nagent providers egress despite the local label), gated on the autofill\nopt-in since gap keywords form a resume-membership oracle, throttled\nper connection, and sharing the import path's normalized cache key.\nImports now carry a best-effort match score bounded to three seconds.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: show import match score and split the import flow module\n\n- popup: render import.result matchScore as a \"— NN% fit.\" suffix on the\n  success/status-unchanged lines (was dead-ends client-side)\n- align extension-protocol.ts's stale matchScore docblock with the\n  constants-file doc (best-effort keyword-only, omitted on failure)\n- match_live: split score_import_posting_bounded's logging so a genuine\n  timeout warns while an ordinary no-resume/no-text/scoring-failure None\n  only debugs; add a #fragment cache-key parity variant\n- popup: add a doCheckFit test covering the per-connection throttle reply\n- extension_bridge: move the import.request flow (ImportOk/result_reply/\n  persist_import_application/usable/handle_import) into a new\n  import_flow.rs sibling module so mod.rs clears the R8 LOC cap\n  (1399 -> 1121), behavior-identical relocation\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: bound live scoring and make the match throttle survive reconnects\n\nWrap resolve_match_live's keyword-only scoring in the shared timed()\nhelper (3s cap, same as the import-time score) so a hung/slow scorer\ncan't block the connection's serial frame loop indefinitely, folding a\ngenuine timeout and an unexpected internal error shape onto the same\nfixed sentinel. Move MatchLiveThrottle off the per-connection stack\nonto BridgeState (behind a Mutex, shared across connections) so a\nloopback reconnect no longer refreshes the burst allowance. Reorder\nresolve_match_live's validation so the autofill opt-in gate runs\nbefore url/html emptiness checks, matching sibling verbs. Document the\nthreat model for the ungated import-time match score.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-15T00:17:32+02:00",
          "tree_id": "851eebf12379f4728e3bb7cd117969a68d92378a",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/bd87ac06333cfb34e49fca2735ba9b5ec352cc33"
        },
        "date": 1784068013092,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2139416,
            "range": "± 29842",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2548427,
            "range": "± 22600",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 283763,
            "range": "± 4440",
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
          "id": "d6d820f01f7c2f7dc82cdbdf95868e24c15ada92",
          "message": "feat: draft application answers from the extension behind a new opt-in (#643)\n\n* feat: draft application answers from the extension behind a new opt-in\n\nNew answer.assist verb, the bridge's first billable one: behind its own\ndefault-off opt-in (provider snapshot pinned at enable time and shown\nin settings), the desktop drafts a grounded paste-ready answer for a\npage question — salary questions ride the shipped no-fabrication\nmachinery, others get a compact rust port of the answer prompt with\nevery untrusted block fenced and neutralized. Optional web research\nrides the existing daily limiter through the tested charge ordering;\nonly fixed sentinels ever reach the wire. Copy-only in the popup.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: keep the ai assist opt-in toggle reversible and harden fence tests\n\nOnly gate the Switch's disabled state on the ON direction, so a live\nprovider-config change can never trap the billable opt-in in the ON\nstate. Adds forged-opening-tag and close-then-reopen fence tests,\na11y label on the assist textarea, debug-logs the provider resolution\ncause on answer-assist failure, and caches the fence-tag regex per\nfixed tag instead of recompiling it on every fenced() call.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: neutralize cross tag fence tokens and tidy ai assist tests\n\nNeutralize every known fence tag (not just the wrapping one) inside each\nfenced prompt block, since answer_assist composes six blocks in one\nmessage and an untrusted question could forge a sibling tag like\njob_posting; documented divergence from @ajh/prompts' same-tag-only\nneutralizeFenceTag. Also: base_url round-trip test for the ai-assist\nopt-in snapshot, omit-not-null serialization for its provider/model\nreply fields, a copy-assist popup test, and ExtensionBridgeSection test\nhygiene (shared stub reset via beforeEach, extended renderSection helper,\nnoProvider description assertion).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-15T03:34:36+02:00",
          "tree_id": "c17bbbc68bbdc28d2c27110426c74fcaa02df7d2",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/d6d820f01f7c2f7dc82cdbdf95868e24c15ada92"
        },
        "date": 1784079846320,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2163954,
            "range": "± 57454",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2669830,
            "range": "± 50095",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 303201,
            "range": "± 21346",
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
          "id": "9985ff6136dd01afa52417b6b4a97fa376e19369",
          "message": "feat: stream extension answer drafts over the bridge (#646)\n\n* feat: stream extension answer drafts over the bridge\n\nAdds an additive streaming frame family (assist.chunk/done/cancel) keyed\non the existing reqId and upgrades answer.assist draft mode from one-shot\nto live streaming, reusing the in-app provider stream fenced by a server\nminted job id and a per-connection sink so no stream can cross\nconnections. The per-connection read loop no longer blocks on an\nin-flight stream, so a cancel is actually reachable; drafts are capped\nlive, the client timeout resets on activity and cancels on stall, and\nthe cancel registry is per-connection. Rewrite mode is a follow-up.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: cancel orphaned answer streams and record partial spend\n\nCloses the ensemble findings on the streaming transport: a dead sink or\na dropped connection now cancels the in-flight generation immediately\n(cancel_all on disconnect, ForwardOutcome::SinkGone on a gone writer)\ninstead of billing to completion for no listener; provider errors fail\nthe job instead of leaving it stuck running; hitting the draft cap now\nrecords the partial provider usage in the spend store; and the compose\ninternals moved into stream.rs for R8 headroom. Cancel is now\ntrait-tested, and a check-before-remove bug in the registry was fixed.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: preserve early-cancel marker across disconnect and make cancel atomic\n\ncancel_all's drain-and-reinsert dropped an already-CancelledEarly entry\ninstead of reinserting it, so a cancel-then-disconnect during the\npre-compose window let a later register() start a full billable\ngeneration for a request the user had already cancelled. cancel() also\nsplit its Running/Pending decision across two separate lock\nacquisitions (a TOCTOU a concurrent register() could win); both are now\none exhaustive match under a single lock. Also fixes the module doc's\nstale \"Three ways\" count and adds ai_provider/stream.rs comments\ndocumenting the cancel-branch record_usage provider caveat and its\nmirror test.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: make the native messaging relay reads cancellation safe\n\nThe duplex select! loop raced a stdin read_exact branch against a ws\nread branch; read_exact is not cancellation-safe, so select! dropping\nthe losing branch mid-read discarded already-consumed stdin bytes and\ndesynced the length-prefixed frame stream. Split the relay into two\nindependent tasks (stdin->ws, ws->stdout), each owning its reader and\nwriter half exclusively, so no read_exact future is ever raced or\ndropped mid-frame; relay() only selects on the tasks' JoinHandles,\nwhich is safe (dropping one detaches rather than aborts it).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: join the losing relay pump before shutdown so last frames flush\n\n`relay()` selected on the two pump tasks' JoinHandles and dropped the\nloser, but `Runtime::drop` in `run()` cancels still-incomplete spawned\ntasks rather than letting them finish — so a `pump_ws_to_stdout` task\nmid-write of the last `assist.chunk`/`assist.done` frame could be axed\nbefore the write/flush landed. Now the loser is `.await`ed with a\nbounded 200ms timeout before `write_ready(false)`, which also fixes the\n`write_ready` stdout race by sequencing it after the join. Renamed the\nfragmented-delivery test to reflect what it actually covers and\ndocumented why a genuine select!-cancellation test against\n`read_stdin_frame` isn't included (read_exact is inherently not\ncancel-safe; the architecture never races it in production).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: harden assist stream registry, spend accounting, and interruption ux\n\n- unregister the pre-compose registry entry on a rejected daily-budget\n  charge, closing a Pending-entry leak between registry.begin and\n  compose_draft_stream\n- reject a reused in-flight reqId in AssistStreamRegistry::begin instead of\n  silently orphaning the original job\n- record accumulated provider usage on a transport read error too, mirroring\n  the existing cancellation-branch spend accounting\n- render the interrupted state (not just the partial draft) when a live-push\n  answerAssistProgress update reports a later stream failure\n- settle a superseded answerAssist request's promise and drop its stale\n  chunk listener when a newer request starts, so it can no longer mutate the\n  new request's shared draft buffer\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix: guard answer-assist stream buffer against overlapping requests\n\nThe background service worker holds a single-slot assistBuffer for the\nstreaming answer.assist draft. MV3 popups are torn down on close, and the\nreattach path re-rendered an in-flight stream without re-disabling the\nbutton, so a second overlapping request could reset the shared buffer\nwhile the first was still streaming — the first run's late chunk and\nterminal writes then stomped the second's buffer, rendering a garbled or\nprematurely-\"done\" draft as if coherent.\n\nAdd a monotonic assistGeneration guard: each runAnswerAssist captures its\ngeneration and (a) early-bails after setup once a newer run has superseded\nit, before resetting the buffer or issuing the billable request, and\n(b) drops its onChunk and terminal writes when superseded mid-stream. The\npopup now reflects an in-flight reattached stream by disabling the button\nuntil terminal. Adds two overlapping-call regression tests (both proven to\nfail without the guard).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: reject a reused reqid until its cancelled-early marker is consumed\n\nAssistStreamRegistry::begin rejected only Pending/Running entries, so a\nreused reqId could overwrite a CancelledEarly marker with a fresh Pending.\nThat marker exists to be consumed by the original pre-compose run's later\nregister() call (which removes it and returns false, aborting before any\nbillable job starts); overwriting it made register() see Pending instead,\ninsert Running, and start a billable generation for a request the user had\nalready cancelled.\n\nbegin now rejects any occupied entry (contains_key), mirroring the existing\ncancel_all hardening that re-inserts rather than drops CancelledEarly. The\nmarker is always cleared within the original run's lifecycle (register\nconsumes it, charge failure unregisters, or cancel_all on disconnect), so a\nreqId is never permanently locked out; a well-behaved client uses a fresh\nuuid anyway.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-15T10:27:18+02:00",
          "tree_id": "dda0a6590012fdea0c928551aa74c43270f24227",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9985ff6136dd01afa52417b6b4a97fa376e19369"
        },
        "date": 1784105253046,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2159718,
            "range": "± 84895",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2606729,
            "range": "± 63745",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287897,
            "range": "± 5212",
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
          "id": "1adfc4286e42f90cfa810c50e226f12fcd95fcc3",
          "message": "fix: harden the extension bridge answer-assist streaming transport (#648)\n\n* fix: harden the extension bridge answer-assist streaming transport\n\nThree concurrency/lifecycle fixes on the billable answer.assist stream:\n\n- run_writer now wraps each write in a 25s timeout so a stalled-but-open\n  peer (which parked the unbounded outbound channel forever) becomes a\n  dropped receiver, tripping the existing SinkGone -> job_cancel path;\n  bounds channel memory and wasted spend. Kept the unbounded channel (a\n  bounded one would re-couple the read loop or drop reply frames).\n- hoist registry.begin into the synchronous read-loop dispatch (before\n  tokio::spawn) so an assist.cancel racing the spawn always finds the\n  Pending marker instead of being silently dropped and billing a\n  cancelled request; unregister the Pending entry on any early-gate Err\n  so hoisting begin ahead of the gates does not leak registry entries.\n- reorder job_start before register in compose_draft_stream, cancelling\n  the just-started job if a cancel raced ahead, so a cancel in the gap\n  always targets a real, cancellable job (no orphaned Running generation).\n\nThe 0/0 spend-row recording on cancel/error for end-of-stream-only\nproviders is intentional convention (matches cli_agent and finish: \"a\ncall happened at $0\") and is left unchanged.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: extract the assist stream registry into its own module\n\nPure behavior-preserving move to keep stream.rs under the R8 hard LOC cap\nafter the hardening additions (stream.rs 1483 -> 896; new assist_registry.rs\n643). The reqId state machine (StreamEntry, AssistStreamRegistry, the\nJobCanceller/JobStarter traits, start_and_register) and its tests move out;\na re-export keeps every existing super::stream::AssistStreamRegistry path\nunchanged. Test count unchanged (259).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: propagate writer-timeout teardown and generation-scope registry cleanup\n\nAddress the two review findings on the hardening PR:\n\n- The detached run_writer task's write-stall/error exit went unnoticed by\n  handle_connection's read loop, so cancel_all/set_connected(false) ran only\n  after the next inbound frame — a stalled/idle peer lingered \"connected\".\n  The read loop now races reader.next() against the kept writer JoinHandle\n  (via a generic, unit-testable next_step seam); a WriterEnded result breaks\n  the loop straight into the existing teardown.\n- reqId-keyed unregister across multiple sites let a request's late cleanup\n  clobber a reused-reqId successor's fresh entry (stranding a billable job\n  as uncancellable). Consolidated cleanup to a single owner AND made removal\n  generation-scoped: AssistStreamRegistry mints a strictly-monotonic gen at\n  begin, carries it on every StreamEntry, and unregister_gen removes only a\n  matching-gen entry — so a stale request can never remove a successor's\n  entry, while cancel/cancel_all stay reqId-targeted (current holder).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-15T16:19:37+02:00",
          "tree_id": "092268f244e9b4512739ace7c2d65fb30ee7afbd",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/1adfc4286e42f90cfa810c50e226f12fcd95fcc3"
        },
        "date": 1784125772948,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2244340,
            "range": "± 53699",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2647340,
            "range": "± 18705",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287167,
            "range": "± 11172",
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
          "id": "2ec7a8cfcbe3851283bcd3f2d1b157c94afd2c74",
          "message": "fix: bring hmac keyinit trait into scope for the 0.13 api (#674)\n\nhmac 0.13 (#672) moved `Hmac::new_from_slice` from an inherent method to\nthe `KeyInit` trait, so `main` stopped compiling in the extension_bridge\npairing handshake. Bring the trait into scope — one import, zero logic\nchange (same MAC key/message/constant-time verify; KAT + tamper/reject\ntests pass unchanged). sha2 0.11 (#669) and tokio-tungstenite 0.30 (#670)\nneeded no changes — their use sites were already API-compatible.\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-15T20:29:49+02:00",
          "tree_id": "f629211681e22a344a7c39542378c5f635786d70",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/2ec7a8cfcbe3851283bcd3f2d1b157c94afd2c74"
        },
        "date": 1784141401391,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2148024,
            "range": "± 87238",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2561153,
            "range": "± 43679",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 286497,
            "range": "± 6262",
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
          "id": "9bf24aa0e11dca454dcbe8bf3715d289a44fc349",
          "message": "fix: restore rewrite-mode review fixes lost in #649 (#675)\n\n* fix: address rewrite-mode review findings\n\n- Refuse a field replace when its value changed after pick (data integrity):\n  thread the field's expected current value through answerReplace end-to-end;\n  replaceFilledField returns CHANGED_SINCE_PICK and never overwrites a manual\n  edit made between pick and Accept/Restore. The expected baseline updates to\n  whatever a successful Accept/Restore wrote.\n- Fix the rewrite picker reset: Number('') is 0, so resetting the picker to\n  its placeholder silently re-picked index 0 instead of clearing the target;\n  guard the empty value before coercion.\n- Validate rewrite required fields BEFORE acquiring the ai_research limiter,\n  via a pure validate_rewrite_fields (which structurally can't touch the\n  limiter), so malformed rewrite frames can't burn rate-limiter slots.\n- Extract assist_prompt_for_mode (pure mode->prompt) and test it directly;\n  the remaining resolve_answer_assist end-to-end slice has no mock-app harness\n  in this crate (pre-existing, same as draft mode).\n\nKeeps the rewrite prompt policy in extension_bridge to mirror the shipped\ndraft path (ANSWER_ASSIST_SYSTEM + build_user_message live there too) —\nconfirmed a CodeRabbit false positive by the independent review.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: capture rewrite target before await to prevent mid-flight repick clobber\n\nsendRewriteReplace re-read the module-level rewriteTarget after the await to\nstamp expectedValue; a re-pick to a different field mid-flight would corrupt\nthe new target's baseline. Capture the target by reference before the send so\nthe in-flight request only ever touches the object it captured.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-15T20:55:09+02:00",
          "tree_id": "f664653d52db8502373b47995508f57dad47405f",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9bf24aa0e11dca454dcbe8bf3715d289a44fc349"
        },
        "date": 1784142299517,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2171479,
            "range": "± 34243",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2571794,
            "range": "± 67562",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 292767,
            "range": "± 15215",
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
          "id": "b9230da4ca21f546926c14c4e06b01f549cbd35c",
          "message": "fix: own the active ai provider config in the backend so the renderer can't set base_url (#682)\n\n* fix: own the active ai provider config in the backend so the renderer can't set base_url\n\nThe renderer's Zustand preferences were the source of truth for the active AI\nprovider/model/base_url and injected them into every generation call, which the\nbackend trusted. An XSS'd renderer could set provider=openai + base_url=attacker\nand exfiltrate the stored key + full prompt on every call (unguarded egress\npath); the extension bridge was worse, persisting the attacker base_url to disk.\n\nIntroduce a backend-owned AiConfigStore (SQLite, mirrors EmbeddingConfig) as the\nsingle source of truth for { activeProvider, providers:{model,baseUrl} }, wired\ninto the GDPR reset registry + backup bundle. Generation, research, the extension\nbridge, and autopilot now resolve provider/model/base_url server-side via\nCompleter::from_active; provider + baseUrl are removed from AiGenerateRequest\n(compile-time lock). base_url is validated on write (http(s) only, cloud-metadata\naddress blocked) — provenance, not IP-filtering, so localhost/LAN gateways still\nwork. The renderer flips to a backend-backed useActiveConfig query with a boot\nprefetch and a one-time post-hydration seed from existing prefs; effort and\nmodelLimits stay renderer-side. The three settings-time inspection commands keep\nbase_url (not the generation surface).\n\nBehavior change: a scheduled autopilot run now follows the currently-active\nprovider instead of the one pinned when the schedule was created.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: scope base_url ownership docs, drop inert native base_url, guard cold-boot caption\n\nAddress the #16 review findings (all advisory, none blocking):\n\n- Scope the AiConfigStore module doc + net/ssrf doc: base_url ownership is closed\n  for the flipped generation commands (ai_generate, pipeline, research/salary,\n  bridge, autopilot); the agent_run path is a tracked follow-up (NEXT_ISSUES #5).\n  Correct the metadata-block claim to \"any IPv4 notation\" (IPv6 forms are not\n  covered; the block is defense-in-depth, not the boundary).\n- Persist base_url only for openai-compatible in both validate_settings and the\n  seed/import scrub path; native providers store NULL so an inert base_url can't\n  reach record_usage's cost gate (+ tests).\n- Guard the autopilot StepAction and StepFineTune captions on the useActiveConfig\n  isPending window so an already-configured user never sees a \"no provider\" flash\n  on cold boot (+ test).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: close out task #16 with the base_url provenance adr\n\nAdd ADR-0012 (backend-owned AiConfigStore, provenance not IP-filtering,\nscoped to the flipped commands with agent_run tracked as open). Sync the\nAI-provider and autopilot behavior-change notes into automation-domain.md,\nretire the answer-assist provider-snapshot description in extension-domain.md\nand ADR-0011, and add two LOW follow-ups to NEXT_ISSUES.\n\n* fix: handle the error-union in provider-config mutations and cover from_active + boot\n\nAddress the #682 claude review:\n\n- HIGH: useSetActiveProvider/useSetProviderSettings/useConfigureActiveProvider\n  now narrow the { error } union and throw so a rejected write surfaces via\n  onError instead of a false success; useConfigureActiveProvider stops before\n  setActiveProvider when setProviderSettings errors. CloudProviderConfig's\n  base_url save gains error feedback (+ i18n key). So a base_url rejected by the\n  new SSRF/provenance guard is now shown, not swallowed.\n- Extract an AppHandle-free from_config/resolve_parts seam from Completer so the\n  fail-closed base_url re-validate is unit-tested (tampered metadata/scheme ->\n  reject, never fall back to the default endpoint).\n- Add the missing AiConfigBoot seed-logic test (once-only guard, pre-hydration\n  skip, fresh-install skip, invalidation).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-16T02:07:40+02:00",
          "tree_id": "87e90b76ab8b1e5158cfb3997eff38f2e3d66d82",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b9230da4ca21f546926c14c4e06b01f549cbd35c"
        },
        "date": 1784161656938,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2143453,
            "range": "± 66839",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2538073,
            "range": "± 25107",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287417,
            "range": "± 3930",
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
          "id": "9eb2e75ade9434797b405e5f75a4f4ad07d5d070",
          "message": "fix: resolve the agent_run provider from the backend store, not the renderer request (#685)\n\n* fix: resolve the agent_run provider from the backend store, not the renderer request\n\nThe agent \"prep this application\" loop still trusted req.provider/model/base_url\nand threaded base_url into ToolContext, so an XSS'd renderer could point an\nopenai-compatible base_url at an attacker and exfiltrate the key + resume/JD per\nturn — the same class task #16 closed for ai_generate. It was also split-brain:\nresearch_company already resolved from the store while complete_trusted used\nctx.base_url, so one run could hit two endpoints.\n\nagent_run now resolves via Completer::from_active; ToolContext carries only\njob_id; complete_trusted resolves from the store too (unifying every agent tool +\nthe agent's own turns on one store-configured endpoint). provider/model/base_url\nare removed from AgentRunRequest (Rust struct + Zod schema) so the renderer can no\nlonger supply them — the compile-time lock. Renderer sends only { resumeId, jobId }.\n\nCloses the last renderer-settable base_url path (NEXT_ISSUES #5).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: remove the dead request-driven provider-resolve constructor\n\nAfter #16 + #25 flipped every generation path onto `Completer::from_active`, the\nrequest-driven `Completer::resolve(provider, model, base_url)` had zero callers —\nand it's the exact \"accept a renderer-supplied base_url\" footgun the whole exfil\nclass removed. Delete it (keeping resolve_parts/from_config/from_active) so\nstore-resolution is the ONLY way to bind egress routing; a future command can no\nlonger re-introduce the class by wiring renderer input into it. Reword the doc\nlinks that pointed at it. No behavior change.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: close adr-0012 scope now that agent_run is store-routed\n\nTask #25 flipped agent_run onto Completer::from_active and deleted the\nold request-driven Completer::resolve constructor, so it is no longer\na tracked exception. Update adr-0012's scope/consequences/references to\nsay the base_url provenance closure covers every generation path, mark\nNEXT_ISSUES #5 closed, and correct the automation-domain knowledge note\nthat still described agent_run as unflipped.\n\nReviewed by tauri-security-reviewer + rust-backend-architect, no\nHIGH/CRITICAL findings.\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-16T04:17:45+02:00",
          "tree_id": "dd0ca7f647c09e908cc33f40cf47f94fe2175779",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/9eb2e75ade9434797b405e5f75a4f4ad07d5d070"
        },
        "date": 1784168817380,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2137353,
            "range": "± 30437",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2517817,
            "range": "± 66381",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 284142,
            "range": "± 15020",
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
          "id": "798c82fefe5ecaddabb54f5d222eb7052e078f16",
          "message": "feat: auto-track sent applications on a detected form submit (opt-in) (#687)\n\n* feat: auto-track sent applications on a detected form submit (opt-in)\n\nAfter the user invokes the extension on an application page (an existing\ninjection gesture) and enables the new default-OFF \"Auto-track sent applications\"\nopt-in, a pure-DOM submit-watch arms a capture-phase submit + apply-button\nlistener. On a detected submit it re-checks the opt-in, runs applied.check, and\nauto status.update {to:'applied', auto:true} for a tracked saved job (silent\nno-op if already applied; an action-badge nudge to import when untracked). It\nnever blocks/alters the submit and never auto-creates.\n\nThe auto write is desktop-enforced: status.update refuses auto:true when the\nopt-in is off (the manual popup mark-as-applied is unflagged and ungated as\nbefore). No new permissions/manifest changes; Firefox data_collection stays none.\n\nLayer A of the auto-track feature; full-page-nav submits are best-effort (Layer C\nemail parsing is the complement).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: cover the auto-track background wiring, distinguish auto vs manual, harden sender check\n\nCloses the review findings (all advisory):\n- add background.ts integration tests for the submitDetected route, arm-after-\n  gesture, and badge-clear (the wiring the pure-fn tests didn't exercise)\n- pin the duplicated SUBMIT_DETECTED_MSG literal with a parity test so the\n  background/lib copies can't silently diverge\n- record \"auto-tracked via extension\" vs \"via extension\" in the status history\n  so an auto write is distinguishable later, not only in the notification body\n- assert sender.id === runtime.id before acting on submitDetected (belt-and-\n  braces; already mitigated by the absent externally_connectable)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: amend adr-0009 and extension-domain for auto-track layer a\n\nRecords the auto-track opt-in (task #22) as a distinct sanctioned\nauto-action consent class in adr-0009: the server-side enforcement\nboundary in handle_status_update, the honest residual risk, and the\nnew autotrack.check/autotrack.result verbs plus status.update's auto\nflag. extension-domain.md gets the verb-table entries and a new\ngesture-armed submit-watch section documenting detection, routing,\ndecision branches, and honest limits.\n\n* refactor: extract auto-track opt-in machinery into an autotrack submodule for r8\n\nThe auto-track additions pushed extension_bridge/mod.rs to 1425 LOC, over the\n1400 hard cap. Move the opt-in file load/persist, the enabled accessors, and the\nautotrack result reply into a sibling autotrack.rs (mirrors status_update.rs).\nmod.rs is now 1372. No behavior change.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-16T06:40:45+02:00",
          "tree_id": "11fca9a9677900043faa687330466c9f2f84bbef",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/798c82fefe5ecaddabb54f5d222eb7052e078f16"
        },
        "date": 1784177379383,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2131496,
            "range": "± 56582",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2529331,
            "range": "± 30433",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 284161,
            "range": "± 3635",
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
          "id": "7de160e10f457b19a83d89575ac61733ee985299",
          "message": "feat: connect gmail for email-confirmation watching (auto-track layer c, foundation) (#689)\n\n* feat: add the email-watch foundation (store, imap connect, ipc)\n\nBackend half of auto-track Layer C (task #23): EmailWatchStore (email_watch.db,\naccount singleton + seen dedupe, Resettable, excluded from backups), a thin\nIMAP validate_connection seam (native-tls; imap's rustls-tls bridge pins a\nrustls-webpki with 4 live RUSTSEC advisories and no update path), the\nemail-imap keychain slot, and the 5 email_watch_* commands with contracts,\ntauri-client namespace, and mock parity. No poller/parser yet — that is PR B.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* feat: add the email-watch settings section and service hooks\n\nFrontend half of PR A (task #23): use-email-watch hooks (status query + 4\nmutations seeding the status cache), the accounts EmailWatchSection\n(connect form, connected view with enable switch and check-now, honest\nconsent disclosure), settings-search entry, and en/de translations.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: harden the email-watch foundation per critic review + add adr-0013\n\nReview round (security PASS, rust-arch PASS, frontend 1 HIGH — all resolved):\nexact-pin the alpha imap dep, try_state in the mutating commands instead of a\npanicking state(), fresh-account semantics on an address switch (clears the\nuid watermark and seen table, +test), sentinel the spawn_blocking join error,\nlog imap error kinds only, keep text labels on the pending check-now and\ndisconnect buttons (a11y HIGH, +2 regression tests, en/de keys).\n\nDocs: ADR-0013 (imap-over-oauth economics, notify-dont-write, zero content\negress), the new imap egress class enumerated in README and SECURITY, and\nthree lessons persisted.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: guard email-watch writes against a racing disconnect and strip password paste-spaces\n\nCloses the /review ensemble's verified advisories: set_enabled/record_check\nnow no-op once the account row is cleared (WHERE id = 1 AND address IS NOT\nNULL, row-affected result + 3 interleaving tests), so a disconnect landing\nduring the multi-second imap round trip can never leave enabled=1 on an\nempty row; the gmail app password is stripped of ascii whitespace before\nvalidation (google displays it space-grouped and pastes keep the spaces);\nadr-0005 gains egress class 7 so adr-0013's citation resolves.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: address the verified coderabbit findings on the email-watch foundation\n\nSeven of eleven bot findings verified real against head and fixed: imap\nvalidation gets explicit connect/read/write timeouts via a manual socket path\n(the pinned crate's builder exposes none, so a black-holing server pinned a\nblocking worker forever); connect()/clear() multi-statement writes are now\ntransactional (a landed update + failed seen-delete could permanently skip the\nstale-mailbox cleanup); the app password clears from component state on a\nfailed connect too; adr-0005's stale six-classes count, adr-0013's overstated\nrevocation and read-only claims, and the readme/security sqlite-credentials\nconflation are corrected. The other four were verified already-fixed, false\npositives, or an advisory metric - in-thread replies document each.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-16T18:33:40+02:00",
          "tree_id": "a5f70f8188977487bfdeeb19f36744a7ae6b381a",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/7de160e10f457b19a83d89575ac61733ee985299"
        },
        "date": 1784220885752,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2212912,
            "range": "± 41236",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2596431,
            "range": "± 23527",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 297890,
            "range": "± 9966",
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
          "id": "b6281d6d84d9f36b24f4de514961799ac43232e0",
          "message": "fix: track bridge connections with a refcount and push status changes to settings (#693)\n\n* fix(shared): add a bridge live-connection-changed push event\n\nWire-protocol lockstep for the bridge's live-connection change push: a new\nEVENT_CHANNELS.extensionBridge.changed constant, its ExtensionBridgeChangedEvent\npayload type, and the matching onChanged subscription method on the\nExtensionBridgeContract, plus the generated Rust EXTENSION_BRIDGE_CHANGED\nconstant (pnpm gen:ipc).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix(bridge): count live connections instead of one shared flag\n\nBridgeState.connected was a single AtomicBool set true on every AuthOk and\nfalse on ANY socket close. With two browsers paired on the same token,\nwhichever socket closed last decided is_connected() for every other\nstill-open one — Chrome's MV3 service worker idling its socket closed the\nflag while Firefox was still connected, so the Settings pill went stale.\n\nReplace it with an AtomicUsize refcount: inc_connected on AuthOk,\ndec_connected on that same connection's teardown (gated by a per-connection\n`authenticated` flag so an unauthenticated close never decrements), and\nemit `EXTENSION_BRIDGE_CHANGED` only on the 0->1 / ->0 transitions. Saturating\ndecrement so an unmatched call can never wrap the count below zero.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix(desktop): subscribe the renderer to the bridge connection event\n\nWire the client, the app-global root layout, and a new\nuseExtensionBridgeEvents hook to the bridge's live-connection push:\ninvalidates the extension-bridge status query on a 0->1 / ->0 transition so\nthe Settings pill flips immediately on pair/unpair, instead of only on the\nexisting 30s poll (kept as a fallback for a missed/dropped event).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix(desktop): add a manual refresh button to the extension-bridge status\n\nBeside the connection pill in Settings, a RefreshButton that calls\nrefetch() on the status query — its label stays visible and only the icon\nspins while pending, matching this section's other pending-button pattern.\nAlso documents (en+de) that multiple browsers can pair with the same token\nand that Regenerate token disconnects all of them, not just the current one.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* refactor: extract the applied-check read path from the bridge module\n\nextension_bridge/mod.rs grew past the R8 hard LOC cap (tests/architecture.rs)\nonce the live-connection refcount landed. Move the self-contained\napplied.check machinery (AppliedCheckOk, resolve_applied_check,\napplied_result_reply, handle_applied_check) into a sibling applied_check.rs,\nmirroring the existing status_update.rs / autotrack.rs split. Pure move: no\nbehavior change, pub(super) visibility so import_tests.rs keeps direct\naccess, call sites updated to applied_check::handle_applied_check.\n\nmod.rs: 1419 -> 1316 LOC (cap is 1400).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-16T23:47:23+02:00",
          "tree_id": "4de6d3ccb74077e51b2ec7775ce9c558f1ca3d2d",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b6281d6d84d9f36b24f4de514961799ac43232e0"
        },
        "date": 1784239053154,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2125104,
            "range": "± 49147",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2525684,
            "range": "± 25692",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287334,
            "range": "± 13954",
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
          "id": "d2f6fb2040202e1e3598c559540e6bf03c5398de",
          "message": "feat: watch the inbox for application confirmations and notify to confirm (auto-track layer c) (#696)\n\n* feat: add the email-watch poller, parser, matcher, and scheduler\n\nThe watcher half of auto-track layer c (task #23 pr b): a separate l2\nscheduler mirroring the autopilot one (15-min ticks, failure backoff, 60s\ncheck-now rate limit), the imap read path (uid search above the watermark,\nheaders first, bodies only for fingerprint hits, all read-only), an en+de\nconfirmation-email parser (mail-parser, rfc2047/mime decode, sender-domain\nhints boost but never gate), and a token-jaccard company/title matcher\nagainst saved applications. New matches stamp the seen table first, then\npush a notification card routing to the matched application - never an\nautomatic write. Also treats a failed socket-timeout set as a connect error\n(the deferred pr-a review advisory).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ui: update the email-watch settings copy for the live watcher\n\nCheck now runs the real fetch-parse-match-notify pass and the watch switch\ngoverns the 15-minute automatic check, so the placeholder copy is rewritten\nto the true semantics (en+de); the 60s check-now rate limit surfaces as\nfriendly fixed copy instead of a raw error, with tests for both rejection\nshapes.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: guard concurrent email checks and broaden confirmation recall\n\nCritic round two (security PASS, match-quality PASS, rust-arch 1 HIGH): an\nin-flight RunGuard on the shared run_check seam closes both the check-now\nTOCTOU (gmail login bursts) and the stamp-before-notify concurrency gap;\neffective_last_uid is reused not duplicated; advance_last_uid enforces its\nmonotonic invariant in SQL; per-tick header/body/subject byte bounds; EXAMINE\nover SELECT for a read-only session; try_state for the credential store in the\nscheduler. Parser recall broadened per job-match review (thanks-for-your-\napplication and received-your-application EN shapes, dative ihrer bewerbung,\neingangsbestätigung, informal deine bewerbung), and/und boundary truncation\nfixed, plus 8 adversarial fixtures documenting the rejection-email and\nwrong-role precision limits that must gate any future auto-write.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* docs: add the email-watch domain doc and close out the layer c pointers\n\nNew knowledge doc for the watcher domain (read-only imap posture, watermark\nand dedupe contracts, en+de fingerprint philosophy, honest limits incl. the\nrejection-parity constraint that gates any future auto-write), the\nextension-domain layer c pointer updated, and four lessons persisted from\nthis build's review rounds.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: clear seen on mailbox renumber and guard tick writes against disconnect\n\nReview-ensemble round (two verified HIGHs plus advisories): a uidvalidity\nchange now wipes the seen table in the same transaction as the watermark\nreset, so reused uids from a renumbered mailbox can never silently swallow a\nreal confirmation; the three post-tick writes and the notify path re-check\nthe account still exists, closing the disconnect-mid-tick resurrection race\npr a solved for its own writes; the uid search is watermark-scoped when the\ngeneration is confirmed unchanged; a concurrent-run refusal no longer\ninflates the backoff counter; and the renderer rate-limit sentinel is pinned\nby a cross-language parity test. The email-watch domain doc is corrected to\nthe shipped behavior (native-tls, credential persistence, real seen schema,\nnotify-only wording).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* fix: pin the disconnect-mid-tick guard with a test and cap body fetches at the protocol level\n\nExtracts the account-still-connected check run_check_inner branches on into\na pure should_commit_outcomes fn (mirroring is_due/backoff_interval/\neffective_last_uid/classify_tick_outcome's own pattern) so the guard that\nsuppresses both post-tick writes and notify_match after a disconnect\nactually has a test pinning it, not just inline logic.\n\nAlso bounds fetch_bodies at the protocol level via a partial-octet\nBODY.PEEK[]<0.N> fetch item (confirmed supported by the imap crate's own\ndocumented grammar) instead of only truncating in-process after the whole\nmessage is already resident - a large/hostile mailbox no longer costs a\nmemory spike per candidate. The post-fetch cap in poller.rs stays as\ndefense-in-depth for a non-compliant server.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-17T03:21:09+02:00",
          "tree_id": "e6334d5373faccc2fabf2c5bbf61ce6ce02667f3",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/d2f6fb2040202e1e3598c559540e6bf03c5398de"
        },
        "date": 1784252350789,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1721403,
            "range": "± 13413",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2109663,
            "range": "± 34437",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 238407,
            "range": "± 7130",
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
          "id": "a69cace83b81daa5a87737f126ad69c8350df260",
          "message": "ci: credit external contributors with @mentions in release notes (#697)\n\n* ci: credit external contributors with @mentions in release notes (corrected)\n\nCORRECTED IMPLEMENTATION: Properly wrap preset transform, not replace it.\n\nConvert .releaserc.json → release.config.mjs (ESM with top-level await).\nResolve preset factory with top-level await to extract its transform function,\nthen wrap it with a composition that:\n1. Calls preset's transform first (preserves type→section bucketing, hidden\n   filtering, reference linkification, BREAKING CHANGE handling)\n2. Respects the preset's filtering (returns false/null for hidden commits)\n3. Augments the result with (@<login>) credit if applicable\n\nThis fixes the v10 empty-notes gotcha: directly setting writerOpts.transform\nREPLACES the preset transform entirely, losing all the preset's logic.\nBy wrapping, we preserve the preset's work while adding attribution.\n\nAdd comprehensive tests covering:\n- extractGitHubLogin unit tests (6 cases)\n- Config loads with ESM and top-level await\n- Transform is properly wrapped in release-notes-generator plugin options\n- Guard script still validates notes correctly (9 tests, all passing)\n\nUpdate eslint.config.mjs to allow CommonJS in release.config.mjs (if needed\nfor CI compatibility—actually ESM now so may not be needed, but kept for safety).\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* test: add real end-to-end render test for release notes with contributor credits\n\nAdd comprehensive test that validates the wrapped preset transform by rendering\nactual release notes through @semantic-release/release-notes-generator. This\ntest would have caught the replace-not-wrap bug because the output would have\nbeen broken (no sections, leaked chore, no linkification).\n\nTest covers synthetic commits (thejesh23 fix, owner feat, chore) and asserts:\n1. Non-empty notes output\n2. Section grouping preserved (### headers present)\n3. Exactly one (@thejesh23) credit for external contributor\n4. Owner commits have NO (@mention) suffix\n5. Hidden commits (chore/ci/test) filtered out entirely\n6. References (#679) linkified in GitHub URLs\n\nHermetic fixture (no git dependency) ensures test is stable across branches.\nComplements existing extractGitHubLogin unit tests and config structure checks.\n\nThis render test guards the v10 empty-notes gotcha forever: if transform ever\nreverts to replace-not-wrap, these assertions will fail loudly.\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>\n\n* ci: dedupe login-extraction and fix stale .releaserc.json doc refs\n\nRe-validation of the contributor-credit release-notes change found two\ndrift risks and fixed them before push:\n\n- release.config.mjs defined its own copy of extractGitHubLogin instead\n  of importing the one in scripts/release-notes-transform.cjs, so the\n  unit tests (which do import from that file) were not actually\n  exercising the production regex — the two could silently drift.\n  release.config.mjs now imports the single implementation.\n- eslint.config.mjs's config-file override still referenced a\n  .releaserc.cjs path that never existed on this branch; removed (the\n  new release.config.mjs is already covered by the *.config.* glob).\n- Nine docs/agent-config files (CLAUDE.md x2, CONTRIBUTING.md,\n  docs/DEPLOYMENT.md, docs/DESIGN_DECISIONS.md, ADR-024,\n  .claude/agents/project-steward.md, .claude/skills/deployment-rules,\n  .claude/review-routes.json, a Rust doc-comment, and the guard script's\n  comment) still named the removed .releaserc.json; updated to\n  release.config.mjs. The review-routes.json glob was live-checked by\n  `pnpm check:agent-system`, which failed until fixed.\n\n* fix: harden preset transform resolution and esm require in test\n\nAddress two findings from the AI review gate:\n\n1. Harden release.config.mjs resolvePresetTransform to handle both\n   preset.writer.transform and preset.writerOpts.transform shapes,\n   for version robustness across preset versions (some use writer,\n   others writerOpts). Add explicit function type-check before\n   returning. Update comment to remove imprecise 'commits' mention\n   and clarify that both shapes are handled.\n\n2. Fix scripts/release-notes-transform.test.mjs line 161: add proper\n   ESM-compatible require via createRequire(import.meta.url) at the\n   top of the file (line 4), so the test works under plain node,\n   not just vitest's shim. Guard-validates test still passes.\n\nBoth gates verified:\n- pnpm test -- --project scripts: 15/15 suites, 31/31 tests (including\n  real generateNotes E2E render confirming 3 (@thejesh23) credits)\n- pnpm lint:strict: zero warnings/errors\n- pnpm check:agent-system: in sync\n\n---------\n\nCo-authored-by: Claude Opus 4.8 <noreply@anthropic.com>",
          "timestamp": "2026-07-17T04:55:40+02:00",
          "tree_id": "6a91ca6750ff8352feab4885160c235e327c6dcf",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/a69cace83b81daa5a87737f126ad69c8350df260"
        },
        "date": 1784257435457,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 1725401,
            "range": "± 31521",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2059871,
            "range": "± 10997",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 228067,
            "range": "± 3297",
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
          "id": "4faf353f1725bc844293ec7e8b3148d418e2209f",
          "message": "refactor: consolidate static landing into apps/landing and retire the scroll-film app (#737)\n\n* refactor: consolidate static landing into apps/landing and retire the scroll-film app\n\nThe TERMINAL VELOCITY scroll-film (ADR-0016) is abandoned by owner decision mid-M4.\napps/landing is now the self-contained static site formerly at landing/ (no build step,\nno workspace package); pages.yml publishes it directly. The film milestones M1-M3 remain\nin history; the uncommitted M4 tree is preserved in a stash on feat/tv-m4-robot.\nADR-0017 records the decision and supersedes ADR-0016.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* chore: reroute landing review ownership to project-steward for the static site\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: mark gl fleet dormant in routing table and webgl-standards skill per adr-0017\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-20T20:46:05+02:00",
          "tree_id": "b7eae4beb3eb9fb287d3a913d69628f86a0b6489",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/4faf353f1725bc844293ec7e8b3148d418e2209f"
        },
        "date": 1784573751756,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2229606,
            "range": "± 58430",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2662471,
            "range": "± 43590",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 291163,
            "range": "± 3866",
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
          "id": "408d0dc30eebd6fc308d64705ccc78604ad80bbf",
          "message": "feat: migrate landing to next.js static export with real routes (#740)\n\n* feat: migrate landing to next.js static export with real routes\n\napps/landing becomes a Next 15 static-export workspace app: the 5 authored pages\nare real routes (faithful port, verbatim CSS/HTML slices + client gag scripts),\ndashboards/benchmarks/storybook stay public/ passthrough, flat export keeps every\nlegacy URL working. Adds the version.json release seam with a client freshness\ncheck (tested, incl. injection regression), resurrects check:parity as a permanent\ngate, rewires pages/ci/quality/release workflows + lint/knip/vitest, and lands\nADR-0018 (amends ADR-0017's no-build-step; directory consolidation stands).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* chore: excise unrelated extension wip from migration commit and drop stray turbo input\n\nThe six extension/scrape_url files belonged to a concurrent branch's in-progress\nwork and entered the previous commit via a blanket git add of a shared working\ntree; restored byte-for-byte to origin/main. That work ships on its own branch.\nAlso removes the nonexistent data/** turbo input (review finding).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-21T00:11:51+02:00",
          "tree_id": "41fd07414345066dc5f8fc18f52318e3d005d230",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/408d0dc30eebd6fc308d64705ccc78604ad80bbf"
        },
        "date": 1784586769901,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2152850,
            "range": "± 114280",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2530622,
            "range": "± 23272",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 286340,
            "range": "± 9706",
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
          "id": "b91a5d1cae12cfc3c6577582694ae24089dd3c79",
          "message": "fix(extension): parse linkedin job ad from captured page dom instead of authwalled refetch (#741)\n\n* fix(extension): parse linkedin job ad from captured page dom instead of authwalled refetch\n\nthe canonical (spa/list-view) import branch discarded the extension's captured html and\nserver-fetched /jobs/view/<id>, which linkedin authwalls — imports lost the description.\nresolve(canonical) stays primary; on an unusable or description-less result the bridge now\ngap-fills title/description from the hint-scoped detail pane only (whole-document json-ld\nis ignored so a list shell can never import the wrong job). the content-script hint tries\nvisible job-detail-pane containers before main.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: update extension-import canonical-url seam after fix 2d6a7eda\n\ndocument the list-shell JSON-LD scope safeguard: canonical branch never\ncalls parse_from_html() on captured list-shell DOM; only the hint-scoped\njob_root_generic_html() extraction may gap-fill when resolve() fails.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(extension): walk ancestor chain in job-node visibility gate\n\nisHiddenByStyle only checked the matched element's own computed\ndisplay, which does not inherit; reuse field-signal.ts's isHidden\n(ancestor-walk) instead, built via content.ts's own isolated\nRollup pass so the import stays classic-script-safe.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-21T01:08:02+02:00",
          "tree_id": "b53f58b48d7005f3acede33a33747d7543ed0792",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/b91a5d1cae12cfc3c6577582694ae24089dd3c79"
        },
        "date": 1784589448743,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2208501,
            "range": "± 16430",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2633551,
            "range": "± 14462",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287702,
            "range": "± 6067",
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
          "id": "a0230890dfbfe85f8444fe1a9554cc734ea19d59",
          "message": "fix(jobs): post-756 review findings and ai review sticky restyle (#758)\n\n* docs: broaden adr-029 dash-tail risk note and log review fast-follows\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(scraping): dedupe split request keys before the count cap\n\nclamp_split_request trimmed/blank-filtered/byte-capped other_keys but did not\nde-duplicate before .take(MAX_OTHER_KEYS), so a repeated key wasted one of the\n32 slots (the insert is idempotent anyway). De-dup first-seen order preserved\nvia a HashSet seen-check before the cap; self-pairs equal to the clamped\nmember_key are already dropped. Extended the clamp tests with a 33-entry,\n2-duplicate case → 31 distinct pairs.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(jobs): guard agency list edits pre-load and cap the zod schema\n\nPost-#756 AI-review-gate findings:\n\n- AgencyCompaniesPreferences add()/remove() early-return when useJobPreferences\n  is still undefined (pre-load) — otherwise a quick add builds the next list from\n  an empty [] and replaces the user's saved agency list with just that one entry\n  (single-column setter, but still column data loss). Mirrors the sibling-panel\n  guards in 92e5302b, with a regression test asserting no mutate fires pre-load.\n- JobPreferencesSchema.extraAgencyCompanies gains .max(500) to mirror the Rust\n  MAX_EXTRA_AGENCY_COMPANIES cap (same pattern as otherKeys' .max(32)); IPC\n  codegen unchanged (gen:ipc --check green).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* test(scraping): pin split-request dedup collapse and full cap capacity\n\nThe Stop review-gate asked for two explicit cases on clamp_split_request:\n- duplicate other_keys collapse to ONE entry, first-seen order preserved;\n- de-dup runs BEFORE the count cap, so repeats never steal slots — >32 distinct\n  keys with one key hammered still yields the FULL 32 distinct keys, not fewer.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* ci: restyle ai review sticky comment into severity sections\n\nReplace the cramped one-table sticky body with a claude-review-style\nlayout — bold verdict line, expanded Critical/High/Medium sections with\nper-finding anchors, and a collapsed Low details block. Presentation\nonly: validateFindings/blockingFindings/exit-code logic (ADR-0008) is\nunchanged.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-21T07:04:28+02:00",
          "tree_id": "ebfe5327a600f3ab123e1d3b9c5739f18e4f4952",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/a0230890dfbfe85f8444fe1a9554cc734ea19d59"
        },
        "date": 1784610910889,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2119936,
            "range": "± 46834",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2558016,
            "range": "± 27490",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 287045,
            "range": "± 6853",
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
          "id": "ccaddb3a6b750e43ca05c5d6bd2920e32237d0e2",
          "message": "feat(scraping): harvest ats slugs passively with watched-company autopilot targets (adr-030) (#760)\n\n* feat(scraping): harvest ats slugs passively with watched-company autopilot targets\n\nPassively extract ATS company slugs from every scraped/imported posting URL\nand persist them, so the (later) slug typeahead and watched-company autopilot\ntargets populate with zero user effort (ADR-030).\n\n- extract_ats_ref (scraping/ats_ref.rs): the single URL-shape authority mapping\n  a posting URL to (ats, slug) for greenhouse/lever/personio/workable/ashby/\n  recruitee/smartrecruiters/breezy/bamboohr/pinpoint. scrape_url's greenhouse/\n  lever/ashby/smartrecruiters parsers now reuse these slug fns (no forked\n  shapes); host gates tightened to exact/suffix, slug casing preserved.\n- DiscoveredCompanyStore (discovered/mod.rs): own SQLite db, transactional\n  migration, upsert bump + display-name backfill, starred-first search,\n  watched() pairs. Resettable + DataStore (discoveredCompanies) + managed.\n- Parse-only harvest (commands::discovery::harvest_ats_refs) wired after\n  scrape_boards / scrape_url / autopilot record / extension import; degrades on\n  error, zero new network.\n- discovery IPC namespace (search/setStarred honest {error} union/watched) +\n  service hooks + query keys + mock-client entries.\n- AutopilotTarget gains watchedCompaniesOnly: a run resolves the starred set at\n  run time and injects per-ATS slugs (manual-style fan-out); empty stars skip\n  company boards with needs-company instead of the curated seed.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(scraping): scope watched company fan-out per board and harvest pre-filter\n\nAddress scraping-applier-expert review of the ADR-030 slug-harvest feature.\n\n- HIGH: watched-company autopilot resolution no longer flattens every starred\n  slug into the shared BoardSearchInput.companies (which fanned a foreign ATS's\n  slugs at each company board and crowded its MAX_COMPANIES cap). It now builds a\n  PER-BOARD map (board id -> slugs starred under THAT ATS) and threads it through\n  the engine's existing per-board seeded_companies override via a new\n  scrape_boards_with_overrides; input.companies stays empty. A selected\n  company-scoped board with no matching star is skipped needs-company (never\n  fetched with a foreign slug, never the curated seed). scrape_boards_with_resolver\n  keeps its signature (existing engine tests untouched); the core impl gains the\n  override. New engine mock-transport tests: ashby-only stars -> ashby gets its\n  slugs, greenhouse skipped, no cross-ATS request; plus a per-board purity test.\n- MEDIUM: autopilot harvest moved to the pre-filter scraped postings (ADR-030\n  paragraph c), matching the manual-scrape harvest point.\n- LOW: discovery_set_starred rejects an atsKind that isn't a registered\n  company-scoped board id (registry lookup) so a compromised renderer can't\n  materialize garbage rows.\n- LOW: discovery_watched now uses a dedicated watched_companies() store query\n  (full starred rows, no search-cap coupling); watched() pairs stay for the resolver.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* feat(jobs): slug typeahead with starred companies and watched autopilot target\n\nReplace the ScrapeForm comma-separated company input with an ADR-030 slug\ntypeahead: a new @ajh/ui CompanyTypeahead primitive (multi-select chips +\nper-row star toggle + free-text add) fed by the discovery service hooks,\nmerging passively-harvested slugs with the selected boards' curated seeds.\nAdd a 'My watched ATS companies' target toggle to the autopilot wizard board\nstep that sets watchedCompaniesOnly, with an inline watched list + empty hint.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* test(scraping): close ats harvest acceptance gaps\n\nAdd the missing acceptance-matrix deltas for ADR-030 (passive ATS slug\nharvest + watched companies):\n\n- ats_ref: multi-level subdomains under a real ATS suffix extract None\n  (pins the subdomain_slug label.contains('.') branch).\n- discovery: fixture posting URLs harvested through extract_ats_ref into\n  a real DiscoveredCompanyStore surface via search (URL->store->typeahead\n  seam; reuses the shared production mapping, never re-derived).\n- autopilot_helpers: real store stars resolve through watched() + the\n  registry requires_company filter into per-board targets (the store->\n  resolver seam between the store and pure-resolver tests).\n- CompanySlugField / WatchedCompaniesField: star/unstar failure surfaces\n  an error toast when the mutation throws (no silent failure).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* chore: remove rtk tool prefix from agent and assistant configs\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(jobs): per-company control labels and blur-commit for the slug typeahead\n\nAddress frontend-reviewer findings on 84bf3a77:\n- Per-instance aria-labels: chip remove, star toggle, and wizard unstar now\n  interpolate the company name ({{company}}) so rotor/voice-control users no\n  longer meet N identical controls; CompanyTypeahead's starLabel/removeLabel\n  props take a per-row function. Tests switch to name-based queries.\n- Blur-commit: a typed-but-uncommitted slug is now flushed on focusout (guarded\n  by relatedTarget so a mid-click on a suggestion/star doesn't double-add), so\n  Start Scrape can no longer silently drop it.\n- Result-count aria-live status region on the typeahead (no role=option added).\n- Remove the orphaned jobs.companies.hint key from both locales.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* chore: route apps/landing to the frontend fleet with next.js 16 hardening\n\nlanding ownership moves from project-steward to frontend-author with\nfrontend-reviewer + ui-ux-expert as critics; agent definitions gain\nnext.js 16 static-export guidance (async params, no server features,\nunoptimized images, turbopack, parity gates) sourced from current docs.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: record ats slug harvesting (adr-030) in the knowledge base\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* style: apply rustfmt to discovery acceptance test\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(scraping): bound the watched-companies queries\n\nBoth `DiscoveredCompanyStore::watched()` (pairs) and `watched_companies()` (full\nrows) selected every starred row with no LIMIT, unlike `search`'s clamp(1,100)\ndiscipline (CWE-770 in principle — an unbounded read + per-run autopilot fan-out).\nAdd a shared `WATCHED_LIMIT = 500` bound (generous over any real starred set) to\nboth queries and pin it with a test (WATCHED_LIMIT + 1 stars → exactly 500).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: align adr-030 ipc names with the shipped discovery contract\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(scraping): recognize rippling posting urls in the ats slug extractor\n\nextract_ats_ref covered 10 of the 11 company-scoped ATS boards — Rippling had no\nparser, so a Rippling posting could never be harvested, appear in the typeahead,\nor be starred/watched. Add `rippling_slug` + register in PARSERS.\n\nShape (verified against boards::rippling): posting urls are host-locked to\n`ats.rippling.com/{slug}/jobs/{id}` (the board's `is_valid_rippling_job_url` guard\n+ every parse fixture, e.g. `https://ats.rippling.com/acme/jobs/job-abc-123`).\nCompany slug = first path segment, casing preserved (Rippling slugs are URL path\nsegments, mixed case allowed per `is_valid_rippling_slug` — not DNS labels).\nExact-host gate so the API host `api.rippling.com` (whose first segment is\n`platform`), the apex, and look-alikes → None. Positive + near-miss tests added.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* test(scraping): cover the harvest display-name fallback via a shared pure seam\n\nThe harvest acceptance test re-implemented the display-name fallback as an\nunconditional `Some(company)`, while production does\n`display_name.or_else(|| trim + empty-check)` — the trim/empty branch had zero\ncoverage (ADR-029 seam lesson: a test that re-implements the mapping tests\nnothing). Extract the pure per-posting mapping (`extract_ats_ref` + display\nfallback) into `posting_to_ref` (AppHandle-free); `harvest_ats_refs` and the\nacceptance test now both call it, and a new case pins that an empty/whitespace\ncompany yields `display_name` None (never an empty string).\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(jobs): flush pending slug imperatively before start scrape\n\nThe blur-commit in 592ca7c9 is not sufficient: on WebKit (WKWebView /\nWebKitGTK — the app ships dmg/deb/rpm/appimage) a Start-button click doesn't\nreliably blur the focused input, and even when it does, add() queues an async\nstate update while onStart reads companies synchronously.\n\nCompanyTypeahead now exposes a CompanyTypeaheadHandle.commitPending() via a\nforwarded ref that synchronously runs the pending-query add path (idempotent\nonce the query is cleared). CompanySlugField forwards the ref; ScrapeForm's\nstart flow (Start button + query Enter-submit) calls\nflushSync(() => fieldRef.current?.commitPending()) before onStart when the\ncompany field is shown — the deterministic backstop the removed flushSync\nguard documented. Blur-commit stays for Chromium UX; zero role=option kept.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* chore: scrub rtk from cursor rules and use rg in the stale-branch check\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* docs: point scraping-domain at the extractor and store instead of restating them\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(scraping): validate rippling slugs at extraction and tidy count casts\n\nAddress PR #760 external review quick-wins:\n\n- ats_ref::rippling_slug validated the first path segment against the SAME shape\n  boards::rippling enforces (is_valid_rippling_slug, now pub(crate)), so the store\n  never persists a slug the board would later refuse (path-traversal/query chars,\n  leading/trailing hyphen, over-length). Negative test added.\n- discovered/mod.rs: the seen_count column no longer routes through the epoch-ms\n  ts_from_db/ts_to_db helpers — plain inline casts (u64::try_from(..).unwrap_or(0)\n  / i64::try_from(..).unwrap_or(i64::MAX)) with the same clamp/saturate semantics,\n  since it is a count, not a timestamp. first_seen_at/last_seen_at keep the helpers.\n- engine test: present-but-EMPTY override entry ({\"greenhouse\": []}) is treated as\n  absent — needs-company skip, no fetch — pinning the empty-vec branch of the skip\n  gate distinct from a missing key.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n* fix(jobs): guard rapid star toggles and assert the watched field renders\n\n- CompanySlugField.toggleStar: bail when the star mutation is already in\n  flight (setStarred.isPending), so a rapid double-click can't fire two writes\n  off the same stale option.starred snapshot. Regression test: two rapid clicks\n  during a deferred (in-flight) mutation → the write fires once.\n- StepTarget.test: stop mocking WatchedCompaniesField to null. Render the real\n  component (discovery service hooks stubbed, NotificationProvider added) and\n  assert the watched-companies toggle appears in the target step, so its\n  insertion is actually covered.\n\nCo-Authored-By: Claude Fable 5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Fable 5 <noreply@anthropic.com>",
          "timestamp": "2026-07-21T11:58:08+02:00",
          "tree_id": "9144cd8a8e1f968b263aa36e5ecd94466ddb2dbf",
          "url": "https://github.com/saeedkolivand/ai-job-hunter-app/commit/ccaddb3a6b750e43ca05c5d6bd2920e32237d0e2"
        },
        "date": 1784629157570,
        "tool": "cargo",
        "benches": [
          {
            "name": "pdf/classic",
            "value": 2169186,
            "range": "± 68516",
            "unit": "ns/iter"
          },
          {
            "name": "pdf/atelier_two_column",
            "value": 2597806,
            "range": "± 15442",
            "unit": "ns/iter"
          },
          {
            "name": "docx_classic",
            "value": 288543,
            "range": "± 15128",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}