// Drift guard: every backend event subscription is declared with its mount
// lifetime.
//
// ── The defect this exists to stop ──────────────────────────────────────────
//
// The renderer subscribes to Tauri events through a small family of service
// hooks (`useJobEvents`, `useAutopilotStepEvents`, `usePipelineStageEvents`, …).
// Each is a `useEffect` that registers a listener on mount and unregisters it on
// unmount. That is correct React — and it is exactly the problem, because the
// WORK those events describe lives in the Rust backend and does not unmount with
// the component.
//
// A board scrape, an autopilot run, a résumé pipeline, an Ollama model pull and
// an embeddings re-index all take minutes. If the only listener is mounted
// inside a route, then navigating away and back means:
//
//   * every event emitted while the user was away is dropped — including the
//     TERMINAL one, so nothing ever tells the UI the work ended;
//   * the component's local progress state was discarded on unmount, so it
//     re-renders as idle for work that is still running;
//   * a React Query mutation's `onSuccess` invalidation belongs to the unmounted
//     observer, so the finished result never reaches the cache either.
//
// That combination shipped as a user-visible bug: an autopilot card showed a
// finished run while the status bar showed it running, and clicking Run was
// refused by the backend with "a run is already in progress". An audit then
// found the same shape in the jobs scrape, the résumé pipeline, the model pull
// and the re-index.
//
// ── What this enforces ──────────────────────────────────────────────────────
//
// NOT "no route-scoped subscriptions" — some are legitimate (a live activity
// feed has nothing to preserve). It enforces that the question was ANSWERED:
// every file that subscribes appears in SUBSCRIBERS, classified, and a
// route-scoped one carries a note saying what is lost and why that is
// acceptable. A new feature that subscribes fails this check until someone
// writes that sentence — the build asks at the moment the code is written,
// instead of an audit asking a year later.
//
// Both directions are checked, so the list cannot rot: an undeclared subscriber
// fails, and so does a declared file that no longer subscribes.
//
// The hook family is DISCOVERED, not hardcoded — a brand-new subscription hook
// added to `services/` is covered without touching this file.
//
// Lives here rather than in a vitest file because the renderer's tsconfig has no
// node types, and this needs the filesystem. It runs in the `lint-format` CI job,
// which carries no path filter, so it cannot be skipped by a diff that happens to
// miss the renderer.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const RENDERER = join(REPO_ROOT, 'apps/desktop/src/renderer');
const SERVICES = join(RENDERER, 'services');

/**
 * Every file outside `services/` that mounts a backend event subscription.
 *
 * Keys are POSIX paths relative to `apps/desktop/src/renderer`.
 *
 * `mount: 'always'`  — mounted for the life of the app; no event can be missed.
 * `mount: 'route-scoped'` — mounted only while a route/panel renders, so events
 *   are LOST while the user is elsewhere. These are a DEBT LIST, not an approval
 *   list: each note says what that costs today. They are recorded rather than
 *   fixed here so each fix is a separate reviewable change per feature — and so
 *   the list cannot silently grow in the meantime.
 */
