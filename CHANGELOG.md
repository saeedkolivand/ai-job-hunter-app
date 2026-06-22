# Changelog

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
