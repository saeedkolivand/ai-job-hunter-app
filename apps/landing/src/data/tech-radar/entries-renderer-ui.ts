import type { TechRadarEntry } from './types';

export const rendererUiEntries: readonly TechRadarEntry[] = [
  // ── Renderer & UI ───────────────────────────────────────────────────────
  {
    id: 'tauri',
    name: 'Tauri 2',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'tauri',
    summary: 'OS-native WebView desktop shell — not Electron.',
    rationale:
      "Rust backend plus the OS's own WebView (WebView2 on Windows, WebKit on macOS/Linux) instead of bundling Chromium: installers land in the tens of MB rather than 150+ MB, and the renderer only reaches Rust through commands explicitly allowed by Tauri's capability manifest. The trade-off — WebKit on macOS doesn't always render identically to WebView2 — is handled with a cross-platform CSS pass before each release.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'react',
    name: 'React 19',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'react',
    summary: 'Concurrent rendering, Actions, first-class TanStack support.',
    rationale:
      'The renderer is React 19.2 throughout apps/desktop and apps/landing — Actions/useActionState for async transitions, ref as a plain prop, and the concurrent-safe integration TanStack Query and Zustand both depend on.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'tanstack-query',
    name: 'TanStack Query 5',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: '@tanstack/react-query',
    summary: 'The only sanctioned way for a component to reach IPC.',
    rationale:
      'Every server-state read/write goes through a service hook in renderer/services/ — no useState + useEffect fetching, no direct window.api access from features/routes/components. Query keys are centralized so cache invalidation after a mutation is deliberate, not guessed at.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'tanstack-router',
    name: 'TanStack Router',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: '@tanstack/react-router',
    summary: 'File-based routing with typed route + search params.',
    rationale:
      'Chosen for compile-time-checked routes and search-param types over a stringly-typed router API — the desktop app has 11+ feature routes, and a renamed route param fails at build time instead of at runtime.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'zustand',
    name: 'Zustand 5',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'zustand',
    summary: 'Minimal client-only state — persisted prefs, transient session.',
    rationale:
      "Used over Redux because the client-state slices (persisted preferences, the transient generation session) are simple enough that Redux's action/reducer boilerplate bought nothing. Zustand is a plain hook with no provider tree, and it's React 19 concurrent-safe.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'tailwind',
    name: 'Tailwind CSS 4',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'tailwindcss',
    summary: 'CSS-first @theme config — the design-token backbone.',
    rationale:
      "packages/ui defines every design token (--color-brand, --color-surface-elevated, …) as a CSS custom property consumed via Tailwind 4's @theme. ESLint then bans a raw hex color in any className, so a color can't drift from the token file without the linter catching it.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'motion',
    name: 'Motion (motion/react)',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'motion',
    summary: 'Animation library, wrapped behind named transition tokens.',
    rationale:
      'packages/ui/src/lib/motion.ts exposes named presets (transition.fast/.spring/.modal/…) over the raw library; ESLint blocks an inline { duration, ease } object anywhere in feature code, so every animation in the app traces back to one small token file.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'custom-state-machine',
    name: 'Hand-rolled state machine (lib/machine.ts)',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'technique',
    summary: '~80-line machine + a useMachine hook for any 3+-state flow.',
    rationale:
      'Any flow with 3+ states (onboarding, streaming generation) gets a state machine. Named states replace boolean tangles (isLoading && isDone) and make an impossible state impossible to represent, without pulling in a general-purpose library — see XState, held, for why not that one.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'xstate',
    name: 'XState',
    ring: 'hold',
    quadrant: 'renderer-ui',
    subjectKind: 'not-adopted',
    summary: 'Considered for the same job as our micro state machine — held.',
    rationale:
      "The flows in this app top out at a handful of linear states, so XState's parallel states, history, and guards buy nothing today, at the cost of bundle weight and its own config DSL. Kept as an explicit option if a flow ever genuinely needs what it offers — this entry exists so that reasoning stays visible instead of getting re-litigated.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'wcag22aa',
    name: 'WCAG 2.2 AA',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'technique',
    summary: 'The non-negotiable accessibility floor for every route.',
    rationale:
      "Enforced via eslint-plugin-jsx-a11y, @axe-core/playwright, and apps/landing's own check:a11y script; /accessibility publishes the conformance statement. Colour is never the sole signal for state — including on this very page's ring encoding — and every interactive element ships a visible :focus-visible ring.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'webgl-landing',
    name: 'WebGL / shader-driven landing experiences',
    ring: 'hold',
    quadrant: 'renderer-ui',
    subjectKind: 'technique',
    summary: 'Two hand-built WebGL landing pieces were built, then shelved.',
    rationale:
      'TERMINAL VELOCITY (a scroll-driven CG retelling of the job hunt) and RIPBOOK (a notebook-styled concept) were each built across real milestones and then deliberately abandoned before their visual approval gates passed — the landing site returned to a plain static/Next export. The dormant webgl-author/shader-engineer/webgl-reviewer agent trio stays dormant; a returning WebGL surface would route back to them, not the general frontend author.',
    adrSlug: '0017-landing-consolidation-static-site',
    lastReviewed: '2026-08-05',
  },
];
