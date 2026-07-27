---
status: accepted
---

# Landing `/world` route: scroll-scrubbed papercraft camera flight

## Context

Recorded from owner decision 2026-07-21.

ADR-0018 established the landing as a **Next.js static-export app** (no server features, no runtime SSR) with two visual skins: marketing tier (pages 1–4: home, creature, how-it-works, privacy) and docs tier (planned PR2–PR4). The marketing tier was sealed to preserve brand and tone.

On 2026-07-21, the owner shipped a third surface: `/world`, a **scroll-scrubbed video route** that tells the product narrative through an interactive camera flight over papercraft diorama scenes. This route breaks the binary tier classification — it is marketing content (public entry point, linked from home), but uses interactive video (beyond foley JS) under the "client component only" constraint of ADR-0018's no-server-features rule. The route is technically built by Next (and subject to the parity gate), but the media pipeline and rendering engine are intentionally orthogonal to the app: the engine is a **vendored vanilla-JS** portable module, and the video masters and encodes are pre-produced artifacts, not generated at build time.

## Decision

**The `/world` route is a scroll-driven video experience**, delivered as a Next.js client-component-only route with pre-produced media assets.

### Narrative structure

Six dive scenes tell the story (slump → doomscroll → workshop → robot engine → godmode → the offer/CTA), connected by five aerial transition clips (connectors). Each scene uses a still poster (fallback for no-media / a11y), a video clip, and optional copy + CTA. Desktop is 16:9; mobile is optionally a separate natively-rendered 9:16 clip chain (lighter encodes, native aspect ratio).

### Engine: vendored scroll-world scrub-engine

- **Source**: `apps/landing/src/app/world/scrub-engine.js` — portable vanilla-JS (zero dependencies) scroll-scrubbing engine, framework-agnostic (works in plain HTML, Next, Vue, server-rendered, anything).
- **Mount point**: `WorldClient.tsx` — a React client component (`'use client'`) that calls `mountScrollWorld(container, config)` from a `useEffect` hook and mounts the engine's DOM + CSS into a `<div id="world">` ref.
- **Byte fidelity**: The engine file is vendored from the scroll-world skill and stays as close to verbatim as possible. **Every deviation must be listed in the deviation log below and re-applied on future re-vendors**; edits stay minimal and well-scoped so a future re-vendor diff remains readable.
- **Mobile awareness**: The engine is phone-aware out of the box. It classifies the device once at mount (coarse pointer **and** a phone-sized screen — see D2), loads lighter `clipMobile` / `connectorsMobile` variants when provided, primes video decoders on touch (iOS workaround — see D3), coalesces seek requests (prevents frame-pile freezes), and drops particle effects on touch devices.
- **A11y**: On `prefers-reduced-motion: reduce`, the engine loads stills + copy, skips video playback, and presents a static fallback view. This is built into the engine.

### Media pipeline (local, zero cost)

- **Stills**: Codex CLI image_gen (gpt-image-2, subscription-billed).
- **Video**: Local ComfyUI on the owner's RTX 4090. Renders (1024x576 desktop / 576x1024 portrait), then SeedVR2 3B temporal upscale to 1920x1080-class. Workflow: WanImageToVideo for dive clips (2.2 I2V A14B fp8 + lightning 4-step LoRAs), WanFirstLastFrameToVideo for connectors (frame-locked endpoints matching the dives' rendered frames). Encode: ffmpeg with denoise (hqdn3d) pre-processing, then dual-codec primary+fallback strategy: **desktop receives AV1 primary** (libsvtav1, 1440w, crf 38) plus **H.264 fallback** (1104w, crf 27) selected at runtime via canPlayType check in WorldClient (withAv1Sources in world-config.ts); **mobile H.264 only** (720w, crf 28) because phone software AV1 decode cannot sustain scrubbing; `-g 4` GOP for mobile seeks smoother with tiny keyframe stride.
- **Masters and encodes**: Pre-rendered; all clips are committed as plainly versioned assets in `apps/landing/public/world/`.
- **Reproducibility**: The local pipeline (ComfyUI workflow graphs, ffmpeg commands) is documented and preserved in this session's scratchpad (scroll-world skill session files). To regenerate any scene, follow the documented method. The committed video masters are the source of truth — there is no build-time generation step.

### Weight policy (owner decision)

- **Hard budget**: ~20 MB per delivered set (visitors stream one codec set, not both desktop variants).
- **Final achieved**:
  - Desktop AV1 primary: 11 clips at 1440w, crf 38, denoised = **~23 MB**.
  - Desktop H.264 fallback: 11 clips at 1104w, crf 27, denoised = **~20 MB**.
  - Mobile H.264 only: 11 clips at 720w, crf 28, denoised = **~17 MB**.
  - Stills (posters): ~4 MB.
  - **Total in `apps/landing/public/world/`**: ~64 MB committed to git (all codec variants stored; each visitor loads one set).
