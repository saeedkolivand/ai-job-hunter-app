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
- **Byte fidelity**: The engine file is vendored verbatim from the scroll-world skill. **One documented deviation is present and must be re-applied on future re-vendors**: the file ends with `export { mountScrollWorld }` (lines 450–456) as an ESM export. Reason: Turbopack statically analyzes this file and doesn't see the conditional CJS tail (`typeof module !== 'undefined' && …`), so without an explicit `export` the dev import resolves to no exports and `/world` returns a 500. The real export converts the file to an ES module, but the CJS/global paths above still run unchanged — **the file remains portable**. When re-vendoring the engine from upstream, preserve this export line.
- **Mobile awareness**: The engine is phone-aware out of the box. It detects coarse-pointer (touch) + ≤860px viewport, loads lighter `clipMobile` / `connectorsMobile` variants when provided, primes video decoders on first touch (iOS workaround), coalesces seek requests (prevents frame-pile freezes), and drops particle effects on mobile.
- **A11y**: On `prefers-reduced-motion: reduce`, the engine loads stills + copy, skips video playback, and presents a static fallback view. This is built into the engine.

### Media pipeline (local, zero cost)

- **Stills**: Codex CLI image_gen (gpt-image-2, subscription-billed).
- **Video**: Local ComfyUI on the owner's RTX 4090. Workflow: WanImageToVideo for dive clips (2.2 I2V A14B fp8 + lightning 4-step LoRAs), WanFirstLastFrameToVideo for connectors (frame-locked endpoints matching the dives' rendered frames). Output: SeedVR2 3B temporal upscale to ~1080p. Encode: ffmpeg with denoise (hqdn3d) pre-processing, x264 crf27 (desktop) / crf28 (mobile), `-g 4` GOP (mobile seeks smoother with tiny keyframe stride).
- **Masters and encodes**: Pre-rendered; all clips are committed as plainly versioned assets in `apps/landing/public/world/`.
- **Reproducibility**: The local pipeline (ComfyUI workflow graphs, ffmpeg commands) is documented and preserved in this session's scratchpad (scroll-world skill session files). To regenerate any scene, follow the documented method. The committed video masters are the source of truth — there is no build-time generation step.

### Weight policy (owner decision)

- **Hard budget**: ~20 MB per device variant (desktop + mobile target, separately).
- **Final achieved**:
  - Desktop: 11 clips (6 dives + 5 connectors) at 960px width, crf27, denoised = **~18 MB**.
  - Mobile: 11 clips at 720px width, crf28, denoised = **~19 MB**.
  - Stills (posters): ~4 MB.
  - **Total in `apps/landing/public/world/`**: ~41 MB committed to git.
- **Why git, not LFS?** The owner's CI checkout bandwidth quota and LFS complexity outweighed the repo size. Denoise (hqdn3d) before x264 makes the papercraft grain highly compressible; at these crf values and resolutions, plain git is simpler and faster.

### Route integration

- **URL**: `/world` (served as `world.html` by the static export, per ADR-0018's parity gate).
- **Next.js file**: `apps/landing/src/app/world/page.tsx` — metadata + OG tags + `<WorldClient />`.
- **Parity gate** (`scripts/check-parity.mjs`): Updated `REQUIRED_FILES` to include `world.html` — this ensures the built `out/` contains the `/world` route and prevents accidental deletion.
- **Marketing tier link**: One additive link in `src/content/home/body.html` (line 349: `→ or fly through the world (new)`). No other marketing copy touched; the marketing tier remains protected.

### Origin invariant (ADR-0018 security constraint reaffirmed)

All media is same-origin (`apps/landing/public/world/`). The scrub-engine injects inline `<style>` and builds its own DOM; no external stylesheets or scripts. This preserves the origin invariant for browser-stored secrets (mission-control dashboard stores a GitHub PAT in localStorage).

## Consequences

- **Repo size**: `+41 MB` in committed video assets (total landing `public/` grows by ~41 MB).
- **Deploy**: Via existing `pages.yml` workflow — no new deploy steps.
- **Regeneration**: Requires the local ComfyUI pipeline (documented in scroll-world session skill files). The scripts are preserved and method is clear; the committed encodes are final.
- **Type safety**: Vendored `scrub-engine.js` is a `.js` file (not `.ts`); the engine's config type is documented in JSDoc comments at the top of the file. Consumers (e.g., `world-config.ts`) must import the engine and infer types from usage — the engine itself is not strict-typed.
- **Mobile exclusion optional**: A page can use the engine with `clip` + `connectors` only (no mobile variants); it still works on phones (just heavier). `clipMobile` + `connectorsMobile` are opt-in optimizations.
- **Edit safety**: When editing the `scrub-engine.js` file, **use Bash heredoc (`cat >> file <<'EOF'`)** or direct file operations (not the Edit tool). The Edit tool's post-processing rewrites the file wholesale, which strips the vendored-integrity guarantee. Commit the file with `git diff` to verify byte-for-byte fidelity against the source before pushing.

## Addendum: The ESM-export deviation

The vendored engine file includes one modification from the upstream skill's original:

```javascript
// Line 450–456 (re-apply on re-vendoring)
// Deviation from the vendored original (re-apply this line on re-vendoring):
// Turbopack statically analyzes this file as ESM and doesn't see the
// conditional CJS tail above, so without a real `export` the dev import
// resolves to no exports and /world 500s. A real export just makes this an
// ES module too (`typeof module` stays safely undefined); the CJS/global
// lines above still run unchanged.
export { mountScrollWorld };
```

This is the **only** documented change from the upstream source. On future re-vendors, reapply this export line to maintain Turbopack compatibility.

## References

- Related: ADR-0018 (`apps/landing/` static export, no-server-features constraint, origin invariant).
- Files: `apps/landing/src/app/world/` (page.tsx, WorldClient.tsx, scrub-engine.js, world-config.ts), `apps/landing/public/world/` (video + stills), `apps/landing/scripts/check-parity.mjs` (parity gate).
- Workflow: `pages.yml` (deploy unchanged).
