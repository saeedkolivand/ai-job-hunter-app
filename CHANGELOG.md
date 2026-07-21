# Changelog

## [0.128.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.127.0...v0.128.0) (2026-07-21)

### ✨ Features

* add jobicy keyless remote-jobs board with full descriptions ([#700](https://github.com/saeedkolivand/ai-job-hunter-app/issues/700)) ([1a7559a](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1a7559a48f528ba4e7b5f1e7dc7c6be698e6e9db))
* **ai:** persist url import provenance and harvest from resolves (adr-031) ([#761](https://github.com/saeedkolivand/ai-job-hunter-app/issues/761)) ([420ba60](https://github.com/saeedkolivand/ai-job-hunter-app/commit/420ba60427f15bba3a86e323799bd871fdd84ebb))
* **extension:** match autofill fields labeled in major eu languages ([#738](https://github.com/saeedkolivand/ai-job-hunter-app/issues/738)) ([1e9d466](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1e9d466df09fc2c4d2523dd6c855a3f401a95dc8))
* landing docs tier — docshell, mission control, agent-system route, next 16 ([#742](https://github.com/saeedkolivand/ai-job-hunter-app/issues/742)) ([774468e](https://github.com/saeedkolivand/ai-job-hunter-app/commit/774468eb5a736e37134117a734920c093f585ccb))
* landing gl p4 - deep fried set-piece and godmode rise ([#708](https://github.com/saeedkolivand/ai-job-hunter-app/issues/708)) ([16b2994](https://github.com/saeedkolivand/ai-job-hunter-app/commit/16b2994656a0601bf0fed6fb3b2d331ef003bd8b))
* landing gl takeover p0 - app skeleton + semantic parity port ([#702](https://github.com/saeedkolivand/ai-job-hunter-app/issues/702)) ([03a4ac6](https://github.com/saeedkolivand/ai-job-hunter-app/commit/03a4ac6bdfbee0506b81f9ae4bb039c390d0b36d))
* landing gl takeover p1 - engine spine ([#703](https://github.com/saeedkolivand/ai-job-hunter-app/issues/703)) ([46d1a44](https://github.com/saeedkolivand/ai-job-hunter-app/commit/46d1a44e0b0210a63978ee59e628d27d374f477b))
* landing gl takeover p2 - ink strokes, self-hosted fonts, gl text ([#705](https://github.com/saeedkolivand/ai-job-hunter-app/issues/705)) ([10533df](https://github.com/saeedkolivand/ai-job-hunter-app/commit/10533df95c6d7568517bcbeaae08a28437cd44db))
* landing gl takeover p3 - slump and descent beats ([#706](https://github.com/saeedkolivand/ai-job-hunter-app/issues/706)) ([e80cdb8](https://github.com/saeedkolivand/ai-job-hunter-app/commit/e80cdb8658242cb304b6bce8963a3d9b5cb7249c))
* landing gl takeover p5 - the skeleton completes ([#709](https://github.com/saeedkolivand/ai-job-hunter-app/issues/709)) ([66c36c6](https://github.com/saeedkolivand/ai-job-hunter-app/commit/66c36c68899974446454a6c2e7766c2cfa4c14ad))
* migrate landing to next.js static export with real routes ([#740](https://github.com/saeedkolivand/ai-job-hunter-app/issues/740)) ([408d0dc](https://github.com/saeedkolivand/ai-job-hunter-app/commit/408d0dc30eebd6fc308d64705ccc78604ad80bbf))
* nightly metrics snapshot data plane for mission control ([#759](https://github.com/saeedkolivand/ai-job-hunter-app/issues/759)) ([4f6684b](https://github.com/saeedkolivand/ai-job-hunter-app/commit/4f6684b15ce52dc2b6c34ff112aef42e205b52af))
* ripbook m1 - notebook shell and rip rig proof ([#713](https://github.com/saeedkolivand/ai-job-hunter-app/issues/713)) ([f6e0936](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f6e0936bab077dc82a88405de9c11856703de54b))
* ripbook m2 - kraft paper material system ([#714](https://github.com/saeedkolivand/ai-job-hunter-app/issues/714)) ([d833486](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d833486463d506b4e9ae5a085800ff95bea899d4))
* **scraping:** cluster cross-board duplicate jobs with source chips and split (adr-029) ([#756](https://github.com/saeedkolivand/ai-job-hunter-app/issues/756)) ([d435455](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d4354552e572d4d862781f654abea1f36504fe61))
* **scraping:** harvest ats slugs passively with watched-company autopilot targets (adr-030) ([#760](https://github.com/saeedkolivand/ai-job-hunter-app/issues/760)) ([ccaddb3](https://github.com/saeedkolivand/ai-job-hunter-app/commit/ccaddb3a6b750e43ca05c5d6bd2920e32237d0e2))
* terminal velocity m1 — scroll rig, playhead, semantic layer parity ([#720](https://github.com/saeedkolivand/ai-job-hunter-app/issues/720)) ([3db3540](https://github.com/saeedkolivand/ai-job-hunter-app/commit/3db3540183e39862bd674ee4d9eb0caa31f4203d))
* terminal velocity m2 — tower canyon and paper storm ([#721](https://github.com/saeedkolivand/ai-job-hunter-app/issues/721)) ([407559c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/407559cf88f4dbc1241aac662e865d15f614b7db))
* terminal velocity m3 — water, splash vat, deep and blackout ([#722](https://github.com/saeedkolivand/ai-job-hunter-app/issues/722)) ([bd87e89](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bd87e89bf0ab77bf85f88371e82a6fce27087d88))
* use dom_smoothie readability pass in scrape_url generic extraction fallback ([#699](https://github.com/saeedkolivand/ai-job-hunter-app/issues/699)) ([026baa5](https://github.com/saeedkolivand/ai-job-hunter-app/commit/026baa51d7e4c5588c5b229d702c9ac438b311d3))

### 🐛 Bug Fixes

* **analyze:** drop false language mismatch when a language is empty string ([#731](https://github.com/saeedkolivand/ai-job-hunter-app/issues/731)) (@thejesh23) ([d46fac5](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d46fac53134620f3d1a3677eb64f4a89fc8232dd))
* **applications:** standalone apply-by-email generation and scrollable jobs scrape form ([#736](https://github.com/saeedkolivand/ai-job-hunter-app/issues/736)) ([b8d4f22](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b8d4f227cb06a1bc2a682471521f58e4ae9135bc))
* **export:** keep applicant name out of recipient block and prefer profile casing for signature ([#735](https://github.com/saeedkolivand/ai-job-hunter-app/issues/735)) ([921e723](https://github.com/saeedkolivand/ai-job-hunter-app/commit/921e7238af6e768d531746b9b269b79ca50b5cd4))
* **export:** keep Unicode letters in exported filenames ([#729](https://github.com/saeedkolivand/ai-job-hunter-app/issues/729)) (@thejesh23) ([3a3c9fa](https://github.com/saeedkolivand/ai-job-hunter-app/commit/3a3c9fa073a0fcc51169f436ffd9a9176cf49681))
* **extension:** parse linkedin job ad from captured page dom instead of authwalled refetch ([#741](https://github.com/saeedkolivand/ai-job-hunter-app/issues/741)) ([b91a5d1](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b91a5d1cae12cfc3c6577582694ae24089dd3c79))
* hold the gl experience behind a pre-launch gate until the production flip ([#707](https://github.com/saeedkolivand/ai-job-hunter-app/issues/707)) ([8b42603](https://github.com/saeedkolivand/ai-job-hunter-app/commit/8b426036414e1307d4ba1706a9f44b81501ee507))
* **jobs:** post-756 review findings and ai review sticky restyle ([#758](https://github.com/saeedkolivand/ai-job-hunter-app/issues/758)) ([a023089](https://github.com/saeedkolivand/ai-job-hunter-app/commit/a0230890dfbfe85f8444fe1a9554cc734ea19d59)), closes [Post-#756](https://github.com/saeedkolivand/Post-/issues/756)
* **language-detection:** return 'unknown' for codes outside the map ([#727](https://github.com/saeedkolivand/ai-job-hunter-app/issues/727)) (@thejesh23) ([bf2aab1](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bf2aab12f26597d1ee6939b765e138d610ad275c))
* **links:** capture markdown urls containing balanced parens ([#733](https://github.com/saeedkolivand/ai-job-hunter-app/issues/733)) (@thejesh23) ([0879044](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0879044006430dedc248f0995b964f43aa27fc98))
* **prompts:** omit company lines instead of emitting placeholders when company unknown ([#734](https://github.com/saeedkolivand/ai-job-hunter-app/issues/734)) ([c0ff9cb](https://github.com/saeedkolivand/ai-job-hunter-app/commit/c0ff9cbe8ae1835bfe564d0cfe7284b313988619))
* **referral:** pass iso code instead of display name to locale ([#725](https://github.com/saeedkolivand/ai-job-hunter-app/issues/725)) (@thejesh23) ([d37aff2](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d37aff25746cf7d5f5953bd6792e44be66e996b5))

### ♻️ Refactors

* consolidate static landing into apps/landing and retire the scroll-film app ([#737](https://github.com/saeedkolivand/ai-job-hunter-app/issues/737)) ([4faf353](https://github.com/saeedkolivand/ai-job-hunter-app/commit/4faf353f1725bc844293ec7e8b3148d418e2209f))
* port architecture-map passthrough to typed-data next route ([#757](https://github.com/saeedkolivand/ai-job-hunter-app/issues/757)) ([be4477c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/be4477ccab00aea9718984eb9606bdbb8f09839b))

### 📚 Documentation

* add the critic contract and seed the miss ledger ([#716](https://github.com/saeedkolivand/ai-job-hunter-app/issues/716)) ([bf59ca3](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bf59ca3516903f543a188f16b217be847b37ab1a))
* adopt the ripbook notebook landing plan ([#711](https://github.com/saeedkolivand/ai-job-hunter-app/issues/711)) ([3019ca0](https://github.com/saeedkolivand/ai-job-hunter-app/commit/3019ca04e8791cf66db25b80a670f55b0807eddc))
* adr-0016 — terminal velocity scroll-film supersedes ripbook ([#718](https://github.com/saeedkolivand/ai-job-hunter-app/issues/718)) ([332aa7a](https://github.com/saeedkolivand/ai-job-hunter-app/commit/332aa7a0e79af2c0bd7f15a01848535d33ba12e2))
* bump board count to 24 and document the jobicy board ([#701](https://github.com/saeedkolivand/ai-job-hunter-app/issues/701)) ([add0d7e](https://github.com/saeedkolivand/ai-job-hunter-app/commit/add0d7e012b6b7999abb85576ea7724e75a9b577)), closes [698-#700](https://github.com/saeedkolivand/698-/issues/700) [#700](https://github.com/saeedkolivand/ai-job-hunter-app/issues/700)
* cache-bust the contributors image so new contributors show ([a5a9322](https://github.com/saeedkolivand/ai-job-hunter-app/commit/a5a93229f1a685444366ff73864efc382d88581a))
* dedupe fleet map critics and de-concept gl agent descriptions ([#723](https://github.com/saeedkolivand/ai-job-hunter-app/issues/723)) ([e6fb785](https://github.com/saeedkolivand/ai-job-hunter-app/commit/e6fb7853e7d5c7580ad34234ebde7a87e39369bd))
* refresh readme + landing (12 templates, 23 boards, http linkedin) and unify footers ([#698](https://github.com/saeedkolivand/ai-job-hunter-app/issues/698)) ([289dedc](https://github.com/saeedkolivand/ai-job-hunter-app/commit/289dedcdebd28738f4bd83a5fae8bf4dc28cfbb3))
* rewrite webgl-standards and gate-audit skills for terminal velocity ([#719](https://github.com/saeedkolivand/ai-job-hunter-app/issues/719)) ([562b582](https://github.com/saeedkolivand/ai-job-hunter-app/commit/562b58203cff92be500c0f96bb740089a5654752))

## [0.127.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.126.0...v0.127.0) (2026-07-17)

### ✨ Features

* add ai answer rewrite mode to the extension ([#649](https://github.com/saeedkolivand/ai-job-hunter-app/issues/649)) ([b416749](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b416749752d8dce75269940c668032a33873f428))
* add applied.check bridge verb with adaptive popup status ([#631](https://github.com/saeedkolivand/ai-job-hunter-app/issues/631)) ([6399447](https://github.com/saeedkolivand/ai-job-hunter-app/commit/63994473b965ebeeb351474862737e769441dc38))
* add match.live check fit scoring and import match scores ([#641](https://github.com/saeedkolivand/ai-job-hunter-app/issues/641)) ([bd87ac0](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bd87ac06333cfb34e49fca2735ba9b5ec352cc33)), closes [#fragment](https://github.com/saeedkolivand/ai-job-hunter-app/issues/fragment)
* add status.update bridge verb for one-click mark as applied ([#632](https://github.com/saeedkolivand/ai-job-hunter-app/issues/632)) ([4580e11](https://github.com/saeedkolivand/ai-job-hunter-app/commit/4580e112e58075ad5614654024d0bb18aa343848))
* auto-suggest answers on popup open and offer the saved salary expectation ([#695](https://github.com/saeedkolivand/ai-job-hunter-app/issues/695)) ([89d51ee](https://github.com/saeedkolivand/ai-job-hunter-app/commit/89d51ee45560ad9e0db59e742ba2fca2421a772e))
* auto-track sent applications on a detected form submit (opt-in) ([#687](https://github.com/saeedkolivand/ai-job-hunter-app/issues/687)) ([798c82f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/798c82fefe5ecaddabb54f5d222eb7052e078f16))
* capture filled application answers from the extension ([#636](https://github.com/saeedkolivand/ai-job-hunter-app/issues/636)) ([c25c2b8](https://github.com/saeedkolivand/ai-job-hunter-app/commit/c25c2b813ab2fa78badcda3562b236c5c74e7d2a))
* connect gmail for email-confirmation watching (auto-track layer c, foundation) ([#689](https://github.com/saeedkolivand/ai-job-hunter-app/issues/689)) ([7de160e](https://github.com/saeedkolivand/ai-job-hunter-app/commit/7de160e10f457b19a83d89575ac61733ee985299)), closes [#23](https://github.com/saeedkolivand/ai-job-hunter-app/issues/23) [#23](https://github.com/saeedkolivand/ai-job-hunter-app/issues/23)
* draft application answers from the extension behind a new opt-in ([#643](https://github.com/saeedkolivand/ai-job-hunter-app/issues/643)) ([d6d820f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d6d820f01f7c2f7dc82cdbdf95868e24c15ada92))
* fill portfolio and custom link fields from contact profile extra links ([#634](https://github.com/saeedkolivand/ai-job-hunter-app/issues/634)) ([d267ad8](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d267ad87bd65cd7099697025d3c3f32a138d20fd))
* prefer extension job-root hint in generic page parsing ([#633](https://github.com/saeedkolivand/ai-job-hunter-app/issues/633)) ([27a5d73](https://github.com/saeedkolivand/ai-job-hunter-app/commit/27a5d732ae9625a6b50dce537d90005ff4db2ecc))
* source the settings changelog from the bundled changelog file ([#640](https://github.com/saeedkolivand/ai-job-hunter-app/issues/640)) ([abf8f2b](https://github.com/saeedkolivand/ai-job-hunter-app/commit/abf8f2bdbe5b161ceb35530319107922fe25e842))
* stream extension answer drafts over the bridge ([#646](https://github.com/saeedkolivand/ai-job-hunter-app/issues/646)) ([9985ff6](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9985ff6136dd01afa52417b6b4a97fa376e19369))
* suggest saved answers for application form questions ([#637](https://github.com/saeedkolivand/ai-job-hunter-app/issues/637)) ([d888929](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d888929429b677e7d25c06ecdc26aabfec883fc7))
* update dependencies with 24h cooldown and adopt typescript 7 ([#638](https://github.com/saeedkolivand/ai-job-hunter-app/issues/638)) ([c0753b3](https://github.com/saeedkolivand/ai-job-hunter-app/commit/c0753b3b270bedb09face681f51dd2894f340117))
* watch the inbox for application confirmations and notify to confirm (auto-track layer c) ([#696](https://github.com/saeedkolivand/ai-job-hunter-app/issues/696)) ([d2f6fb2](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d2f6fb2040202e1e3598c559540e6bf03c5398de)), closes [#23](https://github.com/saeedkolivand/ai-job-hunter-app/issues/23)

### 🐛 Bug Fixes

* **analyze:** don't flag language mismatch when a language is unknown ([#680](https://github.com/saeedkolivand/ai-job-hunter-app/issues/680)) (@thejesh23) ([1bb7474](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1bb747438d159b9b35043dd4734db687c21236ed)), closes [#677](https://github.com/saeedkolivand/ai-job-hunter-app/issues/677)
* bring hmac keyinit trait into scope for the 0.13 api ([#674](https://github.com/saeedkolivand/ai-job-hunter-app/issues/674)) ([2ec7a8c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/2ec7a8cfcbe3851283bcd3f2d1b157c94afd2c74)), closes [#672](https://github.com/saeedkolivand/ai-job-hunter-app/issues/672) [#669](https://github.com/saeedkolivand/ai-job-hunter-app/issues/669) [#670](https://github.com/saeedkolivand/ai-job-hunter-app/issues/670)
* contain model selector within its panel row ([#635](https://github.com/saeedkolivand/ai-job-hunter-app/issues/635)) ([d191951](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d191951d52d1d4fc2fbdc4b5c3e6e15c8c8a1b39))
* harden the extension bridge answer-assist streaming transport ([#648](https://github.com/saeedkolivand/ai-job-hunter-app/issues/648)) ([1adfc42](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1adfc4286e42f90cfa810c50e226f12fcd95fcc3))
* own the active ai provider config in the backend so the renderer can't set base_url ([#682](https://github.com/saeedkolivand/ai-job-hunter-app/issues/682)) ([b9230da](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b9230da4ca21f546926c14c4e06b01f549cbd35c)), closes [#16](https://github.com/saeedkolivand/ai-job-hunter-app/issues/16) [#5](https://github.com/saeedkolivand/ai-job-hunter-app/issues/5) [#16](https://github.com/saeedkolivand/ai-job-hunter-app/issues/16)
* **prompts:** count zero words for empty or whitespace-only resume ([#681](https://github.com/saeedkolivand/ai-job-hunter-app/issues/681)) (@thejesh23) ([014dead](https://github.com/saeedkolivand/ai-job-hunter-app/commit/014dead79eb75876f42e050ee6e61303ad01cf52)), closes [#678](https://github.com/saeedkolivand/ai-job-hunter-app/issues/678)
* repair main after dependabot merges (ts7 lint pin + aes-gcm deprecation) ([#662](https://github.com/saeedkolivand/ai-job-hunter-app/issues/662)) ([f58216f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f58216f23e12771254f7884a3b0329d0d1321ec5)), closes [#660](https://github.com/saeedkolivand/ai-job-hunter-app/issues/660) [#14](https://github.com/saeedkolivand/ai-job-hunter-app/issues/14) [#659](https://github.com/saeedkolivand/ai-job-hunter-app/issues/659)
* resolve the agent_run provider from the backend store, not the renderer request ([#685](https://github.com/saeedkolivand/ai-job-hunter-app/issues/685)) ([9eb2e75](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9eb2e75ade9434797b405e5f75a4f4ad07d5d070)), closes [#5](https://github.com/saeedkolivand/ai-job-hunter-app/issues/5) [#16](https://github.com/saeedkolivand/ai-job-hunter-app/issues/16) [#25](https://github.com/saeedkolivand/ai-job-hunter-app/issues/25) [#25](https://github.com/saeedkolivand/ai-job-hunter-app/issues/25) [#5](https://github.com/saeedkolivand/ai-job-hunter-app/issues/5)
* restore rewrite-mode review fixes lost in [#649](https://github.com/saeedkolivand/ai-job-hunter-app/issues/649) ([#675](https://github.com/saeedkolivand/ai-job-hunter-app/issues/675)) ([9bf24aa](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9bf24aa0e11dca454dcbe8bf3715d289a44fc349))
* **shared:** map swedish to iso 639-1 'sv' not iso 639-3 'swe' ([#679](https://github.com/saeedkolivand/ai-job-hunter-app/issues/679)) (@thejesh23) ([0459ea2](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0459ea21f5e12125032f59d6f78f9d2fbbb3f97f)), closes [#676](https://github.com/saeedkolivand/ai-job-hunter-app/issues/676)
* surface matched application status in extension import popup ([#630](https://github.com/saeedkolivand/ai-job-hunter-app/issues/630)) ([fba0211](https://github.com/saeedkolivand/ai-job-hunter-app/commit/fba02111d4ae669d691ff2ad99acadaf1e106feb)), closes [#btn-import](https://github.com/saeedkolivand/ai-job-hunter-app/issues/btn-import)
* track bridge connections with a refcount and push status changes to settings ([#693](https://github.com/saeedkolivand/ai-job-hunter-app/issues/693)) ([b6281d6](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b6281d6d84d9f36b24f4de514961799ac43232e0))

### 🎨 UI/UX

* regroup the extension popup by task with a context probe and collapsed answer tools ([#690](https://github.com/saeedkolivand/ai-job-hunter-app/issues/690)) ([a9c29d9](https://github.com/saeedkolivand/ai-job-hunter-app/commit/a9c29d97c135997de18232abee019687f7820fb1)), closes [#rewrite-preset](https://github.com/saeedkolivand/ai-job-hunter-app/issues/rewrite-preset)

### 📚 Documentation

* close out the extension features roadmap ([#647](https://github.com/saeedkolivand/ai-job-hunter-app/issues/647)) ([eb8c25f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/eb8c25f13a15accf57f56a2c5622f61cbfbc67cf))
* fix contributing release process and merge duplicate sections ([#686](https://github.com/saeedkolivand/ai-job-hunter-app/issues/686)) ([fd08935](https://github.com/saeedkolivand/ai-job-hunter-app/commit/fd08935e1eff89e233565350f84b5e4538528351))
* fix drifted directory paths, add apps/extension listings, narrow CI path filter ([#683](https://github.com/saeedkolivand/ai-job-hunter-app/issues/683)) ([8161de3](https://github.com/saeedkolivand/ai-job-hunter-app/commit/8161de30ee0327bae20cfe5c7ef3004a5fed899d))
* fix stale apps/tauri trees, app-data dir name, and ci badge ([#684](https://github.com/saeedkolivand/ai-job-hunter-app/issues/684)) ([09a1458](https://github.com/saeedkolivand/ai-job-hunter-app/commit/09a1458895a88490c38a84ecbc1fb54e1448e71a))
* refresh the status tracker, de-drift the knowledge base, retire finished audit worklists ([#692](https://github.com/saeedkolivand/ai-job-hunter-app/issues/692)) ([4abedd2](https://github.com/saeedkolivand/ai-job-hunter-app/commit/4abedd27d8b5dddb5a65cefa39d44841d6858ed6)), closes [#548](https://github.com/saeedkolivand/ai-job-hunter-app/issues/548) [#549](https://github.com/saeedkolivand/ai-job-hunter-app/issues/549) [#551](https://github.com/saeedkolivand/ai-job-hunter-app/issues/551) [#623](https://github.com/saeedkolivand/ai-job-hunter-app/issues/623) [#563](https://github.com/saeedkolivand/ai-job-hunter-app/issues/563) [#590-594](https://github.com/saeedkolivand/ai-job-hunter-app/issues/590-594) [#687](https://github.com/saeedkolivand/ai-job-hunter-app/issues/687) [#689](https://github.com/saeedkolivand/ai-job-hunter-app/issues/689) [#499](https://github.com/saeedkolivand/ai-job-hunter-app/issues/499) [#618](https://github.com/saeedkolivand/ai-job-hunter-app/issues/618)

## [0.126.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.125.0...v0.126.0) (2026-07-13)

### ✨ Features

* add detector-resistant sampling params for prose generation ([#615](https://github.com/saeedkolivand/ai-job-hunter-app/issues/615)) ([d6102dd](https://github.com/saeedkolivand/ai-job-hunter-app/commit/d6102dd54b2a8b52b810e7ce6d8cef22905e317a))
* add jooble as a byo-key aggregator fallback provider ([#618](https://github.com/saeedkolivand/ai-job-hunter-app/issues/618)) ([7e4c72f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/7e4c72f943fcabb3be6154b234c9fd4401c6fc73)), closes [597-#604](https://github.com/saeedkolivand/597-/issues/604)
* ai spend visibility — real per-provider token + estimated cost tracking ([#624](https://github.com/saeedkolivand/ai-job-hunter-app/issues/624)) ([9f93f42](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9f93f42db49beea10f9c5a18930bbb3e400b12ae))
* assisted autofill for application forms from contact profile ([#625](https://github.com/saeedkolivand/ai-job-hunter-app/issues/625)) ([49ba3e6](https://github.com/saeedkolivand/ai-job-hunter-app/commit/49ba3e60e8bdd80be68ece395234a6f5e062ab87))
* bundle a verified company-to-ats-slug seed directory (data only) ([#620](https://github.com/saeedkolivand/ai-job-hunter-app/issues/620)) ([ab54a80](https://github.com/saeedkolivand/ai-job-hunter-app/commit/ab54a8061ff2908698beb181f058b0299088b7d0))
* interview practice mode (mock questions + star answer feedback) ([#623](https://github.com/saeedkolivand/ai-job-hunter-app/issues/623)) ([65bf385](https://github.com/saeedkolivand/ai-job-hunter-app/commit/65bf38502404c3e1c68b0e7167c2731846368f1f)), closes [#617](https://github.com/saeedkolivand/ai-job-hunter-app/issues/617)
* language-aware anti-ai-tell voice + résumé style transfer for prose ([#616](https://github.com/saeedkolivand/ai-job-hunter-app/issues/616)) ([5ddb3d9](https://github.com/saeedkolivand/ai-job-hunter-app/commit/5ddb3d9b50236d5458bb1c9e393b44cce8535528))
* mutual hmac handshake for the extension bridge (protocol v2) ([#627](https://github.com/saeedkolivand/ai-job-hunter-app/issues/627)) ([548f841](https://github.com/saeedkolivand/ai-job-hunter-app/commit/548f84196f4ee6e493e04059cab19faa223897c4))
* route the ats seed into company-scoped boards (engine-side) — DO NOT MERGE until disclosure decision ([#621](https://github.com/saeedkolivand/ai-job-hunter-app/issues/621)) ([0dd532b](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0dd532b51af0e1c62206a56b8a49c8ccd6dcfcf3))

### 🐛 Bug Fixes

* derive multi-board batch cap from the scraper registry so selecting all boards works ([#629](https://github.com/saeedkolivand/ai-job-hunter-app/issues/629)) ([3e2f325](https://github.com/saeedkolivand/ai-job-hunter-app/commit/3e2f32549a4b8eefdf30a11411240c942ea9e0b7))
* fence the scraped job ad as untrusted input in all prompt builders ([#617](https://github.com/saeedkolivand/ai-job-hunter-app/issues/617)) ([bac6c0a](https://github.com/saeedkolivand/ai-job-hunter-app/commit/bac6c0a6992f7b099e66073993319dde59bd69bf))
* parse jooble timezone-less timestamps instead of dropping posted_at ([#619](https://github.com/saeedkolivand/ai-job-hunter-app/issues/619)) ([c6b8b85](https://github.com/saeedkolivand/ai-job-hunter-app/commit/c6b8b85d5037c005772a305691c2f6bfb79b6353))
* remove dead work-type stub, wire monitoring i18n, correct landing license claim ([#614](https://github.com/saeedkolivand/ai-job-hunter-app/issues/614)) ([6c7a9e0](https://github.com/saeedkolivand/ai-job-hunter-app/commit/6c7a9e050320aefc12d143c64027c4c201d8ac53))

### 📚 Documentation

* fix architecture drift (rusqlite not drizzle, semantic scoring not hybrid search) ([#628](https://github.com/saeedkolivand/ai-job-hunter-app/issues/628)) ([b17d12f](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b17d12f4498387b41254d6add95eb53ca445a8fd))
* sync scraping docs for merged jooble + ats seed work ([#622](https://github.com/saeedkolivand/ai-job-hunter-app/issues/622)) ([9760d38](https://github.com/saeedkolivand/ai-job-hunter-app/commit/9760d38e7bdeb0779964a919299f48217dc1df74)), closes [#618](https://github.com/saeedkolivand/ai-job-hunter-app/issues/618) [#619](https://github.com/saeedkolivand/ai-job-hunter-app/issues/619) [#620](https://github.com/saeedkolivand/ai-job-hunter-app/issues/620) [#621](https://github.com/saeedkolivand/ai-job-hunter-app/issues/621) [#621](https://github.com/saeedkolivand/ai-job-hunter-app/issues/621)

## [0.125.0](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.124.2...v0.125.0) (2026-07-11)

### ✨ Features

* aria and saffron photo templates with per-template placement (templates pr 4/6) ([#593](https://github.com/saeedkolivand/ai-job-hunter-app/issues/593)) ([4a5d18c](https://github.com/saeedkolivand/ai-job-hunter-app/commit/4a5d18c955c270d0bf272fb33acfc612b0c0b6ff))
* cadence and regent single-column templates (templates pr 3/6) ([#592](https://github.com/saeedkolivand/ai-job-hunter-app/issues/592)) ([1172aff](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1172aff7a72ebea50954e612846f88cc113b9137))
* canonical location model with honest per-board location handling ([#602](https://github.com/saeedkolivand/ai-job-hunter-app/issues/602)) ([7abc20e](https://github.com/saeedkolivand/ai-job-hunter-app/commit/7abc20e2f80596c4b5247b93dfe8ae458abf550c))
* collapse cross-source duplicate jobs behind one canonical key ([#601](https://github.com/saeedkolivand/ai-job-hunter-app/issues/601)) ([0b13fc3](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0b13fc3bb44286b8941ca84baba8563b04c21b64))
* make location broadening and guessed markets visible ([#600](https://github.com/saeedkolivand/ai-job-hunter-app/issues/600)) ([b5e9c38](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b5e9c387282e6c41bfb21e6a46ea85e0c4ce0c3b))
* merge modern into classic and add per-export document accent (templates pr 1/6) ([#590](https://github.com/saeedkolivand/ai-job-hunter-app/issues/590)) ([0eda637](https://github.com/saeedkolivand/ai-job-hunter-app/commit/0eda637072557b64c13e35dc5db54f828ab12785))
* record honest autopilot run status with per-board summaries ([#598](https://github.com/saeedkolivand/ai-job-hunter-app/issues/598)) ([dcd5081](https://github.com/saeedkolivand/ai-job-hunter-app/commit/dcd50817f698a04d314647e85f114c699ec27d41))
* scheduler retry with honest scores and partial-visibility notes ([#604](https://github.com/saeedkolivand/ai-job-hunter-app/issues/604)) ([75ba0f5](https://github.com/saeedkolivand/ai-job-hunter-app/commit/75ba0f5f75e960a2d1b5783c8e414ba246e9470f))
* selectable letter layouts inheriting the resume template palette (templates pr 5/6) ([#594](https://github.com/saeedkolivand/ai-job-hunter-app/issues/594)) ([1adb050](https://github.com/saeedkolivand/ai-job-hunter-app/commit/1adb050e7827640f636470e2efcc624bf2581b8e))
* surface per-board scrape diagnostics in jobs and autopilot views ([#599](https://github.com/saeedkolivand/ai-job-hunter-app/issues/599)) ([f47851b](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f47851b803de050f13612e678ea8e2f135ffd484))
* template tiers with grouped gallery and lebenslauf ats toggle fix (templates pr 2/6) ([#591](https://github.com/saeedkolivand/ai-job-hunter-app/issues/591)) ([256ee72](https://github.com/saeedkolivand/ai-job-hunter-app/commit/256ee72fb933ccb95a9ca05d0804094a93a25905))

### 🐛 Bug Fixes

* classic cover letters render the recipient address block again ([#596](https://github.com/saeedkolivand/ai-job-hunter-app/issues/596)) ([f060c8d](https://github.com/saeedkolivand/ai-job-hunter-app/commit/f060c8de1f15070bd40a0a8959712ed345a526cb))
* job-search trust quick wins (silent failures, location determinism, dedupe) ([#589](https://github.com/saeedkolivand/ai-job-hunter-app/issues/589)) ([5cf5bdd](https://github.com/saeedkolivand/ai-job-hunter-app/commit/5cf5bdd5ae610c9320251fc7d34b780bc2ba7818))
* live-verified board hygiene with honest failure reasons ([#603](https://github.com/saeedkolivand/ai-job-hunter-app/issues/603)) ([8b1a167](https://github.com/saeedkolivand/ai-job-hunter-app/commit/8b1a16715b0e41117ef397f87cc11e4a6c00f347))
* represent board fetch failures instead of silent zero results ([#597](https://github.com/saeedkolivand/ai-job-hunter-app/issues/597)) ([4245370](https://github.com/saeedkolivand/ai-job-hunter-app/commit/4245370dc79fe7d0b6f9107c7b8b109bf4bccdba))

### 🎨 UI/UX

* autopilot board results behind an info popover ([#611](https://github.com/saeedkolivand/ai-job-hunter-app/issues/611)) ([b002882](https://github.com/saeedkolivand/ai-job-hunter-app/commit/b002882cefd0050ba21870ef31dc2eb31fb6358b))

### 📚 Documentation

* template series closeout (templates pr 6/6) ([#595](https://github.com/saeedkolivand/ai-job-hunter-app/issues/595)) ([8e28429](https://github.com/saeedkolivand/ai-job-hunter-app/commit/8e28429d48462a5db5989c6e8b4cef84f9c5ed24))

## [0.124.2](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.124.1...v0.124.2) (2026-07-09)

### 🐛 Bug Fixes

* aggregator low-count filtering + resume experience translation ([#588](https://github.com/saeedkolivand/ai-job-hunter-app/issues/588)) ([360dc7e](https://github.com/saeedkolivand/ai-job-hunter-app/commit/360dc7ebd734aab72e3b0286649f2f1956c0c663))

## [0.124.1](https://github.com/saeedkolivand/ai-job-hunter-app/compare/v0.124.0...v0.124.1) (2026-07-09)

### 🐛 Bug Fixes

* autopilot aggregator zero-jobs + move export diagnostics to developer settings ([#587](https://github.com/saeedkolivand/ai-job-hunter-app/issues/587)) ([8fb0522](https://github.com/saeedkolivand/ai-job-hunter-app/commit/8fb0522a57d6ea3ed6b56653f3122c96d7566269)), closes [#586](https://github.com/saeedkolivand/ai-job-hunter-app/issues/586) [pre-#586](https://github.com/saeedkolivand/pre-/issues/586)
* **deps:** migrate rustcrypto stack to cipher 0.5 and digest 0.11 ([#585](https://github.com/saeedkolivand/ai-job-hunter-app/issues/585)) ([abc24e1](https://github.com/saeedkolivand/ai-job-hunter-app/commit/abc24e1a2789818d025dcbdcbc60875c80a8ca21)), closes [#579](https://github.com/saeedkolivand/ai-job-hunter-app/issues/579) [#577](https://github.com/saeedkolivand/ai-job-hunter-app/issues/577)
* **deps:** migrate typst family to 0.15 ([#586](https://github.com/saeedkolivand/ai-job-hunter-app/issues/586)) ([794f38e](https://github.com/saeedkolivand/ai-job-hunter-app/commit/794f38e712729fd1ba1c5379db844babb1a34c55))

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
