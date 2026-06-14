# ADR-018: Revive accent-tinted aurora ambient background

Last updated: 2026-06-14

**Status:** Accepted

## Context

The aurora/cinematic backdrop was removed in commit `8688eb91` ("apple design system + UX overhaul") as part of restraint-driven design and performance optimization. At that time, `CinematicBackground` became a flat fill, shedding parallax orbs, streaks, grid, film-grain, and the animated aurora ribbons + nebulae.

The two-tone accent-gradient work (brand → brand-2 hue pair) introduced a visual opportunity: a living accent-tinted surface for the gradient to inhabit. The flat backdrop had become inert with respect to the customizable accent, making it visually disconnected from user personalization choices.

## Decision

Revive a **slim aurora** as the ambient backdrop, constrained to performance and a11y guardrails:

### Layer stack (back → front)

1. **Aurora ribbons** — three slow, wide, hue-rotating CSS keyframe blobs, deriving color from `--color-brand` + `--color-brand-2`.
2. **Nebulae** — one (balanced mode) or two (performance mode) medium blobs, same color family.
3. **Cursor glow** — a 900px lerp-smoothed JavaScript blob that trails the pointer, mixing `--color-brand` / `--color-brand-2` inline.

### Color derivation

All layers reference `--color-brand` / `--color-brand-2` via `color-mix` in CSS or inline application, so the entire aurora re-tints whenever the user changes the accent (via Settings → Appearance). The static defaults (violet → indigo, `#a855f7` → `#6366f1`) are shipped in `packages/ui/src/css/tokens.css` and override to match the runtime accent (via `applyAccent()` in `packages/ui/src/lib/theme.ts`).

### Performance mode gating

- **`low-memory`** — renders nothing; the aurora component returns null.
- **`balanced`** — one nebula, cursor glow only; no parallax or streaming animations.
- **`performance`** — adds a second nebula; cursor glow applies transform-only keyframes (no JavaScript loop).

The cursor glow's RAF loop is paused when the tab is hidden (`visibilitychange` listener) to avoid frame burn on background tabs.

### Accessibility

- All animation is disabled under `prefers-reduced-motion` CSS media query.
- Cursor glow position is seeded to viewport center on init, so blob doesn't slide in from (0,0) on first paint.

### Implementation

- **React component:** `apps/tauri/src/renderer/components/background/CinematicBackground/index.tsx`.
- **Styles:** CSS keyframes + tokens in `packages/ui/src/css/tokens.css` (`--aurora-*`, `--nebula-*` vars) and `utilities.css` (`.aurora-ribbon`, `.nebula`, `.cursor-glow` classes).
- **Theme integration:** `applyAccent()` and `restoreTheme()` already handle the brand/brand-2 tokens; the aurora consumes them via CSS variable reference, requiring no new IPC or store updates.

## Consequences

- **Brand vitality** returns as a living, interactive accent-driven backdrop. Users immediately see the visual effect of accent changes in Settings.
- **Accent visibility** — the two-tone gradient now has a clear, persistent surface to inhabit (aurora ribbons + nebulae), reinforcing personalization.
- **Performance trade-off** — a re-added (but bounded) animated layer set. Mitigations:
  - Slim layer count (3 blobs + cursor glow; no parallax/streaks/grid/film-grain/vignette from the original).
  - Transform-only keyframes (no layout thrashing).
  - RAF loop on cursor glow uses 0.5% lerp per frame (extremely slow, no jank).
  - Gated by existing Performance Mode — users on low-memory see zero overhead.
  - Reduced-motion users see static blobs (no animation).
- **Reversibility** — the aurora can be disabled by removing the `CinematicBackground` component call or returning null unconditionally; no cascading refactors needed.

## Trade-offs Evaluated

### Revive aurora vs. keep flat fill

**Chosen:** Revive (slim, constrained).

- Flat fill is inert w.r.t. accent customization; the new gradient work goes unseen at the UI boundary.
- Full revived aurora (parallax, streaks, grid, grain, vignette) was removed for good reasons (performance, restraint); selective revival of 3 blobs + cursor glow splits the difference.
- Layer count and RAF loop are measurably less costly than the original (no `Math.random()` per frame, no SVG filters, no parallax matrix math).

### Animating aurora via CSS vs. RAF

**Chosen:** CSS keyframes (aurora/nebulae), RAF for cursor glow only.

- CSS animation on static blobs scales to any device; RAF loop on pointer-tracking is per-device latency-aware (0.5% lerp doesn't block main thread).
- Decoupling allows low-memory mode to drop the entire component (no RAF setup) without disabling static blobs.

### Cursor glow lerp factor (0.5% per frame)

**Chosen:** 0.5% (0.005 multiplier).

- 0.005 is ~60fps-responsive (at 200 lerp frames, blob reaches ~63% of target distance). Slower than typical UI feedback.
- Faster (e.g. 0.02) feels snappy but cancels the "dreamy" effect; slower (e.g. 0.001) feels laggy on rapid pointer movement.
- No tuning in settings (fixed); breakpoints (low-memory) disable it entirely rather than expose a slider.

## References

- **Revived component:** `apps/tauri/src/renderer/components/background/CinematicBackground/index.tsx`.
- **Token definitions:** `packages/ui/src/css/tokens.css` (aurora/nebula color vars) + `packages/ui/src/css/utilities.css` (aurora/nebula/cursor-glow classes).
- **Accent derivation:** `packages/ui/src/lib/theme.ts` (`applyAccent()`) + `packages/ui/src/lib/color.ts` (`rotateHueHex()` hue rotation helper).
- **Commit that introduced two-tone accents:** feat: derive two-tone accent gradients and revive accent-tinted aurora.
- **Commit that originally removed aurora:** `8688eb91` (apple design system + UX overhaul).
- **Related ADR:** ADR-017 (persisted caches); ADR-004 (ports and adapters / theming concerns).
