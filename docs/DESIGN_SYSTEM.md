# Design System — AI Job Hunter

Last updated: 2026-06-11

The design system lives in `packages/ui` and is published as the `@ajh/ui` internal package. It provides design tokens, a component library, motion primitives, and theming infrastructure.

> **Design language: Apple (hybrid).** The system follows the Apple design language (`DESIGN-apple.md`) — typography-led, restrained chrome, flat surfaces with hairline elevation, the single product shadow, and the Apple type/radius grammar. **Two deliberate divergences from the Apple spec:**
>
> 1. **Accent stays violet** (`--color-brand`), not Apple's Action Blue.
> 2. **Colorful action buttons** are allowed (Apple mandates a single accent). They are _semantic_ (run/edit/delete), token-driven (`--color-action-*`), and never decorative.
>
> "Hybrid" depth: content surfaces are flat (`.surface-card`); frosted **glass is reserved for hero surfaces only** — modals, the dashboard hero, and chrome (sidebar/titlebar/sticky bars). Decorative gradients and glows live in content. A **slim, accent-tinted aurora** (ribbons, nebulae, and cursor glow, gated by performance mode) sits as the ambient backdrop behind all surfaces; it is always present when performance allows.

---

## Design Tokens

All tokens are CSS custom properties defined in `packages/ui/src/css/tokens.css`. [Tailwind CSS][tailwindcss] v4 consumes them via `@theme`.

### Color Tokens

```css
/* Brand palette */
--color-brand          /* Primary interactive color (violet — the accent) */
--color-brand-soft     /* Muted/secondary brand variant */
--color-brand-2        /* Gradient-end hue (hand-tuned per preset, rotateHueHex(-30°) for system/custom) */
--color-brand-2-soft   /* Lighter step of brand-2 (for secondary text/icons) */
--color-brand-dim      /* Darker accent tone (derived via CSS color-mix from brand) */

/* Colorful action tokens (semantic; the deliberate divergence — tokens only,
   never a raw [#hex]). Used by Button variants primary/run/edit/delete. */
--color-action-primary    /* create / generate (violet, = brand) */
--color-action-run        /* run / start (green) */
--color-action-edit       /* edit (blue) */
--color-action-delete     /* delete (red, = destructive) */
--color-action-foreground /* label on a filled action; auto-contrasted for legibility */

/* Elevation — the ONLY drop-shadow on non-chrome, reserved for product/preview
   imagery resting on a surface. Cards/buttons get NO shadow. */
--shadow-product

/* Semantic text */
--color-text-primary
--color-text-secondary
--color-text-muted
--color-text-inverse

/* Semantic surfaces */
--color-surface-base       /* App background */
--color-surface-elevated   /* Cards, panels */
--color-surface-overlay    /* Modals, dropdowns */

/* Semantic borders */
--color-border-default
--color-border-subtle
--color-border-strong

/* Feedback states */
--color-success
--color-warning
--color-error
--color-info
```

### Tailwind Utility Classes

Use these instead of raw hex values or arbitrary colors:

| Need              | Class             |
| ----------------- | ----------------- |
| Primary text/icon | `text-brand`      |
| Muted brand text  | `text-brand-soft` |
| Background fill   | `bg-brand`        |
| Border            | `border-brand`    |
| Focus ring        | `ring-brand`      |
| Success text      | `text-success`    |
| Error text        | `text-error`      |

**Never use** `[#RRGGBB]` in `className` strings — ESLint enforces this.

### Typography Scale (Apple)

