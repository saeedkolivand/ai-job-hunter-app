---
status: accepted
---

# Single-source user-customizable accent color with native OS integration

## Context

The accent color (the primary interactive UI element tint) was previously fragmented across ~5 violet tokens (`--color-brand`, `--color-brand-soft`, `--color-brand-dim`, `--color-action-primary`, and scattered brand glows). Changing the accent required overriding multiple CSS properties, leading to inconsistent derivation and no easy way to support user customization or native OS accent detection. The design system's "Apple hybrid" rule constrains the accent to one, but it was fixed per-scheme with no runtime user control.

## Decision

Collapse all accent color to a **single `--color-brand` source**, from which all derived colors (soft, dim, action-primary, focus rings, glows) **derive deterministically** — either at paint time via CSS `color-mix()` or at boot/theme-change via the runtime applier. Introduce a **`ThemePrefs.accentSource` enum** with three modes:

1. **`'default'`** — cleared override; uses shipped per-scheme violet.
2. **`'system'`** — reads the native OS accent color (Windows via `UISettings::GetColorValue(UIColorType::Accent)`, macOS via `defaults read -g AppleAccentColor` mapped to the fixed macOS palette, Linux unsupported).
3. **`'custom'`** — user-picked hex stored in `ThemePrefs.accentColor`.

The runtime **accent applier** (`applyAccent()` in `packages/ui/src/lib/theme.ts`, using color math from `packages/ui/src/lib/color.ts`) writes `--color-brand` + `--color-brand-soft` (lightened by 28% on dark scheme, 16% on light to preserve canvas contrast) + `--color-action-foreground` (auto-contrasted: dark label on pale accents, white on dark accents, via WCAG luminance) to the document root before paint, ensuring every accent variant stays legible and cohesive. The applier is **idempotent** and re-runs on scheme change (light/dark swap) to recalculate softness per-canvas.

An **IPC command** `system_accent_color()` (in `apps/desktop/src-tauri/src/commands/system/mod.rs`) fetches the native accent async; a `useSystemAccent` service hook wraps it for React. The Settings → Appearance card UI offers Default + 8 preset swatches + System (chip hidden when `supported:false`).

**Semantic action colors (run/edit/delete) and status colors (success/warning/error/info) are explicitly NOT accent-driven** and remain their fixed hues. Only primary CTA, secondary text, focus rings, and glows derive from the accent.

## Considered options

1. **Single-source applier with native OS read + custom picker (chosen).** One accent, deterministic derivation, live native integration, user control. Cost: color math in JS, runtime applier runs on every theme change, IPC command on startup.
2. **Split fixed-brand vs variable-accent tokens.** Keep shipped violet as `--color-fixed-brand` and add a separate `--color-accent` override layer. Downside: two sources of truth, maintenance burden, no clear derivation rule for which tokens respond to which.
3. **Use `objc2` crate for macOS native accent.** Better than `defaults` shell script. Rejected: repo pattern is shell + Cargo `[target.'cfg(windows)']` conditional compilation; adding a heavy ObjC dependency for macOS-only feature breaks this precedent. The fixed macOS palette (8 exact colors) is deterministic and ships with zero new deps.
4. **Constrain picker to preset colors only.** Simpler, no auto-contrast edge cases. Rejected: custom-hex picker offers users more control and the auto-contrast function handles any hex robustly; the tradeoff favors power.

## Consequences

- **Custom accents must auto-contrast** — the runtime applier computes `--color-action-foreground` per accent so any hex stays legible. High-saturation or edge-case colors (near-black, near-white) are readable but may look unusual; the system is honest about what works.
- **macOS accent is constrained to 8 fixed hues.** The `defaults read -g AppleAccentColor` returns a numeric index `0…7` (Apple's fixed palette: red, orange, yellow, green, cyan, blue, purple, pink). Custom hex beyond this palette is possible on the app side but won't match the OS setting. This is documented in the Appearance card UI.
- **Linux System accent is unsupported.** No standard OS accent API on Linux (GNOME/KDE vary); the System chip is hidden on Linux (`system_accent_color` returns `supported:false`). Future work could add a hardcoded colorway or query `dconf` per-DE, but that's out of scope.
- **Accent applier is called on every theme change** (scheme flip, settings update). Cost is sub-millisecond (string parsing + color math) and paid only on user action (no perf hit to rendering).
- **Boot-time System accent read is async.** The IPC call to fetch the native accent happens in the Settings mount; the picker shows a loading state. If the read fails, the System chip still appears but is disabled (graceful degradation).
- **CSS derivation is robust.** `brand-dim` and glows use `color-mix(--color-brand, #000 10%)` and similar, so they scale correctly without runtime involvement — the system is **CSS-first**, JS-applied only for the custom/system overrides and auto-contrast.

## References

- Theme engine: `packages/ui/src/lib/theme.ts` (`applyAccent`, `ThemePrefs`, `AccentSource`).
- Color math: `packages/ui/src/lib/color.ts` (`parseHex`, `luminance`, `lightenHex`, `readableForeground`).
- Tokens: `packages/ui/src/css/tokens.css` (`:root --color-brand`, `[data-color-scheme] --color-brand` overrides).
- System accent command: `apps/desktop/src-tauri/src/commands/system/mod.rs` (`system_accent_color`).
- Contracts: `packages/shared/src/ipc/contracts/system.ts` (`accentColor()`).
- UI: `apps/desktop/src/renderer/features/settings/components/general-section/AppearanceCard.tsx`.
- Tests: `packages/ui/src/lib/color.test.ts` (28 unit tests), `packages/ui/src/lib/theme.test.ts` (accent cases).
