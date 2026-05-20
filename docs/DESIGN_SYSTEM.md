# Design System

This document is the single source of truth for UI consistency in AI Job Hunter.
All UI decisions should trace back to this guide.

---

## Principles

1. **Cinematic** — The UI has depth, atmosphere, and physical presence
2. **Local-first feel** — Dark, calm, focused. Not a SaaS dashboard.
3. **Glassmorphism with readability** — Glass surfaces enhance depth; content always reads clearly
4. **Motion is purposeful** — Animations communicate state, not decoration

---

## Color Tokens

All colors are defined as Tailwind custom tokens in `tailwind.config.ts`.

| Token           | Value                   | Usage                      |
| --------------- | ----------------------- | -------------------------- |
| `foreground`    | `rgba(255,255,255,0.9)` | Primary text               |
| `foreground/80` | —                       | Secondary text             |
| `foreground/50` | —                       | Placeholder / hint text    |
| `foreground/30` | —                       | Disabled / muted text      |
| `[#a855f7]`     | Purple 500              | Primary accent             |
| `[#c084fc]`     | Purple 400              | Icon accent, active states |
| `[#7c3aed]`     | Purple 700              | Gradient start             |
| `emerald-400`   | —                       | Success, match, connected  |
| `amber-400`     | —                       | Warning                    |
| `red-400`       | —                       | Error, destructive         |
| `blue-400`      | —                       | Info, save action          |

---

## Background & Surface System

### App background

Cinematic animated gradient with soft colored blobs — defined in `CinematicBackground.tsx`.
Never use plain `bg-black` for the app shell.

### Glass surfaces (z-order)

```
z-index 0:  App background (gradient + animated blobs)
z-index 10: Sidebar, main panel (glass-surface)
z-index 20: Cards (GlassCard)
z-index 30: Dropdowns, tooltips
z-index 40: Floating click-capture overlays
z-index 50: Modals, dialogs (glass-modal)
z-index 60: Command palette
```

### CSS classes

| Class           | Background               | Blur                        | Use case             |
| --------------- | ------------------------ | --------------------------- | -------------------- |
| `glass-surface` | `rgba(255,255,255,0.04)` | `blur(12px)`                | Sidebar, main panels |
| `glass-modal`   | `rgba(10,10,20,0.85)`    | `blur(40px) saturate(200%)` | Modals, dialogs      |

### Backdrop for modals

When a modal opens, the backdrop behind it must:

1. Apply `backdrop-filter: blur(64px) saturate(120%) brightness(0.6)` to destroy background detail
2. Overlay `bg-[#06060f]/70` to crush remaining contrast (blur alone preserves contrast ratios)
3. Add ambient bokeh blobs (purple + indigo + magenta at low opacity, `filter: blur(48-80px)`)
4. Add a vignette radial gradient to darken edges

**Never use a plain `bg-black/60` alone** — it produces a flat, non-premium result.

---

## Typography Scale

```
text-[9px]    Mono chips, version numbers
text-[10px]   Labels, badges, tracking-heavy uppercase
text-[11px]   Secondary metadata, hints
text-xs       12px — Form labels, timestamps
text-sm       14px — Body text, list items
text-base     16px — Section headings
text-lg       18px — Page titles
text-xl+      Rare — Hero text only
```

### Label convention (section headers)

```
text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/30
```

---

## Spacing & Border Radius

| Use                     | Value                      |
| ----------------------- | -------------------------- |
| Card inner padding      | `px-4 py-4` or `px-5 py-5` |
| Page horizontal padding | `px-8`                     |
| Page vertical padding   | `py-6` – `py-8`            |
| Input padding           | `px-3 py-2`                |
| Card radius             | `rounded-xl`               |
| Button radius           | `rounded-lg`               |
| Badge radius            | `rounded-full`             |
| Modal radius            | `rounded-2xl`              |

---

## Border System

```
border-white/[0.05]  Very subtle — panel dividers
border-white/[0.07]  Default card borders
border-white/[0.1]   Hover / active borders, modal borders
border-white/[0.15]  glass-modal border
border-[#a855f7]/20  Purple accent — active cards
border-[#a855f7]/40  Active nav, selected states
```

---

## Blur System

| Surface              | blur                                  |
| -------------------- | ------------------------------------- |
| `glass-surface`      | `12px`                                |
| Sidebar Ollama badge | `8px`                                 |
| Dropdown menus       | `16px`                                |
| `glass-modal`        | `40px saturate(200%)`                 |
| Modal backdrop       | `64px saturate(120%) brightness(0.6)` |

**Rule**: Always pair heavy blur with a tint layer. Blur destroys shape; tint destroys contrast.

---

## Shadows & Glows

### Depth shadows

```
shadow-2xl            Cards
0 32px 80px rgba(0,0,0,0.6)   Modal lift
```

### Glow utilities

```css
.glow-purple {
  box-shadow: 0 0 40px rgba(168, 85, 247, 0.3);
}
.glow-subtle {
  box-shadow: 0 0 20px rgba(168, 85, 247, 0.15);
}
```

Apply `hover:glow-purple` to primary CTAs.

### Status glows

```
bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]   Connected / active
bg-[#a855f7] shadow-[0_0_4px_rgba(168,85,247,0.6)]    Pulsing indicator
```

---

## Motion Principles

All animation uses `motion/react` (Motion One / Framer Motion).

### Durations

| Category             | Duration  |
| -------------------- | --------- |
| Micro (hover, badge) | 100–150ms |
| Component mount      | 200–280ms |
| Page transition      | 300ms     |
| Backdrop fade        | 150–180ms |
| Ambient / glow       | 600–900ms |

### Easing

```
ease: [0.22, 1, 0.36, 1]   // Default for mounts — fast out, soft settle
ease: 'easeOut'             // Ambient / atmospheric
```

### Spring settings (nav pill)

```
{ type: 'spring', stiffness: 380, damping: 32 }
```

### Page transitions

Wrap route content in `<PageTransition>`. Animates `opacity + y` on mount/unmount.

### AnimatePresence

Always use `mode="wait"` when swapping between two mutually exclusive states (tabs, wizard steps).

---

## Component States

### Button variants

| Variant | Use                                     |
| ------- | --------------------------------------- |
| `glass` | Primary CTA — glass surface with border |
| `ghost` | Secondary — transparent, hover only     |

### Interactive states

```
Default:   text-foreground/45 border-white/[0.07]
Hover:     text-foreground/75 border-white/[0.10] bg-white/[0.04]
Active:    text-[#c084fc]     border-[#a855f7]/40  bg-[#a855f7]/10
Disabled:  opacity-40         pointer-events-none
```

---

## Form Inputs

Standard input class:

```
rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2
text-xs text-foreground/80 placeholder:text-foreground/25
outline-none focus:border-[#a855f7]/40 transition-colors
```

---

## Glassmorphism Checklist

Before shipping any new modal or overlay:

- [ ] Backdrop blur is `blur(64px)` or stronger
- [ ] Contrast crush layer (`bg-[#06060f]/70` or similar) on top of blur
- [ ] Bokeh blobs present for atmospheric depth
- [ ] Modal panel is `≥ 0.82` opacity (not fully transparent)
- [ ] Top edge highlight: `inset 0 1px 0 rgba(255,255,255,0.1)`
- [ ] Modal content is immediately readable without visual noise from behind