- **Why git, not LFS?** The owner's CI checkout bandwidth quota and LFS complexity outweighed the repo size. Denoise (hqdn3d) before x264/AV1 makes the papercraft grain highly compressible; at these crf values and resolutions, plain git is simpler and faster. Dropping the H.264 fallback set is the future lever if AV1 support becomes universal.

### Route integration

- **URL**: `/world` (served as `world.html` by the static export, per ADR-0018's parity gate).
- **Next.js file**: `apps/landing/src/app/world/page.tsx` — metadata + OG tags + `<WorldClient />`.
- **Parity gate** (`scripts/check-parity.mjs`): Updated `REQUIRED_FILES` to include `world.html` — this ensures the built `out/` contains the `/world` route and prevents accidental deletion.
- **Marketing tier link**: One additive link in `src/components/home/HomeBody.tsx` (lines 693–694: `→ or fly through the world (new)`). No other marketing copy touched; the marketing tier remains protected.

### Origin invariant (ADR-0018 security constraint reaffirmed)

All media is same-origin (`apps/landing/public/world/`). The scrub-engine injects inline `<style>` and builds its own DOM; no external stylesheets or scripts. This preserves the origin invariant for browser-stored secrets (mission-control dashboard stores a GitHub PAT in localStorage).

## Consequences

- **Repo size**: `+64 MB` in committed video assets (all codec variants stored; each visitor loads one set per their browser capability). Hqdn3d denoise pre-x264/AV1 is what makes the papercraft grain compressible at these crf values; dropping the H.264 fallback set when AV1 becomes universal is the future size-reduction lever.
- **Deploy**: Via existing `pages.yml` workflow — no new deploy steps.
- **Regeneration**: Requires the local ComfyUI pipeline (documented in scroll-world session skill files). The scripts are preserved and method is clear; the committed encodes are final.
- **Type safety**: Vendored `scrub-engine.js` is a `.js` file (not `.ts`); the engine's config type is documented in JSDoc comments at the top of the file. Consumers (e.g., `world-config.ts`) must import the engine and infer types from usage — the engine itself is not strict-typed.
- **Mobile exclusion optional**: A page can use the engine with `clip` + `connectors` only (no mobile variants); it still works on phones (just heavier). `clipMobile` + `connectorsMobile` are opt-in optimizations.
- **Edit safety**: When editing the `scrub-engine.js` file, **use Bash heredoc (`cat >> file <<'EOF'`)** or direct file operations (not the Edit tool). The Edit tool's post-processing rewrites the file wholesale, which strips the vendored-integrity guarantee. Commit the file with `git diff` to verify byte-for-byte fidelity against the source before pushing.

## Addendum: engine deviation log

`scrub-engine.js` is vendored third-party code. Every change from the upstream skill's original is recorded here and **must be re-applied on a re-vendor**. `apps/landing/src/app/world/scrub-engine.test.ts` is the mechanical guard: it mounts the engine in jsdom and asserts D2–D4 behaviourally, so a re-vendor that drops a deviation fails `pnpm -F @ajh/landing test` instead of silently regressing on hardware nobody has to hand.

### D1 — ESM export (Turbopack)

```javascript
export { mountScrollWorld };
```

Turbopack statically analyzes this file as ESM and doesn't see the conditional CJS tail above it, so without a real `export` the dev import resolves to no exports and `/world` 500s. A real export just makes this an ES module too (`typeof module` stays safely undefined); the CJS/global lines still run unchanged — **the file remains portable**.

### D2 — device classification: coarse **and** phone-sized, frozen at mount

Upstream: `const isMobile = () => coarse || smallMQ.matches`, consulted live per clip. `(hover:none) and (pointer:coarse)` matches iPadOS, Android tablets and touch laptops at **any** width, so every tablet permanently took the 720×1280 crf28 portrait phone encodes, the AV1 desktop branch was dead for them, and desktop CSS (>860px) cropped/upscaled a portrait video.

Now a phone is a coarse pointer **and** a phone-sized screen, tested on the screen's **short side** (`Math.min(screen.width, screen.height) <= 500`) so rotating the device can't flip the decision — iPhone Pro Max ≈440 → phone, iPad mini 744 → desktop. Non-touch windows keep the legacy `≤860px` rule.

The result is a `const` evaluated **once at mount**, not a function: a live check let a resize serve one scene's poster from one set and the next scene's clip from the other. **Behaviour change:** a desktop browser resized (or a DevTools device toggle flipped) across 860px no longer switches asset sets without a reload.

Three call sites consume it: the scene poster (`stillMobile` vs `still`), the clip URL in `loadClip`, and — easy to miss — the `raf()` seek step `eps`. Tablets therefore also move from the phone's coarse `0.02` to the desktop `0.008`, i.e. finer scrubbing and more decodes. That is the intended pairing with the heavier desktop encode they now receive, and it stays bounded by the existing `s.video.seeking` coalescer, which already refuses to queue a seek while the decoder is busy.

`coarse` on its own remains the gate for the particle drop and the URL-bar resize guard — genuine touch-browser traits. It is **not** the gate for priming; see D3.

### D3 — iOS priming: persistent listeners + prime on creation

Upstream registered the primer `{once:true}` on `pointerdown`/`touchstart`, so it only reached videos that existed at the first touch — in practice segment 0 alone, because `loadClip` is gated on scroll proximity. iOS WebKit refuses to load media data for a video created outside a gesture, so every later clip never fired `loadeddata`/`loadedmetadata`, `s.ready` stayed false, `raf()` skipped it, `seeked` never fired, `has-clip` was never added, and every scene past the first showed its poster forever.

Two changes: `loadClip` primes a clip **immediately on creation** when `userReady` is already set (a muted + `playsinline` `play()` _is_ permitted outside a gesture and forces the data load), and the gesture listeners **stay registered** (still passive) so any later touch primes anything still unprimed — which also covers a first touch landing while segment 0's blob fetch is in flight. `primeVideo` now takes the segment and dedupes on a per-segment `s.primed` flag (reset if `play()` is refused, so the next gesture retries).

The priming gate is `canTouch = navigator.maxTouchPoints > 0`, evaluated once at mount — **not** `coarse` and **not** `isMobile`:

- Not `isMobile`, because WebKit's gesture-gated media loading is a browser policy, not an asset tier — iPads need priming even though D2 now gives them the desktop set.
- Not `coarse`, because since iPadOS 13.4 an iPad with a Magic Keyboard/trackpad reports `hover:hover` + `pointer:fine`. A `coarse` gate leaves exactly that configuration on posters forever — the original bug, on a device the original fix's rationale claimed to cover.

`maxTouchPoints` catches every touch-capable browser. The cost is a harmless muted `play()`→`pause()` on Windows touch laptops; a real desktop reports `0` and never primes at all (locked by a test, since that guard is the only thing standing between desktop and a burst of spurious `play()` calls).

`play()` returning a non-promise (pre-promise WebKit) now pauses on the spot rather than leaving the clip running under the scrubber. A per-segment `s.primeTries` caps priming at 3 attempts, mirroring `s.tries` in D4 — a browser that refuses muted playback outright would otherwise take a fresh `play()` on every `pointerdown` for the life of the page, since a refusal deliberately clears `s.primed`.

### D4 — bounded clip-failure recovery

Upstream latched `s.loading = true` forever on success (a clip that then failed to decode wedged its scene on the poster with no path back) while a failing `fetch` cleared the latch and re-requested on **every scroll tick**. An `error` listener on the video now removes the dead element, drops `has-clip`, and resets the segment so a later scroll can retry; a per-segment `s.tries` counter caps that at 3 attempts, so neither failure mode storms or wedges.

**Every** listener on the clip now opens with a shared liveness check, `const live = () => s.video === v;`. Media elements keep firing after a segment has torn them down and replaced them, and a discarded element outlives its replacement, so an unguarded late event mutates the state of the **live** clip:

- `error` — resets the segment while the live clip is mounted, freezing it and stacking a second `<video>` into the scene on the next tick.
- `loadedmetadata` — sets `s.ready = true` when the live video has no metadata at all, after which `raf()` scrubs it against the `duration || 1` fallback.
- `seeked` (`{once:true}`) — adds `has-clip`, hiding the poster to reveal a video that was never painted.
- `loadeddata` — `v.pause()` stays unguarded (it must pause the element that fired), but the priming decision is gated.

Teardown order matters: `v.removeAttribute('src')` + `v.load()` run **before** `URL.revokeObjectURL(...)`, because a detached element can hold decoder resources until its source is dropped and the load algorithm re-run — and the retry path can produce up to three such elements per segment. Live clips keeping their blob URL for the page's lifetime stays deliberate (they must remain seekable); a torn-down element is a different case, and without the revoke a failing segment leaks a multi-MB blob per retry.

### D5 — header doc comment

The file's usage/`MOBILE` comment block was updated to describe D2–D4 accurately (frozen phone/desktop split, continuous priming). No behaviour.

It also no longer lists the particle drop and the URL-bar resize guard as phone-tier behaviours: both key off the coarse pointer **alone**, so tablets get them too. As written before, the header contradicted the inline comments and D2. The block now states all three gates explicitly.

### Runtime consequence of D2 for AV1

`WorldClient.tsx` is unchanged: it still picks AV1 vs H.264 via `canPlayType` before mounting. That branch was previously unreachable for tablets and is now live for them. WebKit only reports AV1 support where there is hardware decode, which most iPads lack, so they resolve to `''` and take the **H.264 desktop** set — correct and intended. `withAv1Sources` never touches `clipMobile`/`connectorsMobile`, so the phone path is unaffected either way.

## References

- Related: ADR-0018 (`apps/landing/` static export, no-server-features constraint, origin invariant).
- Files: `apps/landing/src/app/world/` (page.tsx, WorldClient.tsx, scrub-engine.js, world-config.ts), `apps/landing/public/world/` (video + stills), `apps/landing/scripts/check-parity.mjs` (parity gate).
- Workflow: `pages.yml` (deploy unchanged).
