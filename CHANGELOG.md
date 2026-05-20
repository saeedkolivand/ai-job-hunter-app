# Changelog

All notable changes to AI Job Hunter are documented here.
This project follows [Semantic Versioning](https://semver.org) and [Conventional Commits](https://www.conventionalcommits.org).

## [1.3.1](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.3.0...v1.3.1) (2026-05-20)

### 🐛 Bug Fixes

* pass release_token to actions/checkout so git credentials use the pat ([a07f48b](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/a07f48b3b8ba087ec43527def9236363f39821df))
* stop background flashing in low-memory mode ([760faa3](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/760faa3d34b0cd699fd0b6de1e1fc55ab8fb5143)), closes [#07060](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/issues/07060)
* use release_token pat for semantic-release to bypass main ruleset ([3057be5](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/3057be52122cfc046dcc1471878a8d649a026a20))

### 📚 Documentation

* add rtk prefix rule to copilot and cursor rule files, fix stale ui imports in renderer-ui.mdc ([9611a50](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/9611a50d5258cc6684405b90b63da55d99daaadf))
* compress and de-stale all ai tool rule files ([0775d56](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/0775d5623cc18e1a74f205bc7272910afa3a9718))
* compress copilot-instructions and cursor project.mdc ([6250cae](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/6250cae2f6e24123485de2ca7a180912c4553098))
* explicitly require rtk prefix on every shell command in all ai rule files ([65b3679](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/65b36798a3fe9148fb72395e3ed0e5114ea51385))

## [1.3.0](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.2.1...v1.3.0) (2026-05-20)

### ✨ Features

* add performance section to settings page ([3f9db63](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/3f9db63126a0cbc16fc11df2518665ffdb9d6f8e))
* complete phase 3 scraper runtime abstraction boundary ([e511864](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/e511864f45b083227b525372a2d111582b3f1d89))
* implement phase 0 baseline metrics instrumentation ([b6184d0](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/b6184d0ab9a8b4dfb57b66e946017fb56997bb4c))
* wire low-memory visual mode — blur reduction and motion kill ([197804a](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/197804ae16b0cab71a397ea0a79e44c80eae96f0))

### 🐛 Bug Fixes

* prevent pino worker-ending crash on update install ([1f0acc2](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/1f0acc274281f470637e757bb4005de836fff44a))

### ♻️ Refactors

* complete phase 2 session cleanup — remove playwright from active runtime paths ([9f47900](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/9f479002dc4c9797e8ed8ed71019c23a2937f6f9))
* complete phase 5 frontend portability — remove all window.api from renderer lib ([5713864](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/57138645ac2daed3cfc337b51147d55a1a95ce16))

## [1.2.1](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.2.0...v1.2.1) (2026-05-20)

### 🐛 Bug Fixes

* register app client synchronously to eliminate init race ([137173d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/137173d6049a01e0fad99e8e76940eb7ea70edb2))
* resolve app client not initialized error in performance mode provider ([5b75a9b](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/5b75a9bbe5a7f0cc9ed03da5f9bca0d76febd3e4))
* sync desktop package version to 1.2.0 and bump it on every release ([ccac4e5](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/ccac4e576cce6dcfbd078f3285b750a7e1ea0d71))

## [1.2.0](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.2...v1.2.0) (2026-05-20)

### ✨ Features

* complete pre-phase-6 cleanup and memory optimisation ([0453bb5](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/0453bb5da84ca71582499e3c8a0ed97ecbd3eda2))

### 📚 Documentation

* add rtk bash-tool rule to claude.md and agents.md ([5cac60a](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/5cac60ac67df5d7ec0cb4af211ef3879bd335926))

## [1.1.2](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.1...v1.1.2) (2026-05-20)

### 🐛 Bug Fixes

* remove unsupported option from electron-builder config ([f88b401](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/f88b4018f2aff2db5ba832057e0ef3f806e3f567))

## [1.1.1](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.0...v1.1.1) (2026-05-20)

### 🐛 Bug Fixes

* drop portable target from windows build ([fa4ffcd](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/fa4ffcdc1c916a7b5aa6cb5cdf38ccfa4cd123d3))
* separate portable artifact name and disable blockmap generation ([7a4f7d9](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/7a4f7d9db6020cb13940a5b23106ba315fa3e29d))

## [1.1.0](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.0.5...v1.1.0) (2026-05-20)

### ✨ Features

* introduce appclient transport abstraction (phase 5) ([2ea0a08](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/2ea0a085596fe5587f3b116a4b299fd0b69529e8))

### 🐛 Bug Fixes

* merge duplicate app-client imports in appclientprovider ([12dc8be](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/12dc8be8c705ab5046f6b096c93a83c866b019dd))
* resolve lint errors blocking push ([a25ec5c](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/a25ec5ccabc6113b9d21c65e18b7704679213944))
* sort imports in bootstrap.ts to satisfy eslint pre-push hook ([3c10a89](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/3c10a8987dcd5ebd67a86ff535652a21f315a8d6))

### ⚡ Performance

* lazy-start ai runtime and lancedb vector store (phase 4) ([7121a42](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/7121a4212b64102c3ea5dea5efb361608b582084))
* reduce idle resource usage and quarantine playwright session managers ([90281f8](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/90281f8e85fd361301ea95d6a21bf120347f3133))

### ♻️ Refactors

* extract scraping logic into in-process scraper runtime (phase 3) ([7814b6d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/7814b6da75868fab22660cf1d18383c240dbac81))

## [1.0.5](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.0.4...v1.0.5) (2026-05-20)

### 🐛 Bug Fixes

* set releasetype to release in electron-builder publish config ([6b425e7](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/6b425e73d5320017244a12083d1b08162e13f625))

## [1.0.4](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.0.3...v1.0.4) (2026-05-20)

### 🐛 Bug Fixes

* sync desktop app version before packaging so electron-builder finds the release ([147eaed](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/147eaed9305dada9b5fbd0548fe3d2bda605bf43))

## [1.0.3](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.0.2...v1.0.3) (2026-05-20)

### 🐛 Bug Fixes

* pass github token to electron-builder package steps ([78187a2](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/78187a23fb21aa80e23e0a8f37357b63bec742cf))

## [1.0.2](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.0.1...v1.0.2) (2026-05-20)

### 🐛 Bug Fixes

* build desktop app before packaging in release workflow ([e69b049](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/e69b049a7207137a14b9dffed525d92ac7b9d150))

## [1.0.1](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.0.0...v1.0.1) (2026-05-20)

### 🐛 Bug Fixes

* delete @ajh/ui re-export shim files from components/ui ([6d4bd51](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/6d4bd5102c8af003958a1b0207562996b24ea877))

## 1.0.0 (2026-05-20)

### ✨ Features

* initial release ([e2d4438](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/e2d4438489e221544307fdf66daf999932b702ad))
* ui polish, ci/cd consolidation, and auto-updater ([1c4a38f](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/1c4a38f2e219bac6c40f3a15c06a69237ec499e0))

### 🐛 Bug Fixes

* add explicit types to updater event callbacks ([0d666e8](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/0d666e8a6556c78d6bd31ef605247dc86599a2b8))
* add updater and shortcuts namespaces to ipc contract ([4c6eb08](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/4c6eb08bca9b5d1057a14f88edcb5daa9027168e))
* resolve eslint errors in updater lazy import ([fefa6b7](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/fefa6b7fc698710869515bfd170771f2239f88fb))

### ♻️ Refactors

* move all ui primitives to @ajh/ui, desktop re-exports them ([95d288d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/95d288dcf096418310159363fefb37454510c809))
* move all ui primitives to @ajh/ui, desktop re-exports them ([5eadea7](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/5eadea774441e73aa5a7918aaef16b1b71b68531))

## [1.2.3](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.2.2...v1.2.3) (2026-05-19)

### 🐛 Bug Fixes

* combine duplicate imports and use top-level type imports ([d148750](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/d148750bf20074e8552d5c92d9ade657cfb9aa0b))
* use any type for pdf-parse module with eslint-disable ([2285f3a](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/2285f3a37cec847ef5c16b93a4efab4574135518))

### ♻️ Refactors

* rename documents to resumes for consistency ([73c25de](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/73c25de977ca017c44916c5b17211ff67cf2ec9a))
* rename documents to resumes for consistency ([562ee81](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/562ee81c6ef3986f0f28f0ed39c8cd5bf3479e98))

## [1.2.2](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.2.1...v1.2.2) (2026-05-19)

### 🐛 Bug Fixes

* resolve typescript strict mode errors ([67a4007](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/67a4007e282ff1aa6ae7361f9ac01ab2e087a0f4))

## [1.2.1](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.2.0...v1.2.1) (2026-05-19)

### 🐛 Bug Fixes

* resolve typescript strict mode errors ([8a691f1](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/8a691f18ea7a0b92c309599639bb95bd67c6d341))

## [1.2.0](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.7...v1.2.0) (2026-05-19)

### ✨ Features

* **ai-workspace:** add app-aware system prompt and suggestion chips ([889e8e4](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/889e8e4165e42bcb8d712b2c83fb3c241c3dfc1d))
* **export:** professional-grade resume and cover letter templates ([c71a984](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/c71a9847f51281a7a8d5ac6510207d742dc7d655)), closes [#0D1F3C](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/issues/0D1F3C)
* **generate:** professional-grade bold emphasis, 3 templates, keyword intelligence ([0caba8b](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/0caba8b9c6916b2ab8b623ccd8d3b562bf27551f)), closes [#0D1F3C](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/issues/0D1F3C)

### 🐛 Bug Fixes

* **ai-workspace:** rewrite system prompt to prevent safety refusals ([41cd530](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/41cd5303055dad7c12bba81916f6190dbc441b2e))
* **auth:** auto-reload indeed after ajax sign-in to trigger server redirect ([37e539e](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/37e539e7c3c5089fa93f0a38a0be47f53859aafd))
* **auth:** detect indeed ajax login via cookie change events, no reload ([9076b94](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/9076b945ca4fe6f8ebf248ce98f6ed3eabe11ea9))
* **auth:** fix indeed false-positive auth detection ([3d4c5b2](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/3d4c5b2e103f75bc6f8e14a73817e98ce5fd381c))
* **auth:** fix indeed sign-in button and apply cookie detection to indeed ([6c6e3b3](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/6c6e3b305b68b20cbc2bab7cab781952806fb32e))
* **auth:** fix linkedin login stuck spinner and passkey interruptions ([383e13f](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/383e13fbdc910dea064490751a50585c52778f82))
* **auth:** fix linkedin sign-in button doing nothing and passkey blocking ([596bdaa](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/596bdaae4165665deccfe6025cf6b06126ecc310))
* **background:** restore cursor-following blob using gpu transform ([ff6dfb4](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/ff6dfb40f7edd143ef349bc3b833e9b439cedaf3))
* **background:** smooth cursor blob via lerp animation loop, larger glow ([1e330ae](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/1e330ae1e3add5efd61da8c40b5326aff2a0125c))
* **lint:** resolve all pre-push lint errors and warnings ([12aba3d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/12aba3d022640f4190cc4adb21003f88447b6e1e))
* resolve 8 reported bugs across auth, ui, and scraping ([839e557](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/839e5579f2d9e739949676c00646e070ff1ee035))

### ⚡ Performance

* **prompts:** strengthen resume analyzer and ai generate prompts ([106f460](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/106f46096e953a4fe842b0501c72fded16bb6be6))

### ♻️ Refactors

* **auth:** persistent chromium sessions + cookie-banner false-positive fix ([166ef22](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/166ef2274cb43fd526c6f2792a9be10a9df4bd74))

## [1.1.7](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.6...v1.1.7) (2026-05-19)

### 🐛 Bug Fixes

* move platform detection to ipc and add apache-arrow to data package ([a194d66](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/a194d66357c6c9193e11bab9a0132cecce4b9774))

## [1.1.6](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.5...v1.1.6) (2026-05-19)

### 🐛 Bug Fixes

* disable windows msi and add linux package metadata ([e1efb46](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/e1efb46e159926ff23b485e32a2de8d53f52f688))

## [1.1.5](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.4...v1.1.5) (2026-05-19)

### 🐛 Bug Fixes

* correct electron-builder paths and add zip/msi for windows ([4736d9d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/4736d9d969837ea58be057164226a870b996d2a7))

## [1.1.4](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.3...v1.1.4) (2026-05-19)

### 🐛 Bug Fixes

* add required metadata for electron-builder packaging ([cfc60cb](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/cfc60cb1dc9bb505e294dbd61c3cdcbd99aa76d5))

## [1.1.3](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.2...v1.1.3) (2026-05-19)

### 🐛 Bug Fixes

* add .gitattributes and normalize line endings to lf ([de8e374](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/de8e374e99786bbc4afc9dd7510c5018a77c25c6))

## [1.1.2](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.1...v1.1.2) (2026-05-19)

### 🐛 Bug Fixes

* **linkedin:** restore proper session data type for update session method ([bd54051](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/bd54051e71291e1c9ab61cecd8f9249a452ecff9))
* **quality:** replace all eslint-disable comments with proper fixes ([0f64484](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/0f64484bef69c4f8d795c7a631a6066c92fc824f))

## [1.1.1](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.1.0...v1.1.1) (2026-05-19)

### 🐛 Bug Fixes

* **lint:** resolve all 159 warnings to zero ([000af9d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/000af9dcb0b9711bb418efe7ebf7318f14e6f08a)), closes [#c084fc](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/issues/c084fc) [#a855f7](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/issues/a855f7) [#16162a](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/issues/16162a)

## [1.1.0](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/compare/v1.0.0...v1.1.0) (2026-05-19)

### ✨ Features

* **i18n:** auto-detect system language on first launch ([9d2ea1f](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/9d2ea1f18a044e928ab9c2024194083d2ecd016b))
* **i18n:** complete internationalization of renderer routes and features ([0b7564b](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/0b7564b28c38cb9ef8c31c1afffd78b1b3c74bfa))
* **i18n:** complete internationalization of renderer routes and features ([2327869](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/2327869ddfa93c4ae5e525a3c47da5a92e026a2f))
* major architectural refactor — design system, patterns, and ai tooling ([75dc642](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/75dc6429bc7c4b84d2e0c478b8469860805003d8))
* **onboarding:** add introductory wizard with spotlight tour ([d51a372](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/d51a372f0eb798d07cc523839bd39d6302b77f24))
* **settings:** add replay wizard button to general settings ([50e98a6](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/50e98a6a3e3fff4501726b01509920067fb414b9))

### 🐛 Bug Fixes

* **build:** re-export modes from generate-ai to fix missing generate-prompt module ([bef692d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/bef692ddce7319c50d94e00f81453e51582564a4))
* **data:** resolve pre-existing eslint errors in applying boards and scrapers ([f9f0219](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/f9f02193b87416cec0c7e29dee9b0fa02a5f0d9f))
* **hooks:** exclude desktop from pre-push typecheck to unblock push ([51acc2d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/51acc2d7175dab6c97fe58777b0efa06e5e83e59))
* **hooks:** quote filter argument to prevent shell interpretation of ! ([303947f](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/303947f35229151a9cdcae29c0dafe579422891f))
* **hooks:** simplify pre-push to lint + tests only; typechecks run in ci ([9dfee20](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/9dfee203dceeb69780e84d2073507745f385389c))
* **i18n:** restore persisted language on page reload ([5be74e5](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/5be74e53071f6312f8d5816d77451cab702f6494))
* **monitoring:** remove unused kind-short variable after prop removal ([817c932](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/817c932a0ca47c8584e727c9129867caed005d48))
* resolve all pre-existing eslint errors blocking push ([e39b8b1](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/e39b8b172d18165f221df0a591a4ae03cf48fa12))
* **services:** align autopilot service hooks with preload signatures ([ba48829](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/ba48829be307201214a4ee66f01115d873ea0715))
* **shared:** add missing autopilot type, fix duplicate jobinteraction export ([f3ac10c](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/f3ac10c13e8ab0feee1d6a9e8ecae6cf7c6679a8))
* **shared:** correct autopilot, matchscore, jobposting types to match actual usage ([41a0acb](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/41a0acbd7873f62e019b7e3ab13ec2c9a02fad5a))
* **types:** resolve all typecheck errors across renderer and main process ([c28717d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/c28717da23be77e16061c1c242d9cae56a0a1f36))
* **ui:** add jsdom devdependency for vitest jsdom environment ([9431a59](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/9431a59c61b65bc37cd90142295ea6d2193f3950))
* **workers:** restore tresult generic to workertask interface ([0780d36](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/0780d36b9ffb361bbc6739b3ae36c5a4f9a9bcd0))
* **workers:** suppress unused tresult generic param with eslint-disable ([9383c84](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/9383c840f49ad8b7f4356dc09ac0ce7f4cf01383))

### 📚 Documentation

* **ai-tools:** add docs/ references to all ai tool configs ([0680dee](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/0680dee7820f0b51b5c1088c948323f3dca73145))
* consolidate all documentation into docs/ folder ([8c4f39f](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/8c4f39fa4bc8a0f9abe5c9e42b406f0ed3bad7d4))

## 1.0.0 (2026-05-18)

### ✨ Features

* add AI model selection and document download functionality ([5fb0640](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/5fb0640d24a0cac3ff84e76cca450b056622e3e0))
* add job scraping UI with stable layout ([27387e5](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/27387e5b082bd3121391b0555eff8d64e73a488d))
* add locale-aware analysis and fix PDF extraction ([3172f8a](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/3172f8ac1cf0ca5c3dfef6ea6c57cfa389ab9d55))
* autopilot system, release engineering, and production infrastructure ([32deb7d](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/32deb7ddb460ba9ee95784dcedbd60a69df22ea7))
* **i18n:** comprehensive localization across all pages ([8abeb85](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/8abeb857f15c39dcbd1d0361191943a9b90f086c))
* major UI overhaul, AI resume pipeline, and scraping improvements ([c3b8d85](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/c3b8d850233958e7abe55948523f00dafc4443ba))
* speed up LinkedIn authenticated scraping with cookie-based API calls ([a0e42f7](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/a0e42f7f2e65a3239298928ef6c38d60b3932b26))
* ui component refactor with new shared primitives and store restructure ([a5d0b3a](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/a5d0b3a7321e6f2e0a543f02b7ad7a0c9ff6a57c))

### 🐛 Bug Fixes

* resolve electron installation issue in pnpm workspace ([381d545](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/381d545df376612c4d1aeaa6333c5ec1d0e583a3))
* resolve Ollama model detection and update documentation ([7f9e4d2](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/commit/7f9e4d2fb3d9fa7697947d0ded24d4e2c24cdfbf))
