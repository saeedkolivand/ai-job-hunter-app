# Design System — AI Job Hunter

Last updated: 2026-06-03

The design system lives in `packages/ui` and is published as the `@ajh/ui` internal package. It provides design tokens, a component library, motion primitives, and theming infrastructure.

---

## Design Tokens

All tokens are CSS custom properties defined in `packages/ui/src/css/tokens.css`. [Tailwind CSS][tailwindcss] v4 consumes them via `@theme`.

### Color Tokens

```css
/* Brand palette */
--color-brand          /* Primary interactive color */
--color-brand-soft     /* Muted/secondary brand variant */
--color-brand-hover    /* Hover state */
--color-brand-active   /* Active/pressed state */

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

### Typography Scale

```css
--font-size-xs: 0.75rem /* 12px */ --font-size-sm: 0.875rem /* 14px */ --font-size-base: 1rem
  /* 16px */ --font-size-lg: 1.125rem /* 18px */ --font-size-xl: 1.25rem /* 20px */
  --font-size-2xl: 1.5rem /* 24px */ --font-size-3xl: 1.875rem /* 30px */;
```

### Spacing Scale

TailwindCSS default scale plus:

```css
--spacing-18: 4.5rem --spacing-22: 5.5rem --spacing-88: 22rem --spacing-112: 28rem
  --spacing-128: 32rem;
```

### Radius

```css
--radius-sm: 0.25rem --radius-md: 0.5rem --radius-lg: 0.75rem --radius-xl: 1rem --radius-2xl: 1.5rem
  --radius-full: 9999px;
```

---

## Theming

The design target is **macOS-grade in both schemes** with the brand violet kept as the
accent: light is a calm gray-canvas/white-card System-Settings model; dark is a neutral
cool charcoal with cards that read _lighter_ than the canvas (macOS elevation). Theme is
**orthogonal axes**, selectable in Settings → General → Appearance and persisted to
`localStorage`:

```typescript
// packages/ui — lib/theme.ts
export type ColorScheme = 'light' | 'dark' | 'system'; // system follows the OS
export type TextScale = 'small' | 'default' | 'large'; // scales the rem root (15/16/18px)
export interface ThemePrefs {
  scheme: ColorScheme;
  reduceTransparency: boolean; // false = follow the OS preference
  contrast: 'normal' | 'more'; // 'normal' = follow the OS preference
  textScale: TextScale; // sizes UI text; default 16px
}
```

The engine (`applyTheme` / `applyThemeAnimated` / `restoreTheme` / `getThemePrefs`) writes
data attributes on `<html>` that the token layer keys off:

```html
<html data-color-scheme="dark" data-contrast="normal" data-text-scale="default">
  <!-- data-reduce-transparency present when reduced -->
</html>
```

- **Color scheme** — `dark` is the default token set (`tokens.css` `:root`). `[data-color-scheme='light']` overrides only the tokens that change, so flipping the tokens flips the whole UI. `system` resolves from `prefers-color-scheme` and tracks live OS changes. User-initiated scheme changes go through `applyThemeAnimated()`, which crossfades via the View Transition API (reduced-motion / unsupported-API guarded).
- **Light legibility** — light overrides only what changes, but two systemic remaps live in `utilities.css`: bright Tailwind palette text steps (`--color-emerald-400`, …) map to their deeper `600/700` so accent/status/gradient text stays legible on white, and faint `text-foreground/NN` steps are lifted to the macOS hierarchy (secondary `~#6E6E73`, muted `~#8E8E93`) — no per-site sweep.
- **Text size** — `data-text-scale` sets the rem root (`small 15px` / `default 16px` / `large 18px`); a 12px floor in `utilities.css` lifts sub-12px arbitrary sizes. Both are rem-based, so they scale together.
- **Accessibility modifiers** — `[data-reduce-transparency]` solidifies all glass (also wired to `@media (prefers-reduced-transparency)` as a JS-independent fallback); `[data-contrast='more']` strengthens borders. Each is either forced on or "auto" (follows the matching OS query).
- **Fonts** — `--font-sans` prefers native San Francisco on macOS (`-apple-system`, `BlinkMacSystemFont`, `SF Pro`), falling back to bundled Inter on Windows/Linux.

Glass surfaces read a single material token set (`--glass-rgb`, `--glass-alpha-*`,
`--glass-sat`, `--glass-specular`) — see `utilities.css`. Never hard-code colors that
don't exist in the token system.

---

## Component Library (`@ajh/ui`)

Import everything from the package root:

```typescript
import { Button, Input, GlassCard, Modal } from '@ajh/ui';
```

### Form Controls

#### `Button`

```typescript
<Button variant="default" size="md" loading={false} disabled={false}>
  Apply Now
</Button>
```

| Prop       | Type    | Values                                                                   |
| ---------- | ------- | ------------------------------------------------------------------------ |
| `variant`  | string  | `default` `glass` `ghost` `danger` `warning` `info` `success` `unstyled` |
| `size`     | string  | `sm` `md` `lg`                                                           |
| `loading`  | boolean | Shows spinner, disables click                                            |
| `disabled` | boolean | Greyed out, no interaction                                               |

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

#### `SelectDropdown`

```typescript
<SelectDropdown
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

---

### Cards & Layout

#### `GlassCard`

The primary card surface. Applies a frosted-glass background with subtle border:

```typescript
<GlassCard className="p-6">
  <h2>Skill Match: 87%</h2>
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

#### `Toast` / `Notification`

```typescript
import { useToast, ToastVariant } from '@ajh/ui';

const { toast } = useToast();
toast({ title: 'Saved', variant: 'success' });
toast({ title: 'Error exporting', variant: 'error' });
```

| `ToastVariant` | Description           |
| -------------- | --------------------- |
| `success`      | Green, checkmark icon |
| `error`        | Red, X icon           |
| `warning`      | Amber, warning icon   |
| `info`         | Blue, info icon       |

#### `Modal` / `ModalShell`

```typescript
<ModalShell open={open} onClose={() => setOpen(false)} title="Export Document">
  {/* modal body */}
</ModalShell>
```

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
| No `<select>` raw element            | Missing SelectDropdown primitive                                                 |
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