The Apple type scale lives in `tokens.css` as Tailwind v4 `--text-*` tokens — each carries its line-height, weight, and letter-spacing. Authored so **body = `1rem`** with the rem root at **17px** (Apple's body baseline), so they hit Apple's exact px at the default UI size and scale with `TextScale`. Negative letter-spacing at ≥17px is the "Apple tight" cadence. **Weight ladder: 300 / 400 / 600 / 700 — `font-medium` (500) is intentionally avoided in new UI.**

| Utility               | px  | Weight | Use                         |
| --------------------- | --- | ------ | --------------------------- |
| `text-hero-display`   | 56  | 600    | Hero headline               |
| `text-display-lg`     | 40  | 600    | Tile/section headline       |
| `text-display-md`     | 34  | 600    | Section head                |
| `text-lead`           | 28  | 400    | Subcopy                     |
| `text-lead-airy`      | 24  | 300    | Airy lead (rare weight 300) |
| `text-tagline`        | 21  | 600    | Tagline / category          |
| `text-body-strong`    | 17  | 600    | Inline strong               |
| `text-body`           | 17  | 400    | Default paragraph           |
| `text-caption`        | 14  | 400    | Caption / secondary         |
| `text-caption-strong` | 14  | 600    | Emphasised caption          |
| `text-button-large`   | 18  | 300    | Large CTA (rare weight 300) |
| `text-fine-print`     | 12  | 400    | Fine print                  |
| `text-nav-link`       | 12  | 400    | Nav items                   |

### Spacing Scale

TailwindCSS default scale (8px base rhythm) plus `--spacing-section` (`5rem` / 80px) for full-bleed tile padding → `p-section` / `py-section` / `gap-section`.

### Radius (Apple grammar)

The Apple `{rounded.*}` grammar is namespaced `--rounded-*` so it does **not** clobber Tailwind's own `--radius-*` (the `rounded-sm/md/lg` utilities). Reference via `var(--rounded-…)`:

```css
--rounded-none: 0px --rounded-xs: 5px --rounded-sm: 8px --rounded-md: 11px --rounded-lg: 18px
  --rounded-pill: 9999px --rounded-full: 9999px;
```

- **Pill** (`--rounded-pill`) — the semantic colorful action CTAs (`run`/`edit`/`delete`) + search input.
- **lg** (18px) — utility/grid cards (`.surface-card`).
- **sm** (8px) — compact utility buttons.

---

## Theming

Both schemes follow the **Apple design language** with the brand violet kept as the accent:
light is a **cool-gray `#f1f1f4` canvas with white `#ffffff` cards** (and a white sidebar) and near-black ink
`#1d1d1f` — separation comes from **hairlines**, not shadows; dark is **near-black
`#1d1d1f` canvas with `#272729` tiles** that read _lighter_ than the canvas (Apple
elevation). Content surfaces are flat; glass is hero-only. Theme is **orthogonal axes**,
selectable in Settings → General → Appearance and persisted to `localStorage`:

```typescript
// packages/ui — lib/theme.ts
export type ColorScheme = 'light' | 'dark' | 'system'; // system follows the OS
export type TextScale = 'small' | 'default' | 'large'; // rem root: 16 / 17 / 19px
export interface ThemePrefs {
  scheme: ColorScheme;
  reduceTransparency: boolean; // false = follow the OS preference
  contrast: 'normal' | 'more'; // 'normal' = follow the OS preference
  textScale: TextScale; // sizes UI text; default 17px (Apple body baseline)
}
```

The engine (`applyTheme` / `applyThemeAnimated` / `restoreTheme` / `getThemePrefs`) writes
data attributes on `<html>` that the token layer keys off:

```html
<html data-color-scheme="dark" data-contrast="normal" data-text-scale="default">
  <!-- data-reduce-transparency present when reduced -->
</html>
```

- **Color scheme** — `dark` is the default token set (`tokens.css` `:root`). `[data-color-scheme='light']` overrides only the tokens that change, so flipping the tokens flips the whole UI. `system` resolves from `prefers-color-scheme` and tracks live OS changes. User-initiated scheme changes go through `applyThemeAnimated()`, which crossfades via the View Transition API (reduced-motion / unsupported-API guarded). A **pre-paint boot script in `index.html`** applies the persisted scheme/text-scale/a11y modifiers before first paint (mirrors `theme.ts`) so there is no light-theme flicker/FOUC.
- **Light legibility** — light overrides only what changes, but two systemic remaps live in `utilities.css`: bright Tailwind palette text steps (`--color-emerald-400`, …) map to their deeper `600/700` so accent/status/gradient text stays legible on white, and faint `text-foreground/NN` steps are lifted to the macOS hierarchy (secondary `~#6E6E73`, muted `~#8E8E93`) — no per-site sweep.
- **Text size** — `data-text-scale` sets the rem root (`small 16px` / `default 17px` / `large 19px` — default is the Apple body baseline, up from 16px to fix "texts too small"); a 12px floor in `utilities.css` lifts sub-12px arbitrary sizes. Both are rem-based, so they scale together.
- **Accessibility modifiers** — `[data-reduce-transparency]` solidifies all glass (also wired to `@media (prefers-reduced-transparency)` as a JS-independent fallback); `[data-contrast='more']` strengthens borders. Each is either forced on or "auto" (follows the matching OS query).
- **Fonts** — `--font-sans` prefers native San Francisco on macOS (`-apple-system`, `BlinkMacSystemFont`, `SF Pro`), falling back to bundled Inter on Windows/Linux.

Content surfaces are **flat** (`.surface-card` — surface fill + 1px hairline, no shadow/blur;
elevation = the `--color-card` step over `--color-background`). Frosted **glass is hero-only**
and reads a single material token set (`--glass-rgb`, `--glass-alpha-*`, `--glass-sat`,
`--glass-specular`) — see `utilities.css`. Never hard-code colors that don't exist in the
token system.

### Accent Color System

A **single-source, user-customizable accent** system colllapses brand-color fragmentation (formerly ~5 violet tokens) into one. The accent is always sourced from exactly one of three origins:

**Accent source enum** — `ThemePrefs.accentSource` in `packages/ui/src/lib/theme.ts`:

- **`'default'`** — uses the shipped per-scheme violet; no runtime override.
- **`'system'`** — reads the OS accent color (Windows via `UISettings::GetColorValue(UIColorType::Accent)`; macOS via the fixed accent palette; unsupported on Linux).
- **`'custom'`** — uses a user-picked hex (`ThemePrefs.accentColor`).

**Single point of derivation** — `--color-brand` in `packages/ui/src/css/tokens.css` is the sole accent. All derived colors compute from it at paint time:

- **`--color-brand-soft`** — a lighter step for secondary accent text/icons (28% whiter on dark schemes, 16% on light to account for canvas contrast).
- **`--color-brand-2` / `--color-brand-2-soft`** — gradient-end hue and its lighter variant. Hand-tuned per preset (e.g. blue #007aff→#22d3ee) or auto-derived via `rotateHueHex()` in `packages/ui/src/lib/color.ts` for system/custom accents. Derived at runtime by `applyAccent()` in `packages/ui/src/lib/theme.ts`; see `ThemePrefs.accentColor2` for preset overrides.
- **`--color-brand-dim`** — a darker tone for disabled/subtle states.
- **`--color-action-primary`** — alias for brand (the signature CTA color); semantic, never diverges.
- **`ring-brand`** — the focus ring (2px brand outline on focus-visible buttons/inputs).
- **Brand glows and gradients** (`.text-gradient`, `.gradient-border`, `.glass-violet`, `.glass-indigo`, aurora ribbons/nebulae) — all reference `var(--color-brand)` / `var(--color-brand-2)` via `color-mix` in `utilities.css` or inline CSS, so they auto-adjust with the accent.

**Auto-contrast for legibility** — `--color-action-foreground` is computed by `readableForeground()` from `packages/ui/src/lib/color.ts`, applied at runtime by `applyAccent()` in `packages/ui/src/lib/theme.ts`. So filled primary CTAs remain readable on any accent.

**Runtime applier** — `applyAccent()` in `packages/ui/src/lib/theme.ts` writes the five accent vars to `<html>.style` before paint, or clears them for `'default'`. It is idempotent and re-runs on scheme changes (light/dark swap). See `ACCENT_VARS` in theme.ts for the full list.

**User picker** — Settings → Appearance card (`apps/tauri/src/renderer/features/settings/components/general-section/AppearanceCard.tsx`) offers:

- Default chip + 8 preset macOS-style swatches (each with hand-tuned `color2`) + System chip (hidden when `supported:false`).
- Swatches preview the two-tone gradient (brand → brand-2), so users see the full accent effect before apply.
- Persisted to `localStorage` as part of `ThemePrefs`.

**One primary CTA per view** — to avoid visual noise and let the accent shine, each view has at most one `variant="primary"` button (the main action). Secondary actions use `variant="default"` (neutral, no accent).

**Semantic action colors unchanged** — status colors (success/warning/error/info) and semantic action buttons (run/edit/delete) are explicitly NOT re-tinted by accent and remain their fixed hues. Only the primary accent-driven elements change.

---

## Component Library (`@ajh/ui`)

Import everything from the package root:

```typescript
import { Button, Input, GlassCard, Modal } from '@ajh/ui';
```

### Form Controls

#### `Button`

```typescript
<Button variant="primary" size="md" loading={false} disabled={false}>
  Generate
</Button>
```

| Prop       | Type    | Values                                                                                                         |
| ---------- | ------- | -------------------------------------------------------------------------------------------------------------- |
| `variant`  | string  | `primary` `run` `edit` `delete` · `default` `glass` `ghost` · `danger` `warning` `info` `success` · `unstyled` |
| `size`     | string  | `sm` `md` `lg`                                                                                                 |
| `loading`  | boolean | Shows spinner, disables click                                                                                  |
| `disabled` | boolean | Greyed out, no interaction                                                                                     |

- **`primary`** — the signature solid **violet** CTA (`--color-action-primary`), on the **utility radius** so it matches neutral buttons like `default`. **`run` / `edit` / `delete`** — solid colorful action **pills** (semantic colour; the deliberate divergence) — these are the only filled actions that take the pill radius.
- **`default` / `glass` / `ghost`** — neutral utility buttons (rounded `sm`, not pill). **`danger` / `warning` / `info` / `success`** — translucent inline state chips.
- Apple micro-interaction baked in: weight **400** (no 500), `active:scale-[0.95]` press, 2px brand focus ring.

> **`unstyled`** is an escape hatch for custom interactive surfaces — segmented controls,
> icon toggles, inline text links, clickable cards — that supply their own appearance via
> `className`. It injects no chrome (no border / background / size / layout) but still routes
> through `Button` for consistent focus-visible + disabled handling. Prefer a semantic variant;
> reach for `unstyled` only when a styled variant would fight the call site. `Input` ships the
> same `unstyled` variant for inline/borderless fields.

#### `Input`

```typescript
<Input
  label="Job Title"
  placeholder="Software Engineer"
  error="This field is required"
  hint="Enter the exact title from the posting"
/>
```

#### `TextArea`

```typescript
<TextArea label="Cover Letter" rows={8} autoResize />
```

#### `RichTextEditor`

WYSIWYG editor for resume and cover-letter markdown, constrained to the vocabulary that survives export (h2/h3 headings, paragraphs, flat bullet lists, **bold**, _italic_, links). The document is internally uncontrolled (Tiptap holds state) and serializes to the **same markdown string** the export pipeline consumes, preserving significant whitespace and a trailing link-reference block verbatim.

```typescript
<RichTextEditor
  value={markdownString}
  onChange={setMarkdownString}
  placeholder="Edit your resume…"
  labels={{
    toolbarLabel: "Formatting toolbar",
    bold: "Bold",
    italic: "Italic",
    link: "Add link",
    bullet: "Bullet list",
    heading2: "Heading 2",
    heading3: "Heading 3",
  }}
  onSelectionChange={setHasSelection}
/>
```

Props:

- `value: string` — markdown content (re-parsed only on external document switch, not on keystroke, to preserve cursor position).
- `onChange: (md: string) => void` — emitted debounced ~200ms.
- `labels?: ToolbarLabels` — a11y strings for toolbar buttons; keep `@ajh/ui` translation-free.
- `onSelectionChange?: (hasSelection: boolean) => void` — fired when selection gains/loses a non-empty range (used by AI-rewrite toggle).
- `disabled`, `readOnly`, `placeholder`, `className`, `spellCheck` — standard HTML semantics.

**Imperative handle** (via `ref`):

- `getSelectionText(): string` — plain text of current selection.
- `getSelectionContext(): { selection, before, after }` — selection + surrounding document text (no markdown marks), used by AI-rewrite prompts.
- `replaceSelection(text: string): void` — replace selection with plain text (inline marks parsed).
- `focus(): void` — move keyboard focus into the editor.

**Locked schema (critical):** enables only h2/h3, paragraph, bullets (flat), bold, italic, links (http/https/mailto only); disables code blocks, blockquotes, nesting, images, colors. This makes typing AND paste incapable of introducing markup that won't survive export. Pasted rich HTML is coerced onto the allowed nodes.

**Round-trip contract:** `serialize(parse(md)) === md` byte-exact for unedited documents, enforced by a Vitest no-drift gate that includes real generated samples. The trailing `\n---\n` link-reference block is held out of the editable body and re-appended verbatim, so editing cannot corrupt backend link data.

Source: `packages/ui/src/components/RichTextEditor/`; exported from `@ajh/ui` at `packages/ui/src/index.ts`.

#### `Dropdown`

The single canonical select — the former `SelectDropdown` and `Dropdown` were merged into one. Keyboard navigation, drop-up flip, a leading `icon`, auto-search at ≥8 options, and optional `options[].section` group headers + `options[].meta` right-aligned text. An opt-in `tone="primary"` brand-tints the trigger (e.g. the application **status** selector); every other dropdown stays the neutral glass `tone="default"`.

```typescript
<Dropdown
  options={[{ value: "en", label: "English" }, { value: "de", label: "Deutsch" }]}
  value={locale}
  onChange={setLocale}
  placeholder="Select language"
/>
```

Accepts an optional `id` prop forwarded to the trigger button, enabling a `<label htmlFor>` to associate correctly with the control (label-click activates the dropdown; required for a11y when used inside a field wrapper such as `WizardField`).

#### `LocationInput`

Async geocode lookup with debounce:

```typescript
<LocationInput value={location} onChange={setLocation} />
```

#### `NumberField`

Controlled numeric input with string-buffer internals: suppresses `onChange` while empty/NaN, clamps to `[min, max]` on blur, and re-syncs from the external value only when it genuinely changes. Eliminates the `Number('') === 0` snap-to-zero regression. Source: `packages/ui/src/components/NumberField/NumberField.tsx`.

```typescript
<NumberField value={count} onChange={setCount} min={1} max={100} step={1} />
```

#### `OptionalHint`

Inline "optional" marker (italic, muted, caption-sized) — placed next to or below the element it qualifies so optional fields read as optional at a glance. Defaults to the word "optional"; pass children to localise or extend.

```typescript
<label>Photo <OptionalHint /></label>
<OptionalHint>optional — appears on your résumé header</OptionalHint>
```

---

### Cards & Layout

#### `GlassCard`

The primary card surface. **Flat by default** (`tone="surface"` — Apple `.surface-card`: surface fill + hairline, no shadow/blur). Frosted glass is **opt-in** for hero surfaces via `tone="glass"` (or the tonal `violet` / `indigo` / `graphite`); `neutral` is a legacy alias of `glass`.

```typescript
<GlassCard className="p-6">           {/* flat Apple content card */}
  <h2>Skill Match: 87%</h2>
</GlassCard>

<GlassCard tone="glass" className="p-6">  {/* hero surface — keeps frosted glass */}
  …
</GlassCard>
```

#### `SettingsSection`

Labeled settings group with optional description:

```typescript
<SettingsSection title="AI Provider" description="Choose your LLM backend">
  {/* settings rows */}
</SettingsSection>
```

#### `PageShell`

Wraps every route page. Provides consistent padding, max-width, and scroll container:

```typescript
// apps/tauri/src/renderer/components/layout/PageShell.tsx
<PageShell title="Dashboard">
  {/* page content */}
</PageShell>
```

---

### Feedback & Overlays

#### `Notification`

Imperative antd-style notifications. Use `useNotification()` (must be within `NotificationProvider`) to access the API:

```typescript
import { useNotification } from '@ajh/ui';

const notify = useNotification();

// Shorthand methods set the variant automatically
notify.success({ message: 'Saved' });
notify.error({ message: 'Export failed', description: 'File was locked' });
notify.warning({ message: 'Warning' });
notify.info({ message: 'Info' });

// Or use .open() for dynamic variant
notify.open({
  message: 'Custom',
  variant: 'success',
  duration: 3,
  placement: 'topRight',
  closable: true,
});

// Dismiss one by key, or all
notify.destroy(key); // dismiss one
notify.destroy(); // dismiss all
```

`NotificationConfig` shape:

| Field          | Type                                                                | Default        | Description                                        |
| -------------- | ------------------------------------------------------------------- | -------------- | -------------------------------------------------- |
| `message`      | `ReactNode`                                                         | required       | Bold title line                                    |
| `description`  | `ReactNode`                                                         | undefined      | Optional secondary line                            |
| `variant`      | `success \| error \| warning \| info`                               | `'info'`       | Icon and color scheme                              |
| `duration`     | `number` (seconds)                                                  | `4.5`          | Auto-dismiss time; `0` = sticky                    |
| `placement`    | `top \| topLeft \| topRight \| bottom \| bottomLeft \| bottomRight` | `'topRight'`   | Corner/edge anchor                                 |
| `closable`     | `boolean`                                                           | `true`         | Show close button                                  |
| `pauseOnHover` | `boolean`                                                           | `true`         | Pause timer on hover                               |
| `btn`          | `ReactNode`                                                         | undefined      | Optional action button/element                     |
| `icon`         | `ReactNode`                                                         | auto           | Override variant icon; `null` to hide              |
| `key`          | `string`                                                            | auto-generated | Stable ID; opening with same key updates that item |
| `onClose`      | `() => void`                                                        | undefined      | Callback when dismissed                            |

#### `ModalShell`

The canonical dialog container — overlay + glass panel + focus trap + Escape key. All other modals (`ConfirmModal`, etc.) compose from `ModalShell` rather than rebuilding.

```typescript
<ModalShell
  open={open}
  onClose={() => setOpen(false)}
  header={<h2>Export Document</h2>}
  footer={<Button onClick={handleExport}>Export</Button>}
>
  {/* scrollable body content */}
</ModalShell>
```

**Props:**

| Prop             | Type         | Default      | Notes                                                                                                                                          |
| ---------------- | ------------ | ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `open`           | boolean      | required     | Controls visibility and animation                                                                                                              |
| `onClose`        | `() => void` | required     | Called on Escape or backdrop click                                                                                                             |
| `children`       | ReactNode    | required     | Body content — scrolled when tall, pinned header/footer stay visible                                                                           |
| `header`         | ReactNode    | `undefined`  | Optional pinned header region (title, close button) — does not scroll; use `ariaLabelledby` to wire a title to the dialog's accessibility name |
| `footer`         | ReactNode    | `undefined`  | Optional pinned footer region (action buttons) — does not scroll; keeps CTAs visible on short windows                                          |
| `maxWidth`       | string       | `'max-w-md'` | Tailwind class capping dialog width (e.g. `max-w-lg`)                                                                                          |
| `className`      | string       | `undefined`  | Extra classes on the panel element                                                                                                             |
| `zIndex`         | number       | `600`        | z-layer (CSS `--z-modal`)                                                                                                                      |
| `borderClass`    | string       | `undefined`  | Border color (e.g. `border-red-500/30`); defaults to white hairline                                                                            |
| `ariaLabelledby` | string       | `undefined`  | `id` of the element labelling the dialog (e.g. title); wired to `aria-labelledby`                                                              |
| `ariaLabel`      | string       | `undefined`  | Accessible name when no visible title element exists to reference                                                                              |

**Anatomy:** The panel wraps a pinned `header` (shrink-0) → scrollable `body` (overflow-y-auto, @container, min-h-0 flex-1) → pinned `footer` (shrink-0). This ensures tall/multi-section modals stay usable on a 600px window and keeps buttons pinned while content scrolls.

**Guidance:**

- Multi-section modals and forms taller than viewport → **use slots**: put title/close in `header`, action buttons in `footer` so they stay pinned.
- Simple single-body modals (confirm dialogs, quick forms) → **may pass everything as `children`** — the panel's height cap and scroll still protects them.
- Set `ariaLabel` or `ariaLabelledby` so assistive tech announces the dialog. Icon-only close buttons must have an `aria-label`.

Source: `packages/ui/src/components/ModalShell/ModalShell.tsx`.

#### `ConfirmModal`

```typescript
<ConfirmModal
  open={open}
  title="Delete job?"
  description="This cannot be undone."
  confirmLabel="Delete"
  variant="destructive"
  onConfirm={handleDelete}
  onCancel={() => setOpen(false)}
/>
```

#### `HoverPopover`

Portal-positioned hover/focus popover with tooltip semantics. Used for inline status indicators (e.g. job queue depth in the StatusBar).

```typescript
<HoverPopover
  trigger={<ActivityIcon />}
  content={<div>3 jobs queued</div>}
/>
```

Popover auto-dismisses on `Escape`, loses focus, or mouse-out. Portal-rendered to avoid stacking-context clipping in scrollable containers. `role="tooltip"` for accessibility.

---

### Content & Data Display

#### `MarkdownMessage`

Renders markdown with syntax highlighting, preserving AI output formatting:

```typescript
<MarkdownMessage content={generatedCoverLetter} />
```

#### `StreamingText`

Displays streaming text with a blinking cursor and smooth character append:

```typescript
<StreamingText text={delta} done={isDone} />
```

#### `EmptyState`

```typescript
<EmptyState
  icon={<BriefcaseIcon />}
  title="No jobs yet"
  description="Run a scrape to get started"
  action={<Button onClick={openScrapePanel}>Scrape Jobs</Button>}
/>
```

#### `ErrorState`

```typescript
<ErrorState
  title="Failed to load"
  description={error.message}
  retry={refetch}
/>
```

#### `RowSkeleton` / `CardSkeleton`

Loading placeholders that match their content's shape:

```typescript
{isLoading ? (
  <>
    <RowSkeleton />
    <RowSkeleton />
    <RowSkeleton />
  </>
) : (
  <JobList jobs={jobs} />
)}
```

#### `OptionTile`

Selectable card for option groups (AI model selection, template picker):

```typescript
<OptionTile
  selected={template === "modern"}
  onClick={() => setTemplate("modern")}
  label="Modern"
  description="Clean two-column layout with accent sidebar"
  preview={<TemplatePreview id="modern" />}
/>
```

---

### Layout Primitives

#### `SegmentedControl`

Radiogroup with roving arrow-key navigation. Two layout variants:

```typescript
<SegmentedControl
  variant="track"   // or "grid"
  value={tab}
  onChange={setTab}
  options={[
    { value: "resumes", label: "Résumés" },
    { value: "cover-letters", label: "Cover Letters" },
    { value: "activity", label: "Activity" },
  ]}
/>
```

#### `Switch`

Boolean toggle rendered as `role="switch"` (built on `Button variant="unstyled"`). Pass `label` for the standard settings row (label/description left, switch right); otherwise the bare switch is returned and `aria-label` supplies the accessible name. Source: `packages/ui/src/components/Switch/Switch.tsx`.

```typescript
<Switch
  label="Reduce transparency"          // optional → renders a row
  description="Use opaque surfaces"     // optional sub-text under the label
  size="md"                            // or "sm"
  checked={enabled}
  onCheckedChange={setEnabled}
/>
```

#### `SetupHint`

Contextual setup nudge — used when a feature requires configuration (e.g. no AI provider set up). Generalises the former `AuthHint`.

```typescript
<SetupHint
  label="No AI provider configured"
  action={{ label: "Go to Settings", href: "/settings#ai" }}
/>
```

#### `NavPill`

Decorative sliding active-indicator for navigation lists. `aria-hidden` + `pointer-events-none`; scoped by `layoutId` so the pill animates within one list only.

```typescript
<NavPill layoutId="sidebar-nav" isActive={isActive} />
```

#### `ActionTile`

Clickable action card used in dashboards:

```typescript
<ActionTile
  icon={<SparklesIcon />}
  title="Generate Cover Letter"
  href="/ai-generate"
/>
```

#### `ErrorBoundary`

Wraps subtrees to catch render errors gracefully:

```typescript
<ErrorBoundary fallback={<ErrorState title="Something went wrong" />}>
  <FeatureComponent />
</ErrorBoundary>
```

---

## Motion System

Motion tokens are defined in `packages/ui/src/lib/motion.ts` and imported through the app-level alias. Uses [motion/react][motion-react] for animation primitives:

```typescript
import { transition } from '@/lib/motion';
```

### Transition Presets

| Token                | Duration | Easing      | Use Case                          |
| -------------------- | -------- | ----------- | --------------------------------- |
| `transition.fast`    | 120ms    | ease-out    | Micro interactions (hover, focus) |
| `transition.normal`  | 200ms    | ease-in-out | Standard UI transitions           |
| `transition.relaxed` | 300ms    | ease-in-out | Cards, panels                     |
| `transition.slow`    | 500ms    | ease-in-out | Page transitions                  |
| `transition.spring`  | 400ms    | spring      | Playful expansions                |
| `transition.modal`   | 250ms    | ease-out    | Modal enter/exit                  |
| `transition.overlay` | 180ms    | ease-out    | Backdrop fade                     |

### Usage

```typescript
import { motion } from "motion/react";
import { transition } from "@/lib/motion";

<motion.div
  initial={{ opacity: 0, y: 8 }}
  animate={{ opacity: 1, y: 0 }}
  transition={transition.relaxed}
>
  <GlassCard>...</GlassCard>
</motion.div>
```

**Never** use inline `{ duration: 0.3, ease: "easeOut" }` objects in feature files — ESLint enforces this.

---

## Icon System

Icons come from `lucide-react`. Always use the named import:

```typescript
import { BriefcaseIcon, SparklesIcon, ChevronRightIcon } from 'lucide-react';
```

Standard sizes: `size={16}` (inline), `size={20}` (buttons/UI), `size={24}` (headings/empty states).

---

## `cn()` Utility

Merges Tailwind classes safely using `clsx` + `tailwind-merge`:

```typescript
import { cn } from "@/lib/cn";

<div className={cn("px-4 py-2 rounded-md", isActive && "bg-brand text-white", className)} />
```

---

## ESLint Design System Rules

These are enforced in CI (`pnpm lint:strict`) by [ESLint][eslint]:

| Rule                                 | What it prevents                                                                 |
| ------------------------------------ | -------------------------------------------------------------------------------- |
| No `[#RRGGBB]` in className          | Hardcoded hex colors                                                             |
| No `<button>` raw element            | Missing Button primitive (use `variant="unstyled"` for custom surfaces)          |
| No `<select>` raw element            | Missing Dropdown primitive                                                       |
| No `<textarea>` raw element          | Missing TextArea primitive                                                       |
| No `<input>` raw element             | Missing Input primitive — `type=range\|file\|checkbox\|radio\|hidden` are exempt |
| No inline `{duration, ease}` objects | Missing motion token (use `transition.*`, `withDelay()`)                         |
| No `window.api.*` in features/routes | Direct IPC bypass                                                                |
| No `react-i18next` direct import     | Missing i18n wrapper                                                             |

---

## Adding a New Component

1. Create `packages/ui/src/components/MyComponent.tsx`
2. Export from `packages/ui/src/index.ts`
3. Add Storybook story if applicable
4. Document it here

Components that belong in `packages/ui`:

- Used in 2+ features
- Pure UI — no IPC, no Zustand, no routing

Components that belong in `features/*/components/`:

- Used only in one route/feature

Components that belong in `components/layout/`:

- App chrome (sidebar, titlebar, statusbar, shell wrappers)

[tailwindcss]: https://tailwindcss.com
[motion-react]: https://motion.dev
[eslint]: https://eslint.org
