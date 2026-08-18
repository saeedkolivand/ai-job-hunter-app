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
import { dirname, join, relative, resolve, sep } from 'node:path';
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
  'components/layout/StatusBar/index.tsx': {
    mount: 'always',
    note:
      'The app-shell status bar, rendered by routes/__root.tsx. Subscribes via ' +
      'useWorkerActivity, and is the one surface that keeps telling the truth about ' +
      'in-flight work while every route-scoped copy below has forgotten it.',
  },

  // ── Debt: backend work whose only listener is route-scoped ────────────────

  'hooks/use-resume-pipeline-session.ts': {
    mount: 'route-scoped',
    note:
      'Mounts FOUR subscriptions (job events, notifications, pipeline stages, draft ' +
      'stream) from features/documents/components/TailorFlow/GeneratingPanel.tsx — ' +
      'route-scoped AND conditionally rendered, so a generation that runs for minutes ' +
      'drops every stage event emitted while the user is elsewhere. Reconnect covers most ' +
      'of that: the session store keeps `applicationApply.applyRun`, TailorFlow hands it ' +
      'back as initialRunId/initialJobId, and the hook re-reads the run record and replays ' +
      'its persisted stage trail — so a remounted panel shows real progress and can still ' +
      'cancel. What is NOT recoverable is the streamed draft/letter/thinking text: it is ' +
      'useState with no durable transcript, so a reconnected run jumps to the finished ' +
      'document instead of the live stream.',
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
  'features/dashboard/components/AISystemStatus/index.tsx': {
    mount: 'route-scoped',
    note:
      'ACCEPTED, not debt. useWorkerActivity feeding a live "what is busy right now" readout; ' +
      'the reading is re-derived from the job registry on mount, so there is no history a ' +
      'missed event could have cost it.',
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

/** Normalize a relative path to POSIX separators, so keys are stable on Windows. */
const toPosix = (rel) => rel.split(sep).join('/');

/**
 * Blank out comments and string/template literals, preserving offsets.
 *
 * Discovery matches identifiers against source text, and prose that MENTIONS a
 * hook is not a call to it. This is not hypothetical: `services/use-jobs` has a
 * doc comment on `useJob` reading "a `useJobEvents()` subscription MUST be
 * mounted somewhere in the tree". Because a declaration slice runs to the next
 * `export`, that comment sits inside the PRECEDING hook's slice — so the plain
 * query hook `useJobQueue` was promoted to a subscription hook by a sentence,
 * and every file calling it was pulled into the inventory. The same structural
 * fact — a doc comment sits above its item, so a naive slice attributes it to
 * the item before — has bitten this repo from the other direction too.
 *
 * Characters are replaced one-for-one with spaces rather than deleted, so every
 * `match.index` still points at the right place in the original text.
 *
 * String literals are blanked alongside comments for the same reason: a hook
 * name inside a string is a mention, not a call. Doing it with a scanner rather
 * than a regex is what keeps a `//` inside a URL from eating the rest of a line
 * — which would silently DROP real code, the one failure direction a guard
 * must not have.
 */
export function stripCommentsAndStrings(src) {
  let out = '';
  let i = 0;
  const blank = (text) => text.replace(/[^\n]/g, ' ');

  while (i < src.length) {
    const two = src.slice(i, i + 2);
    if (two === '//') {
      const end = src.indexOf('\n', i);
      const stop = end === -1 ? src.length : end;
      out += blank(src.slice(i, stop));
      i = stop;
    } else if (two === '/*') {
      const end = src.indexOf('*/', i + 2);
      const stop = end === -1 ? src.length : end + 2;
      out += blank(src.slice(i, stop));
      i = stop;
    } else if (src[i] === '"' || src[i] === "'" || src[i] === '`') {
      const quote = src[i];
      let j = i + 1;
      while (j < src.length && src[j] !== quote) {
        if (src[j] === '\\') j++; // escaped char — skip the pair
        j++;
      }
      const stop = Math.min(j + 1, src.length);
      out += blank(src.slice(i, stop));
      i = stop;
    } else {
      out += src[i];
      i++;
    }
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
  // Every exported hook in services/, with its own body and the source of the
  // file it came from. The file source is kept because the closure below has to
  // resolve import aliases, which are a per-file fact.
  const hooks = new Map();
  for (const file of sourceFiles(servicesDir)) {
    // Comments and strings blanked FIRST: prose that names a hook is not a call to
    // it, and a declaration slice runs to the next `export`, so the next hook's
    // doc comment lands inside this one's body.
    const src = stripCommentsAndStrings(readFileSync(file, 'utf8'));
    const decls = [...src.matchAll(/export\s+(?:const|function)\s+(use[A-Z]\w*)/g)];
    for (const [index, match] of decls.entries()) {
      const start = match.index ?? 0;
      const end = decls[index + 1]?.index ?? src.length;
      hooks.set(match[1], { body: src.slice(start, end), src });
    }
  }

  // Seed: hooks that register a listener themselves.
  const subscribing = new Set(
    [...hooks].filter(([, h]) => /\bapi\.\w+\.on[A-Z]\w*\s*\(/.test(h.body)).map(([name]) => name)
  );

  // Close transitively. A services hook that COMPOSES another subscription hook
  // subscribes just as much as one that registers the listener itself, and a
  // seed-only scan would leave every caller of the wrapper unclassified — the
  // guard silently narrower than it looks. Iterating to a fixed point also
  // handles a chain of wrappers, not just one level.
  for (let changed = true; changed;) {
    changed = false;
    for (const [name, { body, src }] of hooks) {
      if (subscribing.has(name)) continue;
      const callable = [...subscribing, ...importedBindings(src, [...subscribing])];
      if (callable.some((s) => new RegExp(`\\b${s}\\s*\\(`).test(body))) {
        subscribing.add(name);
        changed = true;
      }
    }
  }

  return [...subscribing].sort();
}

/**
 * The LOCAL names a file has bound to any of `hooks`, following `as` aliases.
 *
 * Matching on the exported name alone is not sound in either direction, and a
 * guard that can be stepped around by renaming is worse than none:
 *
 *   * `import { useJobEvents as useEvents }` then `useEvents(cb)` subscribes
 *     exactly as much as the unaliased form, and a name-only scan misses it —
 *     an unclassified subscriber silently dropping events on navigation, which
 *     is the entire defect this check exists to prevent;
 *   * a file that happens to declare its own local `useJobEvents` and never
 *     imports ours would be reported as a subscriber it is not.
 *
 * Binding to the import is what makes both cases correct: a call only counts
 * when the name it calls actually resolves to a service hook.
 *
 * Type-only imports are skipped — `import type { useJobEvents }` cannot call
 * anything.
 */
export function importedBindings(src, hooks) {
  const bindings = new Set();
  const wanted = new Set(hooks);
  // `[^}]*` spans newlines, which matters: this repo's import blocks are
  // multi-line whenever more than two or three names are imported.
  for (const match of src.matchAll(/import\s+(type\s+)?\{([^}]*)\}\s*from/g)) {
    if (match[1]) continue; // `import type { … }`
    for (const specifier of match[2].split(',')) {
      const text = specifier.trim();
      if (!text || text.startsWith('type ')) continue; // inline `{ type Foo }`
      const [imported, alias] = text.split(/\s+as\s+/).map((s) => s.trim());
      if (imported && wanted.has(imported)) bindings.add(alias || imported);
    }
  }
  return bindings;
}

/**
 * Files that import a namespace from a services module (`import * as s from
 * '@/services'`).
 *
 * Nothing in the repo does this today, and supporting it would mean resolving
 * member expressions. Rather than let the pattern silently defeat discovery, it
 * is reported as a violation with an explanation — a guard whose blind spots are
 * visible is honest; one whose blind spots are silent is theatre.
 */
export function namespaceImporters(rendererDir = RENDERER) {
  return sourceFiles(rendererDir)
    .filter((f) => !toPosix(relative(rendererDir, f)).startsWith('services/'))
    .filter((f) =>
      /import\s+\*\s+as\s+\w+\s+from\s+['"][^'"]*services[^'"]*['"]/.test(readFileSync(f, 'utf8'))
    )
    .map((f) => toPosix(relative(rendererDir, f)))
    .sort();
}

/** Files outside `services/` that call any discovered subscription hook. */
export function discoverSubscribers(hooks, rendererDir = RENDERER) {
  if (hooks.length === 0) return [];
  return sourceFiles(rendererDir)
    .filter((f) => !toPosix(relative(rendererDir, f)).startsWith('services/'))
    .filter((f) => {
      const src = stripCommentsAndStrings(readFileSync(f, 'utf8'));
      const bindings = [...importedBindings(src, hooks)];
      if (bindings.length === 0) return false;
      return new RegExp(`\\b(?:${bindings.join('|')})\\s*\\(`).test(src);
    })
    .map((f) => toPosix(relative(rendererDir, f)))
    .sort();
}

/**
 * Every violation, as human-readable lines. Empty means the invariant holds.
 *
 * Returned rather than printed so the check is testable without capturing
 * stdout or trapping `process.exit`.
 */
export function violations(inventory = SUBSCRIBERS, hooks, subscribers, namespaced = []) {
  const problems = [];

  // Discovery resolves imported BINDINGS, which handles `as` aliases but not a
  // namespace import. Reported rather than ignored, so the blind spot is visible
  // instead of silently letting a subscriber through.
  if (namespaced.length > 0) {
    problems.push(
      'These files import a services namespace (`import * as x from "…/services"`), which\n' +
        '  this check cannot resolve to individual hooks — a subscription behind one would go\n' +
        '  undiscovered. Use named imports instead, or extend importedBindings() to resolve\n' +
        '  member expressions:\n' +
        namespaced.map((f) => `    ${f}`).join('\n')
    );
  }

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
  const problems = violations(SUBSCRIBERS, hooks, subscribers, namespaceImporters());

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