const SUBSCRIBERS = {
  'routes/__root.tsx': {
    mount: 'always',
    note: 'The app shell. This is where a subscription belongs.',
  },
  'hooks/use-autopilot-focus-navigation.ts': {
    mount: 'always',
    note: 'Called only from routes/__root.tsx.',
  },
  'hooks/use-window-taskbar-sync.ts': {
    mount: 'always',
    note: 'Called only from routes/__root.tsx.',
  },
  'hooks/use-menu-navigation.ts': {
    mount: 'always',
    note: 'Called only from routes/__root.tsx. Subscribes via useUpdater.',
  },
  'components/ui/UpdateBanner/index.tsx': {
    mount: 'always',
    note: 'Rendered by routes/__root.tsx. Subscribes via useUpdater.',
  },

  // ── Debt: backend work whose only listener is route-scoped ────────────────

  'hooks/use-resume-pipeline-session.ts': {
    mount: 'route-scoped',
    note:
      'The largest instance. Mounts FOUR subscriptions (job events, notifications, ' +
      'pipeline stages, draft stream) but is called from ' +
      'features/documents/components/TailorFlow/GeneratingPanel.tsx — route-scoped AND ' +
      'conditionally rendered. A résumé generation runs for minutes; leaving the tab drops ' +
      'its stage events and its streamed draft. The session store keeps the stage but not ' +
      'the run handle, so the remounted panel shows "generating" and can neither display ' +
      'nor cancel it.',
  },
  'features/jobs/hooks/useScraping.ts': {
    mount: 'route-scoped',
    note:
      'Scrape progress. Unmounting also discards the in-flight job id, which breaks the ' +
      '"cancel the previous scrape first" exclusivity contract: the orphan keeps writing ' +
      'into the postings cache and can no longer be cancelled from the UI, while the ' +
      'always-mounted status bar still shows it running.',
  },
  'features/jobs/components/JobsPage/index.tsx': {
    mount: 'route-scoped',
    note:
      'A scrape that finishes while the user is elsewhere drops its terminal job event, so ' +
      'the per-board diagnostics strip ("aggregator: 429 rate limited") is lost for that ' +
      'run — the one thing that explains an empty result.',
  },
  'features/autopilot/hooks/useAutopilotRun.ts': {
    mount: 'route-scoped',
    note:
      'The originally-reported bug. Partly mitigated: the card now falls back to the ' +
      "backend's persisted runStatus and the list is invalidated on a terminal step, so a " +
      'run survives navigation visibly. The step LOG is still lost.',
  },
  'features/onboarding/steps/ollama/ModelSelectionPanel/useModelPull.ts': {
    mount: 'route-scoped',
    note:
      'A multi-GB Ollama pull loses its progress and its completion event when the ' +
      'onboarding step is skipped or navigated away from. The pull itself continues.',
  },
  'features/settings/components/ai-settings/EmbeddingsSettings/index.tsx': {
    mount: 'route-scoped',
    note:
      'Re-index completion is handled only by the Settings panel that started it, and the ' +
      "panel ignores the backend's own `indexing` flag on remount.",
  },
  'features/settings/components/update-section/index.tsx': {
    mount: 'route-scoped',
    note:
      'The THIRD independent `useUpdater` instance, each with its own subscription and its ' +
      'own local status. The other two are always-mounted (the banner and the menu), so the ' +
      'update itself is never lost — but this copy resets to idle on every route change, so ' +
      'a download started in Settings shows no progress when the user returns.',
  },
  'features/monitoring/hooks/useActivityFeed.ts': {
    mount: 'route-scoped',
    note:
      'ACCEPTED, not debt. A live activity feed is a view of the current moment; there is no ' +
      'per-run state to preserve and nothing is lost by only listening while the monitoring ' +
      'page is open.',
  },
};

/** Minimum note length for a route-scoped entry to count as an explanation. */
const MIN_NOTE_CHARS = 40;

/** Recursively list `.ts`/`.tsx` sources under `dir`, skipping tests. */
function sourceFiles(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) {
      out.push(...sourceFiles(full));
      continue;
    }
    if (!/\.tsx?$/.test(name)) continue;
    if (/\.(test|spec)\.tsx?$/.test(name)) continue;
    out.push(full);
  }
  return out;
}

/**
 * The subscription hooks, discovered from `services/` rather than listed.
 *
 * A subscription hook is an exported function whose body registers a listener on
 * the app client (`api.<namespace>.on<Event>(`) — the single idiom every one of
 * them uses. Discovering it means a NEW subscription hook is covered the day it
 * is written, which a hardcoded list could never promise.
 */
