# Changelog

## [0.124.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.123.0...v0.124.0) (2026-07-08)

### ✨ Features

* copyable email subject and ai select-to-rewrite in apply-by-email ([#565](https://github.com/saeedkolivand/ai-job-hunter-app/issues/565)) ([9395bf4](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9395bf45de95c0e991d718c8318f810300a77c79))
* default the company-search toggle on when the model supports web search ([#567](https://github.com/saeedkolivand/ai-job-hunter-app/issues/567)) ([5544c53](https://github.com/saeedkolivand/ai-job-hunter-app/commit/5544c53349c95e8e173732973750ed82cdc6ceb8))
* fix gemini/codex cli providers on windows and add antigravity ([#582](https://github.com/saeedkolivand/ai-job-hunter-app/issues/582)) ([676f71c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/676f71c6c8100e7e438d0f82b14829720796da27))
* resolve full-history audit findings (fixes, cleanups, docs) ([#564](https://github.com/saeedkolivand/ai-job-hunter-app/issues/564)) ([eb70230](https://github.com/saeedkolivand/ai-job-hunter-app/commit/eb7023079ff0c344ec292c676427eb3496e13b83))

### 🐛 Bug Fixes

* **applications:** dedup indeed imports by job id and link docs via fk ([#568](https://github.com/saeedkolivand/ai-job-hunter-app/issues/568)) ([73fb663](https://github.com/saeedkolivand/ai-job-hunter-app/commit/73fb6631294a9293888183a75ef42a14e3ca3d27))
* floor aggregator sub-day date filters to 3 days ([#569](https://github.com/saeedkolivand/ai-job-hunter-app/issues/569)) ([46f7825](https://github.com/saeedkolivand/ai-job-hunter-app/commit/46f7825c9f1eae6f96892ba91d22ead315c40864))

## [0.123.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.122.0...v0.123.0) (2026-07-07)

### ✨ Features

* add opt-in autopilot ai notes for top matches (headless, notify-only) ([#557](https://github.com/saeedkolivand/ai-job-hunter-app/issues/557)) ([7d2d759](https://github.com/saeedkolivand/ai-job-hunter-app/commit/7d2d759968d44e3ff1773a6f570607e918fc0d3d))
* add optional per-question web search to application answers ([#562](https://github.com/saeedkolivand/ai-job-hunter-app/issues/562)) ([4ceeb66](https://github.com/saeedkolivand/ai-job-hunter-app/commit/4ceeb66b221a34d6803c975bf2637cc28a92733f))
* draft a tailored resume in the prep-this-application flow ([#561](https://github.com/saeedkolivand/ai-job-hunter-app/issues/561)) ([125afd4](https://github.com/saeedkolivand/ai-job-hunter-app/commit/125afd46a3d63296b416aae343bb3f1a71228e94))
* humanize generated application text so it reads as authentic human writing ([#563](https://github.com/saeedkolivand/ai-job-hunter-app/issues/563)) ([169f7ae](https://github.com/saeedkolivand/ai-job-hunter-app/commit/169f7ae8e386b47c58b9546f5cf0986f1d063424))

### 🐛 Bug Fixes

* ground salary-lookup currency in the job's country ([#560](https://github.com/saeedkolivand/ai-job-hunter-app/issues/560)) ([f7765e0](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f7765e0fee720fbb78f16f77d7f157ce911bea2d)), closes [#551](https://github.com/saeedkolivand/ai-job-hunter-app/issues/551)
* prevent prep-this-application runs from getting stuck at pending ([#558](https://github.com/saeedkolivand/ai-job-hunter-app/issues/558)) ([5cf2534](https://github.com/saeedkolivand/ai-job-hunter-app/commit/5cf2534655bd11f609b1da9c7944180160708796))
* repair feature tour on collapsed sidebar, answer text selection, and rewrite popover stacking ([#559](https://github.com/saeedkolivand/ai-job-hunter-app/issues/559)) ([e949bcf](https://github.com/saeedkolivand/ai-job-hunter-app/commit/e949bcfc015bdc78279ef4fa4700795f155cdd7b))

## [0.122.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.121.0...v0.122.0) (2026-07-05)

### ✨ Features

* add agentic tool-calling foundation (provider channel + budgeted loop) ([#552](https://github.com/saeedkolivand/ai-job-hunter-app/issues/552)) ([a6face0](https://github.com/saeedkolivand/ai-job-hunter-app/commit/a6face033944df702c9aaa6cbb008630b1fea44a))
* add prep-application agentic assistant flow with streaming panel ([#555](https://github.com/saeedkolivand/ai-job-hunter-app/issues/555)) ([0fbd7c8](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0fbd7c82946ad7cc7547d2b061c26f9755a874a4))
* gate agent write actions behind human-in-the-loop confirmation ([#556](https://github.com/saeedkolivand/ai-job-hunter-app/issues/556)) ([ce0558f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/ce0558f31c5ce39d20f2bdafed4f8a98092b08d1))

## [0.121.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.120.0...v0.121.0) (2026-07-04)

### ✨ Features

* give a paste-ready number for salary-expectation answers ([#548](https://github.com/saeedkolivand/ai-job-hunter-app/issues/548)) ([c41bc22](https://github.com/saeedkolivand/ai-job-hunter-app/commit/c41bc22c864dcfd43d3e1e1612b8c2ee70222542))
* ground salary answers in a job's scraped salary range ([#551](https://github.com/saeedkolivand/ai-job-hunter-app/issues/551)) ([b3ed856](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b3ed856690e934f2344e4ef562b47f9ad39344e7)), closes [#6](https://github.com/saeedkolivand/ai-job-hunter-app/issues/6)
* research the market salary range for salary-expectation answers ([#549](https://github.com/saeedkolivand/ai-job-hunter-app/issues/549)) ([4bed556](https://github.com/saeedkolivand/ai-job-hunter-app/commit/4bed556c96a6d2a71b77d9ab7157732739d2e9f5))

### 🐛 Bug Fixes

* rate-limit the ai_research_company web-search command ([#550](https://github.com/saeedkolivand/ai-job-hunter-app/issues/550)) ([7da5c92](https://github.com/saeedkolivand/ai-job-hunter-app/commit/7da5c92f275c4dc8fe16df8ab7b4c691d54605ff))
* restore autopilot list to the applied job on back-navigation ([#546](https://github.com/saeedkolivand/ai-job-hunter-app/issues/546)) ([f7bc03f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f7bc03f9c9e0d7f96abf1ccd144e44ba61d909d5))
* stop project links seeding the contact profile on import ([#547](https://github.com/saeedkolivand/ai-job-hunter-app/issues/547)) ([43a78de](https://github.com/saeedkolivand/ai-job-hunter-app/commit/43a78def8fe8623626718ca5b34edc682d348e2d))
* surface newly found autopilot jobs at the top of the list ([#545](https://github.com/saeedkolivand/ai-job-hunter-app/issues/545)) ([af02e53](https://github.com/saeedkolivand/ai-job-hunter-app/commit/af02e53167532450ba5c5260bfaed2da2056461a))

## [0.120.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.119.0...v0.120.0) (2026-07-02)

### ✨ Features

* add job trust / ghost-job validator with badge ([#530](https://github.com/saeedkolivand/ai-job-hunter-app/issues/530)) ([65bf530](https://github.com/saeedkolivand/ai-job-hunter-app/commit/65bf53014ccffcef7611d56444d3496f413c23f7))
* add pinpoint, rippling, breezy, bamboohr ats board scrapers ([#513](https://github.com/saeedkolivand/ai-job-hunter-app/issues/513)) ([29fe2e5](https://github.com/saeedkolivand/ai-job-hunter-app/commit/29fe2e5770ed74fbe21ae1e7dee00738e74dbded))
* add the muse aggregator job board scraper ([#529](https://github.com/saeedkolivand/ai-job-hunter-app/issues/529)) ([32e1dd3](https://github.com/saeedkolivand/ai-job-hunter-app/commit/32e1dd316215b4f44410df5de77e260922c4820a))
* scraping hardening + workable and comeet boards (21 to 23) ([#535](https://github.com/saeedkolivand/ai-job-hunter-app/issues/535)) ([47111d9](https://github.com/saeedkolivand/ai-job-hunter-app/commit/47111d9a085da96c74c64ce1366f0bbd2dc812ac))

### 🐛 Bug Fixes

* harden rippling scraper against a single malformed row ([#531](https://github.com/saeedkolivand/ai-job-hunter-app/issues/531)) ([75f633e](https://github.com/saeedkolivand/ai-job-hunter-app/commit/75f633e5848c1921386748375e299b0e56052ba0))

### ♻️ Refactors

* rename apps/tauri to apps/desktop and @ajh/tauri to @ajh/desktop ([#512](https://github.com/saeedkolivand/ai-job-hunter-app/issues/512)) ([04e6e78](https://github.com/saeedkolivand/ai-job-hunter-app/commit/04e6e78aed29a22b3f1d370a91823bd89cdf1ef9))

## [0.119.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.118.2...v0.119.0) (2026-06-30)

### ✨ Features

* add apify linkedin aggregator provider (opt-in) ([#510](https://github.com/saeedkolivand/ai-job-hunter-app/issues/510)) ([075e240](https://github.com/saeedkolivand/ai-job-hunter-app/commit/075e24010ea48ba2105a40e8c96c5022ed63c63e))
* export a redacted diagnostics bundle for crash reports ([#509](https://github.com/saeedkolivand/ai-job-hunter-app/issues/509)) ([87f0b97](https://github.com/saeedkolivand/ai-job-hunter-app/commit/87f0b97c4860479723c8df4a6f31f34edd48b0cb))
* apply by email — generate a tailored application email per job ([#508](https://github.com/saeedkolivand/ai-job-hunter-app/issues/508)) ([708c6fc](https://github.com/saeedkolivand/ai-job-hunter-app/commit/708c6fc852f688840db36083b579442ff7ac0d01))

### 🐛 Bug Fixes

* jobs-list scroll restore, clearer selection, rewrite ux ([#506](https://github.com/saeedkolivand/ai-job-hunter-app/issues/506)) ([8dee5d6](https://github.com/saeedkolivand/ai-job-hunter-app/commit/8dee5d672a25755d307147977546b8098b2e4ae3))
* prevent cover-letter extractor panic on multibyte job ads ([#504](https://github.com/saeedkolivand/ai-job-hunter-app/issues/504)) ([f622c5c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f622c5c08421375dd11b5af79a5cac28122eb751))

### 🎨 UI/UX

* align landing fleet authors header with its cards ([6efea88](https://github.com/saeedkolivand/ai-job-hunter-app/commit/6efea88298e029dae55a105b467a09ac7bf8f5f8))

### 📚 Documentation

* bump privacy policy last-updated date ([e640c88](https://github.com/saeedkolivand/ai-job-hunter-app/commit/e640c886d226f978997ab2306fac58361f31ae9c))
* correct extension import mode, disclose native-messaging, note rate budget ([#507](https://github.com/saeedkolivand/ai-job-hunter-app/issues/507)) ([951a480](https://github.com/saeedkolivand/ai-job-hunter-app/commit/951a480726dc850ae0c7c956ddab33d3c0950deb))

## [0.118.2](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.118.1...v0.118.2) (2026-06-25)

### 🎨 UI/UX

* **extension:** recolor icons to brand teal and regenerate promo cards ([2a68df9](https://github.com/saeedkolivand/ai-job-hunter-app/commit/2a68df91a26dc112ab40e3aa1be5e450d4da2edd)), closes [#5bb8b7](https://github.com/saeedkolivand/ai-job-hunter-app/issues/5bb8b7)

## [0.118.1](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.118.0...v0.118.1) (2026-06-25)

### 🐛 Bug Fixes

* **landing:** center assembly-line token within belt viewport ([0d73544](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0d73544f969560abfb8a52c979660dc7a417e78a))

### 🎨 UI/UX

* **icons:** recolor app and tray icons from purple to brand teal ([0ab036b](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0ab036b0282411c8af9fcf7927efa78fec46f8c8)), closes [#5bb8b7](https://github.com/saeedkolivand/ai-job-hunter-app/issues/5bb8b7)
* **landing:** rebuild agent-system as dark ink explainer with drawn assembly line ([67526bb](https://github.com/saeedkolivand/ai-job-hunter-app/commit/67526bb35ed8fa4b4bac0ca60575315523045144))

### 📚 Documentation

* **agents:** correct stale agent count and config drift ([b311945](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b31194564fe3f61824a8083e97360307dcbcf04e))
* drop stale codegraph file count ([81cfb76](https://github.com/saeedkolivand/ai-job-hunter-app/commit/81cfb769d984653240b509a9b7749b8f0e3e8833))
* fix stale semgrep workflow reference in coderabbit config ([30899a9](https://github.com/saeedkolivand/ai-job-hunter-app/commit/30899a95ddc36143172fee5ef17be2a8cf6f5c0e))
* update agent system landing page ([f54032c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f54032c6bdc886df31a709513018eb114ecc8b0e))

## [0.118.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.117.0...v0.118.0) (2026-06-25)

### ✨ Features

* github project import, job-summary fixes, and answer rewrite ([#500](https://github.com/saeedkolivand/ai-job-hunter-app/issues/500)) ([5bbc5c7](https://github.com/saeedkolivand/ai-job-hunter-app/commit/5bbc5c7a69746abacd488481f32bc0a222a33df9)), closes [#236](https://github.com/saeedkolivand/ai-job-hunter-app/issues/236)

## [0.117.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.116.1...v0.117.0) (2026-06-24)

### ✨ Features

* **jobs:** linkedin-style jobs page, viewed dwell, show-more dedup ([#499](https://github.com/saeedkolivand/ai-job-hunter-app/issues/499)) ([be378fb](https://github.com/saeedkolivand/ai-job-hunter-app/commit/be378fb5637673ed443dcc0da56a3cea8026c9d5))

## [0.116.1](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.116.0...v0.116.1) (2026-06-24)

### 🐛 Bug Fixes

* invalidate the interactions query prefix when clearing interactions ([#487](https://github.com/saeedkolivand/ai-job-hunter-app/issues/487)) ([ca3c686](https://github.com/saeedkolivand/ai-job-hunter-app/commit/ca3c686b8379001a3d2155e2bc8a0bf2ebdb01d8))
* **landing:** a11y-floor pass across all pages and harden the frontend standard ([#489](https://github.com/saeedkolivand/ai-job-hunter-app/issues/489)) ([0c9e2ab](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0c9e2ab16289ee66d492e186357a42db5cbdc6bd))
* **landing:** point agent-system links to root and add favicons ([#493](https://github.com/saeedkolivand/ai-job-hunter-app/issues/493)) ([03b617f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/03b617feca583ace855efe91d1763bf2b4b88cbf))
* **landing:** resolve the re-audit second-pass a11y findings and 2 regressions ([#490](https://github.com/saeedkolivand/ai-job-hunter-app/issues/490)) ([20b299d](https://github.com/saeedkolivand/ai-job-hunter-app/commit/20b299d5e2aeb51fc40ee6b374d010d69df72351))
* **landing:** restore straight quotes so the creature film plays ([#492](https://github.com/saeedkolivand/ai-job-hunter-app/issues/492)) ([e84b518](https://github.com/saeedkolivand/ai-job-hunter-app/commit/e84b518b63d8027d20a5e9741d62ee0b8d085425)), closes [#endcard](https://github.com/saeedkolivand/ai-job-hunter-app/issues/endcard)

## [0.116.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.115.1...v0.116.0) (2026-06-24)

### ✨ Features

* jobs split view, markdown descriptions, on-demand scoring, and linux/steam deck support ([#486](https://github.com/saeedkolivand/ai-job-hunter-app/issues/486)) ([bc6c9e8](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bc6c9e8789b39f85c208967163a067d3da6a1f18))

## [0.115.1](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.115.0...v0.115.1) (2026-06-23)

### 🐛 Bug Fixes

* **autopilot:** forward country to aggregator + stop keyword-prefill (with dep bump, landing OG fix) ([#483](https://github.com/saeedkolivand/ai-job-hunter-app/issues/483)) ([bc5e00e](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bc5e00e1040936ed59ee4dfef71b36f71fe75d86))
* **autopilot:** match manual-search filters and surface zero-result reasons ([#484](https://github.com/saeedkolivand/ai-job-hunter-app/issues/484)) ([16ebdf7](https://github.com/saeedkolivand/ai-job-hunter-app/commit/16ebdf751de96dc998f9f6b3e477517c95f6e243)), closes [pre-#483](https://github.com/saeedkolivand/pre-/issues/483) [#483](https://github.com/saeedkolivand/ai-job-hunter-app/issues/483)
* **autopilot:** redact standalone credential tokens in scrape diagnostics ([#485](https://github.com/saeedkolivand/ai-job-hunter-app/issues/485)) ([b53e296](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b53e296b023f83930f9fc574c922fc7c9284ca92)), closes [#484](https://github.com/saeedkolivand/ai-job-hunter-app/issues/484)

## [0.115.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.114.0...v0.115.0) (2026-06-23)

### ✨ Features

* auto-commit preview edits and add referral improve-with-ai ([#476](https://github.com/saeedkolivand/ai-job-hunter-app/issues/476)) ([bc78377](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bc7837727cb151e430f14e86542ce85bae993f4d))

### 🐛 Bug Fixes

* buffer cold-start deep-link autopilot focus so it isn't lost ([#477](https://github.com/saeedkolivand/ai-job-hunter-app/issues/477)) ([f15e83d](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f15e83d190cf720aa3eb0af4d6e6253664b9ce10))
* show aggregator in autopilot; centralize board and date-filter constants ([#473](https://github.com/saeedkolivand/ai-job-hunter-app/issues/473)) ([99e7286](https://github.com/saeedkolivand/ai-job-hunter-app/commit/99e7286d3a68571af4de811362d41c94a9d156b5))

### 🎨 UI/UX

* flat white content surfaces in light mode + invalid-field borders ([#482](https://github.com/saeedkolivand/ai-job-hunter-app/issues/482)) ([e49dac1](https://github.com/saeedkolivand/ai-job-hunter-app/commit/e49dac1687d7bb3ba7fcb17e97a5e805211f3aa0))

### ♻️ Refactors

* centralize credential slot names in @ajh/shared via codegen ([#472](https://github.com/saeedkolivand/ai-job-hunter-app/issues/472)) ([5299a13](https://github.com/saeedkolivand/ai-job-hunter-app/commit/5299a13cdb04e239761e5a45c2ed28915b551324))
* centralize data-testid literals into @ajh/test-ids package ([#471](https://github.com/saeedkolivand/ai-job-hunter-app/issues/471)) ([1198158](https://github.com/saeedkolivand/ai-job-hunter-app/commit/11981580c5f2c5dc53579c95932129557ce69855))
* centralize timeouts/durations and use section/stage registries ([#474](https://github.com/saeedkolivand/ai-job-hunter-app/issues/474)) ([1196b8c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1196b8c901b0f39e49c79b9e3e674797160ec8d3))
* replace the native splash window with an in-app overlay ([#475](https://github.com/saeedkolivand/ai-job-hunter-app/issues/475)) ([48f9158](https://github.com/saeedkolivand/ai-job-hunter-app/commit/48f9158f178aff1ccdaa1a6f2dc349d1b29c1c7b))

## [0.114.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.113.0...v0.114.0) (2026-06-22)

### ✨ Features

* settings search, theme-aware splash, aggregator default + tailoring fix, extension polish ([#470](https://github.com/saeedkolivand/ai-job-hunter-app/issues/470)) ([ca59779](https://github.com/saeedkolivand/ai-job-hunter-app/commit/ca59779f66c173afedf177895ee75c5d42dcfc1f))

## [0.113.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.112.0...v0.113.0) (2026-06-21)

### ✨ Features

* **jobs:** prompt for adzuna key in onboarding and scrape form ([#466](https://github.com/saeedkolivand/ai-job-hunter-app/issues/466)) ([e0bafe6](https://github.com/saeedkolivand/ai-job-hunter-app/commit/e0bafe681649bb1b681ee73cc941bae4105679b1))
* **scraping:** add adzuna and jsearch aggregator with key settings ([#465](https://github.com/saeedkolivand/ai-job-hunter-app/issues/465)) ([be2ff48](https://github.com/saeedkolivand/ai-job-hunter-app/commit/be2ff48b44e94fd3f16e8e857388e273203713a3))
* **scraping:** add company identifier for company-scoped boards ([#464](https://github.com/saeedkolivand/ai-job-hunter-app/issues/464)) ([06c5617](https://github.com/saeedkolivand/ai-job-hunter-app/commit/06c5617a1aaf505621fb0a6767f0c8c56cf438e5))

### 🐛 Bug Fixes

* **jobs:** persist scrape results across navigation ([#463](https://github.com/saeedkolivand/ai-job-hunter-app/issues/463)) ([bce7a1c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bce7a1c6575ff33e3786bf4f74f8525a9de7dc37))
* **scraping:** harden company-scoped boards (ssrf, fan-out caps, stale events) ([#467](https://github.com/saeedkolivand/ai-job-hunter-app/issues/467)) ([ea681ef](https://github.com/saeedkolivand/ai-job-hunter-app/commit/ea681ef2c4961e20293ede4e394cb16873f550c4)), closes [#464](https://github.com/saeedkolivand/ai-job-hunter-app/issues/464)

### ♻️ Refactors

* **scraping:** retire 5 anti-bot board scrapers covered by the aggregator ([#469](https://github.com/saeedkolivand/ai-job-hunter-app/issues/469)) ([bd3c344](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bd3c34424d28f3a169b78e2fed06edb407525309))

### 📚 Documentation

* **scraping:** document company-scoped boards and the aggregator ([#468](https://github.com/saeedkolivand/ai-job-hunter-app/issues/468)) ([e1e2b1f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/e1e2b1fa7e82de9365efd76daa868323b72245bd)), closes [463-#467](https://github.com/saeedkolivand/463-/issues/467)

## [0.112.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.111.1...v0.112.0) (2026-06-21)

### ✨ Features

* **jobs:** disable scrape until required boards are logged in ([#458](https://github.com/saeedkolivand/ai-job-hunter-app/issues/458)) ([7e12ad5](https://github.com/saeedkolivand/ai-job-hunter-app/commit/7e12ad56defc776cf096a500fc5b41cd5850abdc))
* **jobs:** repair broken job boards and gate logged-out scraping ([#462](https://github.com/saeedkolivand/ai-job-hunter-app/issues/462)) ([0e3535f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0e3535f2642266a974b5610f16adcc07371276e3)), closes [#458](https://github.com/saeedkolivand/ai-job-hunter-app/issues/458)
* **scraping:** scrape multiple job boards per run with rate-limit hardening ([#455](https://github.com/saeedkolivand/ai-job-hunter-app/issues/455)) ([92bf979](https://github.com/saeedkolivand/ai-job-hunter-app/commit/92bf97973d9e2843c2940ae68cac7df98cbf4149))
* **scraping:** surface per-board login requirement in jobs picker ([#449](https://github.com/saeedkolivand/ai-job-hunter-app/issues/449)) ([eff970d](https://github.com/saeedkolivand/ai-job-hunter-app/commit/eff970d3036404bc728c53b91bfdd2e72a10d63a))

### 🐛 Bug Fixes

* **ai:** make cli-agent stream cancellable and move gemini auth to header ([#443](https://github.com/saeedkolivand/ai-job-hunter-app/issues/443)) ([92df12a](https://github.com/saeedkolivand/ai-job-hunter-app/commit/92df12a3bdfe0e45ada1b494cb16aa18340cb997))
* **data:** surface partial restore failures instead of reporting success ([#434](https://github.com/saeedkolivand/ai-job-hunter-app/issues/434)) ([44afc66](https://github.com/saeedkolivand/ai-job-hunter-app/commit/44afc6613c018b32c7e74e5d588877211cd22ed8))
* **extension:** validate token format in background settoken handler ([#444](https://github.com/saeedkolivand/ai-job-hunter-app/issues/444)) ([928cfb2](https://github.com/saeedkolivand/ai-job-hunter-app/commit/928cfb28670616af5142b32dd75338a8bc8a0c83))
* **i18n:** add the glassdoor board label ([#457](https://github.com/saeedkolivand/ai-job-hunter-app/issues/457)) ([11ab8c0](https://github.com/saeedkolivand/ai-job-hunter-app/commit/11ab8c0a38b2acf9be49a2d96b6d4c92086e0baf)), closes [#456](https://github.com/saeedkolivand/ai-job-hunter-app/issues/456)
* **job-match:** align evidence grounding with scorer and add guidance framing ([#442](https://github.com/saeedkolivand/ai-job-hunter-app/issues/442)) ([2dd85c2](https://github.com/saeedkolivand/ai-job-hunter-app/commit/2dd85c20dddc3b0b835f7cd3ce36452fbba0e9fa)), closes [#447](https://github.com/saeedkolivand/ai-job-hunter-app/issues/447)
* **jobs:** list glassdoor in the board picker ([#456](https://github.com/saeedkolivand/ai-job-hunter-app/issues/456)) ([b9e90be](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b9e90be42edb32c5f726378a68d646015a05804b)), closes [#455](https://github.com/saeedkolivand/ai-job-hunter-app/issues/455)
* **menu:** gate the menu_take_pending poll to macos only ([#451](https://github.com/saeedkolivand/ai-job-hunter-app/issues/451)) ([f9050ba](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f9050ba21a6cb83afe5270323dd6208515c460a6))
* **prompts:** truncate oversized sections and strip fenced code blocks ([#446](https://github.com/saeedkolivand/ai-job-hunter-app/issues/446)) ([448a428](https://github.com/saeedkolivand/ai-job-hunter-app/commit/448a428094303c334e7a61504c01ff5dd1a78bbe))
* **renderer:** a11y, i18n, and design-token fixes across renderer and ui ([#439](https://github.com/saeedkolivand/ai-job-hunter-app/issues/439)) ([91d8669](https://github.com/saeedkolivand/ai-job-hunter-app/commit/91d86694679039565236745c9d8bd0df8cd1e32d))
* **resume:** preserve literal markdown chars and resolve region-suffixed locales ([#441](https://github.com/saeedkolivand/ai-job-hunter-app/issues/441)) ([9664d21](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9664d218a716a97ba54a4884b78459d082f1e9c8))
* **scraping:** close glassdoor browser on error and share linkedin rate limiter ([#436](https://github.com/saeedkolivand/ai-job-hunter-app/issues/436)) ([62f3ff4](https://github.com/saeedkolivand/ai-job-hunter-app/commit/62f3ff46cadd417efe4cc3664ce45dcf52238cd5))
* **scraping:** preserve caller headers and stream-cap http responses ([#454](https://github.com/saeedkolivand/ai-job-hunter-app/issues/454)) ([9b40e7b](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9b40e7bdbdbe5d2a2672694d2c9d3aad664a1fea))
* **scraping:** repair drifted board scrapers and add live smoke tests ([#450](https://github.com/saeedkolivand/ai-job-hunter-app/issues/450)) ([b9817a9](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b9817a9fb2bf2dfcac863b9343d30863504b05a1))
* **security:** restrict system_open_external to http and https schemes ([#435](https://github.com/saeedkolivand/ai-job-hunter-app/issues/435)) ([44aa347](https://github.com/saeedkolivand/ai-job-hunter-app/commit/44aa3472fa11f43c991e22f8f83134b4ba98db3e))
* **ui:** keep modal open when a text-selection drag ends on the backdrop ([#452](https://github.com/saeedkolivand/ai-job-hunter-app/issues/452)) ([7d43d60](https://github.com/saeedkolivand/ai-job-hunter-app/commit/7d43d6081ae1620a58ac1d944c4ea53a9a8c7178))

### ⚡ Performance

* **db:** run sqlite off the async runtime and speed up store hot paths ([#437](https://github.com/saeedkolivand/ai-job-hunter-app/issues/437)) ([c2ac215](https://github.com/saeedkolivand/ai-job-hunter-app/commit/c2ac215aef402b92d9228211d56aef40db520547))

### 🎨 UI/UX

* clarify autopilot wizard target labels ([#453](https://github.com/saeedkolivand/ai-job-hunter-app/issues/453)) ([31285b9](https://github.com/saeedkolivand/ai-job-hunter-app/commit/31285b948570c02b2a9447e86cd10bfd57270b55))

### ♻️ Refactors

* dedupe now_ms, job-id, and renderer doc helpers ([#438](https://github.com/saeedkolivand/ai-job-hunter-app/issues/438)) ([67a6a6b](https://github.com/saeedkolivand/ai-job-hunter-app/commit/67a6a6b83d386755e40a961dea80691de8a95d9a))

### 📚 Documentation

* add scraping endpoint reconnaissance for all 20 boards ([#461](https://github.com/saeedkolivand/ai-job-hunter-app/issues/461)) ([5002ff9](https://github.com/saeedkolivand/ai-job-hunter-app/commit/5002ff9fac50e1b1102c9500f8357733d2694003))
* capture fleet-audit decisions and intentional simplifications ([#448](https://github.com/saeedkolivand/ai-job-hunter-app/issues/448)) ([4ffa4fc](https://github.com/saeedkolivand/ai-job-hunter-app/commit/4ffa4fc609d2444bf3872962c3bc7af2edfd6045))

## [0.111.1](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.111.0...v0.111.1) (2026-06-19)

### 🐛 Bug Fixes

* cut release for pending extension bridge auth fix and ui polish ([1db08ee](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1db08eec778b68815a3eca0b3319b11d8ae1e12f))

## [0.111.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.110.0...v0.111.0) (2026-06-19)

### ✨ Features

* sponsorship, agent-system redesign, landing perf, i18n parity ([#431](https://github.com/saeedkolivand/ai-job-hunter-app/issues/431)) ([6999cc2](https://github.com/saeedkolivand/ai-job-hunter-app/commit/6999cc22be610a1e2a263ac111d139f7260ba6a2))

## [0.110.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.109.4...v0.110.0) (2026-06-19)

### ✨ Features

* add a download button to each saved resume in the uploader ([#423](https://github.com/saeedkolivand/ai-job-hunter-app/issues/423)) ([89b1824](https://github.com/saeedkolivand/ai-job-hunter-app/commit/89b1824a3e991d37e46d2fed565ca12a74871247))
* add a global back button to the titlebar ([#421](https://github.com/saeedkolivand/ai-job-hunter-app/issues/421)) ([6d426bf](https://github.com/saeedkolivand/ai-job-hunter-app/commit/6d426bfb493bdb0dab8fb434b13102610c8dd6b6))
* apply pending extension updates immediately ([#425](https://github.com/saeedkolivand/ai-job-hunter-app/issues/425)) ([0907e7b](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0907e7b86955c252959406aa30b67372cf42bcea))
* collapsible sidebar with persisted state ([#424](https://github.com/saeedkolivand/ai-job-hunter-app/issues/424)) ([1663caf](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1663cafab2ecad6f54de09c12854cdf07f9dc190))
* harden extension job import and gate release behind a manual trigger ([#418](https://github.com/saeedkolivand/ai-job-hunter-app/issues/418)) ([f54b3f4](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f54b3f4f3b97f0bdd076f9a57095d86ed3b76bb6))
* persist the job-ad summary and add a language selector ([#426](https://github.com/saeedkolivand/ai-job-hunter-app/issues/426)) ([9793258](https://github.com/saeedkolivand/ai-job-hunter-app/commit/979325846b92eaf4a9747abd46ad8a475216be84))

### 🐛 Bug Fixes

* keep the autopilot edit wizard open on backdrop click ([#420](https://github.com/saeedkolivand/ai-job-hunter-app/issues/420)) ([ce87b82](https://github.com/saeedkolivand/ai-job-hunter-app/commit/ce87b82dfc67a4521197865271d09de01cb88071))
* let users change a stored provider key in settings in place ([#422](https://github.com/saeedkolivand/ai-job-hunter-app/issues/422)) ([1254338](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1254338b99b5a1c9e5938fe14d22f5db4ba8315e))

### ⚡ Performance

* reduce aurora background blur on retina displays ([#427](https://github.com/saeedkolivand/ai-job-hunter-app/issues/427)) ([b30287c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b30287c7c71dea0dd9239ad5fb184678169a55ef))

### 📚 Documentation

* sync readme scripts with current package.json ([#419](https://github.com/saeedkolivand/ai-job-hunter-app/issues/419)) ([9bf78f4](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9bf78f442b33994476409b9b8aae382e9bb9a0a2))

## [0.109.4](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.109.3...v0.109.4) (2026-06-18)

### 🐛 Bug Fixes

* keep app chrome fixed and drop redundant results referral button ([#417](https://github.com/saeedkolivand/ai-job-hunter-app/issues/417)) ([886f091](https://github.com/saeedkolivand/ai-job-hunter-app/commit/886f091f030a34cd4411d0262aa2cae03a812133))

## [0.109.3](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.109.2...v0.109.3) (2026-06-18)

### 🐛 Bug Fixes

* keep popup views hidden, scroll small viewports, drop autopilot auto-apply copy ([#416](https://github.com/saeedkolivand/ai-job-hunter-app/issues/416)) ([366c5e4](https://github.com/saeedkolivand/ai-job-hunter-app/commit/366c5e4c6f51d395b9906e375783204e9f573091))

## [0.109.2](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.109.1...v0.109.2) (2026-06-17)

### ⚡ Performance

* emit social card as compressed jpeg to cut og image 83% ([#415](https://github.com/saeedkolivand/ai-job-hunter-app/issues/415)) ([028256f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/028256fa9fd0ae3aca3cefc28de3e3f553f58d3d))

## [0.109.1](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.109.0...v0.109.1) (2026-06-17)

### 🐛 Bug Fixes

* firefox-on-windows extension connectivity (native-host launch detection) ([#412](https://github.com/saeedkolivand/ai-job-hunter-app/issues/412)) ([df6863f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/df6863f33041af87761808a3b539da169dc6ab26))

## [0.109.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.108.1...v0.109.0) (2026-06-17)

### ✨ Features

* editable job-ad summary tabs + custom application questions ([#396](https://github.com/saeedkolivand/ai-job-hunter-app/issues/396)) ([fdffe6c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/fdffe6c8e0ab407e3192b6fae5eac81a4e0196d1)), closes [#403](https://github.com/saeedkolivand/ai-job-hunter-app/issues/403)

## [0.108.1](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.108.0...v0.108.1) (2026-06-17)

### 🐛 Bug Fixes

* **extension:** remove "looking for the desktop app" searching placeholder ([#395](https://github.com/saeedkolivand/ai-job-hunter-app/issues/395)) ([77d3149](https://github.com/saeedkolivand/ai-job-hunter-app/commit/77d3149d58d26738069b90fbc690cb58054e2acc)), closes [#view-searching](https://github.com/saeedkolivand/ai-job-hunter-app/issues/view-searching)

## [0.108.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.107.0...v0.108.0) (2026-06-17)

### ✨ Features

* redesign resume input to library-first card ([#393](https://github.com/saeedkolivand/ai-job-hunter-app/issues/393)) ([1f8f01f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1f8f01f26cedbc43140b259d5b65609de8f9d456))

### 📚 Documentation

* responsive layout conventions + ModalShell slot API ([#392](https://github.com/saeedkolivand/ai-job-hunter-app/issues/392)) ([77a5d89](https://github.com/saeedkolivand/ai-job-hunter-app/commit/77a5d893f86bff6b04fb6a93d121d0663a8df4b4)), closes [#391](https://github.com/saeedkolivand/ai-job-hunter-app/issues/391)

## [0.107.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.106.0...v0.107.0) (2026-06-17)

### ✨ Features

* app-wide window-resize responsiveness + scroll-to-top nav ([#391](https://github.com/saeedkolivand/ai-job-hunter-app/issues/391)) ([b79dab4](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b79dab462fff0bd6d8a19eafefeb2c830f20f4ee))

## [0.106.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.105.0...v0.106.0) (2026-06-16)

### ✨ Features

* import selected job from board list views + reset/pairing fixes ([#390](https://github.com/saeedkolivand/ai-job-hunter-app/issues/390)) ([dfc837d](https://github.com/saeedkolivand/ai-job-hunter-app/commit/dfc837d5d87aebf4d60aea34a70ca78b7a8ada97))

## [0.105.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.104.3...v0.105.0) (2026-06-16)

### ✨ Features

* native-messaging transport for firefox https-only mode ([#387](https://github.com/saeedkolivand/ai-job-hunter-app/issues/387)) ([96274db](https://github.com/saeedkolivand/ai-job-hunter-app/commit/96274db22c8bf3d5b1dbd715610e086eae324afc))

## [0.104.3](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.104.2...v0.104.3) (2026-06-16)

### 🎨 UI/UX

* slim the extension offline state and move retry to a header icon ([#386](https://github.com/saeedkolivand/ai-job-hunter-app/issues/386)) ([b56349c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b56349c0e6e71bb8fe95984679e2ec251db260ee))

## [0.104.2](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.104.1...v0.104.2) (2026-06-16)

### 🐛 Bug Fixes

* import jobs as applications only, sidebar nav, tab order, popup UX + Firefox launch ([#385](https://github.com/saeedkolivand/ai-job-hunter-app/issues/385)) ([465ea0b](https://github.com/saeedkolivand/ai-job-hunter-app/commit/465ea0b7eed5b97e92cde3f776df14c98f62d180))

## [0.104.1](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.104.0...v0.104.1) (2026-06-16)

### 🐛 Bug Fixes

* download page button seam + serialize release main pushes ([#384](https://github.com/saeedkolivand/ai-job-hunter-app/issues/384)) ([3f8227f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/3f8227fe10099896e3db8cc885b691afb947800a))

## [0.104.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.103.0...v0.104.0) (2026-06-16)

### ✨ Features

* ui improvements across applications, settings, jobs, onboarding, and the design system ([#383](https://github.com/saeedkolivand/ai-job-hunter-app/issues/383)) ([22298c3](https://github.com/saeedkolivand/ai-job-hunter-app/commit/22298c3d7a3da96649f75cefc93ea59e67eebf8f)), closes [#2ea36b](https://github.com/saeedkolivand/ai-job-hunter-app/issues/2ea36b) [#0f6b3f](https://github.com/saeedkolivand/ai-job-hunter-app/issues/0f6b3f) [#f7a325](https://github.com/saeedkolivand/ai-job-hunter-app/issues/f7a325) [#c2740a](https://github.com/saeedkolivand/ai-job-hunter-app/issues/c2740a) [#272729](https://github.com/saeedkolivand/ai-job-hunter-app/issues/272729) [#1a1a1d](https://github.com/saeedkolivand/ai-job-hunter-app/issues/1a1a1d) [#366569](https://github.com/saeedkolivand/ai-job-hunter-app/issues/366569) [#ba9249](https://github.com/saeedkolivand/ai-job-hunter-app/issues/ba9249)

### 📚 Documentation

* landing seo/social/a11y polish + relicense to polyform noncommercial 1.0.0 ([#382](https://github.com/saeedkolivand/ai-job-hunter-app/issues/382)) ([9b542cc](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9b542ccd84fca93edc67bb314cf4a1f348268feb))
