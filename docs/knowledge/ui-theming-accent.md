# UI Theming & Accent System

Thin pointer to the runtime theme engine and customizable accent color system.

## Theme engine

The **runtime theme engine** applies color scheme (light/dark/system), accessibility modifiers (reduce-transparency, high-contrast), and user-customizable accent colors to the document root before paint. Two independent orthogonal axes:

- **Color scheme** — `'light' | 'dark' | 'system'` (system follows OS preference; live tracking). Persisted in `localStorage`.
- **Accent source** — `'default' | 'system' | 'custom'`. Always exactly one source of truth. Persisted in `localStorage` as part of `ThemePrefs`.

**Owning source:** `packages/ui/src/lib/theme.ts` (`applyTheme`, `applyThemeAnimated`, `restoreTheme`, `getThemePrefs`, `ThemePrefs` interface, `AccentSource` type).

**Design system reference:** `docs/DESIGN_SYSTEM.md` § Theming + § Accent color system.

## Accent color derivation

**Single-source collapse:** `--color-brand` in `packages/ui/src/css/tokens.css` is the ONE accent. All derived colors (`brand-soft`, `brand-dim`, `action-primary`, `ring-brand`, glows) compute from it deterministically:

- **CSS-first:** `brand-dim` + glows use `color-mix()` — no JS, robust scaling.
- **Runtime:** `applyAccent()` (packages/ui/src/lib/theme.ts:75-112) computes and writes `brand-soft` (lightened per-scheme by 28% dark / 16% light), `action-foreground` (auto-contrasted WCAG label color), plus optional gradient mid/end vars (`brand-mid`, `brand-2` + soft steps) from `ThemePrefs.accentColor2` (secondary hue for two-tone gradients; system accents auto-rotated via `rotateHueHex`).

**Color math:** `packages/ui/src/lib/color.ts` (`parseHex`, `luminance`, `lightenHex`, `readableForeground`). Used by the applier to ensure custom/system accents stay legible and cohesive.

## Native accent integration

**System accent read:** `system_accent_color()` IPC command in `apps/desktop/src-tauri/src/commands/system/mod.rs`. Async startup call; returns native OS accent (Windows `UISettings::GetColorValue(UIColorType::Accent)`; macOS fixed palette via `defaults`; Linux unsupported).

**Service hook:** `useSystemAccent` in renderer/services fetches and caches the result.

**UI:** Appearance card (`apps/desktop/src/renderer/features/settings/components/general-section/AppearanceCard.tsx`) offers Default + 8 presets + System chip (hidden if unsupported).

**Contract:** `packages/shared/src/ipc/contracts/system.ts` (`accentColor()`).

## Semantic action colors (unchanged by accent)

Status (success/warning/error/info) and semantic action buttons (run/edit/delete) are explicitly NOT re-tinted by accent — they remain their fixed hues per-scheme. Only primary CTA, secondary text, focus rings, and glows derive from the accent.

## Decision record

See `docs/adr/0004-single-source-user-customizable-accent-color.md` for context, alternatives rejected, and consequences (macOS fixed palette, Linux unsupported, per-scheme lightness recalc, etc.).