export function discoverSubscriptionHooks(servicesDir = SERVICES) {
  const names = new Set();
  for (const file of sourceFiles(servicesDir)) {
    const src = readFileSync(file, 'utf8');
    const decls = [...src.matchAll(/export\s+(?:const|function)\s+(use[A-Z]\w*)/g)];
    for (const [index, match] of decls.entries()) {
      const start = match.index ?? 0;
      const end = decls[index + 1]?.index ?? src.length;
      if (/\bapi\.\w+\.on[A-Z]\w*\s*\(/.test(src.slice(start, end))) names.add(match[1]);
    }
  }
  return [...names].sort();
}

/** Files outside `services/` that call any discovered subscription hook. */
export function discoverSubscribers(hooks, rendererDir = RENDERER) {
  if (hooks.length === 0) return [];
  const pattern = new RegExp(`\\b(?:${hooks.join('|')})\\s*\\(`);
  return sourceFiles(rendererDir)
    .filter((f) => !relative(rendererDir, f).split('\\').join('/').startsWith('services/'))
    .filter((f) => pattern.test(readFileSync(f, 'utf8')))
    .map((f) => relative(rendererDir, f).split('\\').join('/'))
    .sort();
}

/**
 * Every violation, as human-readable lines. Empty means the invariant holds.
 *
 * Returned rather than printed so the check is testable without capturing
 * stdout or trapping `process.exit`.
 */
export function violations(inventory = SUBSCRIBERS, hooks, subscribers) {
  const problems = [];

  // ── Vacuity guards ───────────────────────────────────────────────────────
  // Everything after this is a set comparison, and a set comparison against an
  // empty discovery passes while checking nothing. Both discovery steps are
  // regexes over source text, so a refactor of the service idiom — or of the
  // directory layout — is exactly the change that would silently empty them.
  // These make that fail loudly instead of turning the whole check into theatre.
  if (hooks.length < 5) {
    problems.push(
      `Only ${hooks.length} subscription hooks found in services/ — the ` +
        '`api.<ns>.on<Event>(` idiom this check discovers by must have changed, so every ' +
        'check below is now vacuous. Fix the discovery before trusting a green run.'
    );
    return problems;
  }
  if (subscribers.length < 5) {
    problems.push(
      `Only ${subscribers.length} files outside services/ call a subscription hook — ` +
        'discovery is broken and the inventory is being compared against nothing.'
    );
    return problems;
  }

  const undeclared = subscribers.filter((f) => !(f in inventory));
  if (undeclared.length > 0) {
    problems.push(
      'These files subscribe to a backend event but are not declared in SUBSCRIBERS:\n' +
        undeclared.map((f) => `    ${f}`).join('\n') +
        '\n  A subscription only receives events while its component is mounted, but the work\n' +
        '  it describes runs in the Rust backend and does NOT unmount. If this file is not\n' +
        '  mounted for the life of the app, every event emitted while the user is on another\n' +
        '  route — including the terminal one — is lost, and the UI shows idle for work that\n' +
        '  is still running.\n' +
        '  Add an entry to scripts/check-event-subscriptions.mjs: `always` if it is only ever\n' +
        '  mounted from routes/__root.tsx or a provider, otherwise `route-scoped` with a note\n' +
        '  saying what is dropped and why that is acceptable.'
    );
  }

  const stale = Object.keys(inventory).filter((f) => !subscribers.includes(f));
  if (stale.length > 0) {
    problems.push(
      'Declared in SUBSCRIBERS but no longer subscribing — delete the entries so the list\n' +
        '  stays a true inventory:\n' +
        stale.map((f) => `    ${f}`).join('\n')
    );
  }

  // The note is the whole mechanism. A `route-scoped` entry with an empty note
  // would silence the undeclared check while recording nothing, which is worse
  // than no inventory: it reads as a decision that was made.
  const unexplained = Object.entries(inventory)
    .filter(([, e]) => e.mount === 'route-scoped' && e.note.trim().length < MIN_NOTE_CHARS)
    .map(([f]) => f);
  if (unexplained.length > 0) {
    problems.push(
      'A route-scoped subscription must state what is dropped while the user is elsewhere:\n' +
        unexplained.map((f) => `    ${f}`).join('\n')
    );
  }

  const badMount = Object.entries(inventory)
    .filter(([, e]) => e.mount !== 'always' && e.mount !== 'route-scoped')
    .map(([f]) => f);
  if (badMount.length > 0) {
    problems.push(
      "`mount` must be 'always' or 'route-scoped':\n" + badMount.map((f) => `    ${f}`).join('\n')
    );
  }

  return problems;
}

// Skipped when imported by the test file.
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const hooks = discoverSubscriptionHooks();
  const subscribers = discoverSubscribers(hooks);
  const problems = violations(SUBSCRIBERS, hooks, subscribers);

  if (problems.length > 0) {
    for (const p of problems) console.error(`✗ ${p}`);
    process.exit(1);
  }

  const scoped = Object.values(SUBSCRIBERS).filter((e) => e.mount === 'route-scoped').length;
  console.log(
    `check:event-subscriptions OK — ${hooks.length} subscription hooks discovered, ` +
      `${subscribers.length} subscribing files all declared ` +
      `(${subscribers.length - scoped} always-mounted, ${scoped} route-scoped and explained).`
  );
}

export { SUBSCRIBERS, RENDERER, SERVICES };
