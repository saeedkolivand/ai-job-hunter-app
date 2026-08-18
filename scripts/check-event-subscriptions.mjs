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

import { createHash } from 'node:crypto';
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
  //
  // `hash` is a tripwire, not part of the invariant above: 12 hex chars of the
  // subscriber file's own sha256 (see `hashBytes`/`staleNoteEntries`). When the
  // file's current hash no longer matches, the note was written about a version
  // of the code that no longer exists — re-read the file, fix the note if it
  // drifted, and paste in the new hash `staleNoteEntries` prints. This is
  // deliberately dumb: it fires on a pure rename or comment tweak too, same as
  // a snapshot test. That is the correct trade — the alternative is a note that
  // silently outlives the code it describes, which is exactly how 8 of these 9
  // notes went stale with nobody noticing.
  //
  // KNOWN BLIND SPOT, measured the first time this fired for real: the hash
  // covers the DECLARED file only, so a note also goes stale when the behaviour
  // it describes changes in something that file merely CALLS. Both misses came
  // from one batch — JobsPage's note was fixed inside `useScraping.ts`, and
  // update-section's inside `services/use-updater/` plus the Rust updater —
  // and neither declared file was touched, so neither tripped. Hashing the
  // transitive import graph would fire on nearly every commit and be ignored
  // within a week, so the trade stands; the entries that were fixed elsewhere
  // say so in their own note instead. Re-read a note whenever you change what
  // its file DEPENDS on, not only the file itself.

  'hooks/use-resume-pipeline-session.ts': {
    mount: 'route-scoped',
    hash: '10ff9d4443cb',
    note:
      'Mounted in TailorFlow/index.tsx (not GeneratingPanel, which takes only props) via ' +
      "useTailorPipeline → useResumePipelineSession. Reconnect (the session store's " +
      'applyRun → run-record re-read + stage-trail replay) restores progress, the step ' +
      'checklist, and Cancel, so a remounted panel is not blank. A run that FAILS while the ' +
      "user is elsewhere now shows why: `error` falls back to the run record's " +
      '`stoppedReason`, so returning no longer lands on a bare "configuring" wizard as if ' +
      'Generate had never been pressed. NOT recovered: the streamed draft/letter/thinking ' +
      'text (useState, no transcript — a reconnected run jumps straight to the finished ' +
      "document), and the live message's exact stage/seconds detail, since the record " +
      'persists only the wire token.',
  },
  'features/jobs/hooks/useScraping.ts': {
    mount: 'route-scoped',
    hash: 'b16ef7bcd515',
    note:
      'Nothing user-visible is lost on a route change any more. The progress readout used to ' +
      'show a false 0% for the rest of the run — on the default single-board search ' +
      '`scrape:progress` fires once, at completion, so it never self-corrected — and now ' +
      "falls back to the job record's persisted `progress`, which the watchdog was already " +
      'fetching and discarding. The watchdog also has a leading tick, so recovery is ' +
      'immediate rather than 2.5s late. A scrape that FAILS off-page now restores a ' +
      'sanitized reason instead of an unexplained empty list, and the completed path ' +
      'invalidates the postings cache the way the live event handler does. Still bounded by ' +
      'the 2.5s poll: between ticks the fraction is as stale as the last poll.',
  },
  'features/jobs/components/JobsPage/index.tsx': {
    mount: 'route-scoped',
    hash: 'b2df3b57b2c8',
    note:
      'A scrape that ends off-page — completed OR failed — now comes back explained. The ' +
      "watchdog re-reads the job record's `result.boards` for the diagnostics strip, writes " +
      'a sanitized failure note on the failed branch, and invalidates the postings cache so ' +
      'a remount inside the 30s query staleTime cannot show 0 jobs for a scrape that found ' +
      'some. Cancelled stays deliberately bare: the user initiated it, so there is nothing ' +
      'to explain. NOTE this page composes the fix rather than containing it — all of it ' +
      "lives in `useScraping.ts`, so this entry's own hash could not have detected the " +
      'change (see the dependency caveat above).',
  },
  'features/autopilot/hooks/useAutopilotRun.ts': {
    mount: 'route-scoped',
    hash: 'bf031614c42a',
    note:
      "The originally-reported bug, now closed on both halves. The card reads the backend's " +
      'persisted `runStatus`, and `useAutopilots` sets `staleTime: 0` so every remount ' +
      'refetches — previously it inherited the 30s default with nothing invalidating on run ' +
      'START, so right after create-then-auto-run the card could show idle with an enabled ' +
      'Run button for a live run. The inline error / "already running" banner now lives in ' +
      'the session store and survives navigation. Still lost, accepted: every step-log line ' +
      '(mount-local reducer — only per-board summaries persist, not the rank/re-rank detail ' +
      'that explains a thin result). The terminal-step invalidation does NOT help while ' +
      'unmounted — the subscription itself is torn down — so the remount refetch is the ' +
      'recovery, not that call.',
  },
  'features/onboarding/steps/ollama/ModelSelectionPanel/useModelPull.ts': {
    mount: 'route-scoped',
    hash: 'df7f51d3d579',
    note:
      'A multi-GB pull survives any unmount — and the trigger really is ANY unmount, not just ' +
      'skipping the step: switching to the Cloud/CLI tab and back, or Back/Forward through ' +
      'the wizard, both tear this hook down. On mount it now re-reads the job registry and ' +
      "adopts a still-active `ai.pull_model` job, so progress resumes and the run's " +
      'completion handling still fires. Clicking Download again cannot start a second ' +
      'concurrent pull: `ai_pull_model` uses `job_start_exclusive` and hands back the ' +
      "existing job id to re-attach to. Right after adopting, it also re-reads that job's " +
      'OWN current status (`jobs_get`) and settles immediately if already `completed`/' +
      "`failed` — closing the race where the job's ONE terminal event fires in the gap " +
      'between the registry list resolving and `pullJobId` committing, which would ' +
      'otherwise be dropped for good and leave the panel reporting `pulling` forever (PR ' +
      '#1036 review finding). That commit is synchronous into a ref, not left to the next ' +
      'render, because a promise that resolves purely through microtasks (a real IPC ' +
      'response included) can settle before React ever gets a scheduler turn. NOT ' +
      'recovered, still deliberately: a pull that reaches a terminal state entirely BEFORE ' +
      'the registry snapshot is taken — finished during the unmount gap, never observed as ' +
      'still running/queued — is never adopted at all, so its success toast and the ' +
      'health/models recheck are not retroactively fired; doing that on every later mount ' +
      'would be worse than missing them once.',
  },
  'features/settings/components/ai-settings/EmbeddingsSettings/index.tsx': {
    mount: 'route-scoped',
    hash: '0de0f20640fd',
    note:
      'Completion is reported only by the panel that started the run: its `useJobEvents` ' +
      'handler unsubscribes on unmount, so the complete / partial-failure / failure toast is ' +
      'lost and never re-raised (no notification-center record either). The button also ' +
      'reverts to "Re-index now" mid-run because its busy state is local (`reindexJobId` / ' +
      "`reembed.isPending`), not the backend's `indexing` flag. That flag IS read on remount " +
      '— it only drives the stale-count copy line, which is hidden whenever `stale === 0`, ' +
      'and a re-index of already-indexed documents keeps `stale` at 0 for the whole run, so ' +
      'it never has anywhere to render. No data loss or double-billing: `ai_reembed_all` ' +
      'hands back the already-running job id.',
  },
  'features/settings/components/update-section/index.tsx': {
    mount: 'route-scoped',
    hash: '896ef1fd88e1',
    note:
      'The THIRD `useUpdater` instance, alongside the always-mounted banner and menu — but ' +
      'the three no longer disagree. `useUpdater` keeps status in a module-level store all ' +
      'instances share, so a remounted panel immediately shows a download another instance ' +
      'already knew about instead of rendering a LIVE "Check now" over work in flight. Two ' +
      'backend guards close the rest: `updater_check` returns the known state BEFORE emitting ' +
      '`checking` when a download is running or done (so it neither blanks the banner nor ' +
      "clears Rust's `downloaded_bytes` and forces a re-download), and `updater_download` is " +
      'guarded against re-entry by a Drop-based guard that cannot latch. NOTE the fix lives ' +
      "in `services/use-updater/` and the Rust updater, so this entry's own hash could not " +
      'have detected it (see the dependency caveat above).',
  },
  'features/monitoring/hooks/useActivityFeed.ts': {
    mount: 'route-scoped',
    hash: 'e1923404534f',
    note:
      'ACCEPTED, not debt. A live activity feed is a view of the current moment; there is no ' +
      'per-run state to preserve and nothing is lost by only listening while the monitoring ' +
      'page is open.',
  },
  'features/dashboard/components/AISystemStatus/index.tsx': {
    mount: 'route-scoped',
    hash: '55fe0b8309f1',
    note:
      'Not accepted-by-design: the safety here is real but conditional. StatusBar mounts the ' +
      'identical `useWorkerActivity` from routes/__root.tsx (inside `ProtocolVersionGate`), ' +
      'so the shared job-events cache keeps invalidating while this card is unmounted and the ' +
      'counts read correctly on return — but that is an UNENFORCED cross-file coupling; ' +
      'nothing checks that StatusBar keeps being rendered everywhere, and this entry is the ' +
      'only place the dependency is written down. There is also a real gap: React Query ' +
      "defaults to `networkMode: 'online'` (no override here) and the client sets " +
      '`refetchOnReconnect: false`, so with `navigator.onLine` false every fetch this card ' +
      'depends on pauses and nothing re-drives it — the card then shows stale/idle counts for ' +
      'backend work (embeddings, local-Ollama generation) that keeps running regardless of ' +
      'network state. Minor: `refreshing`, the spinner flag, is local and drops mid-refresh ' +
      'on unmount.',
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
 * First 12 hex chars of a file's sha256 — enough to detect any byte change
 * without carrying a 64-char digest around in a hand-edited table.
 */
export function hashBytes(bytes) {
  return createHash('sha256').update(bytes).digest('hex').slice(0, 12);
}

/**
 * Route-scoped entries whose stored `hash` no longer matches their subscriber
 * file's CURRENT bytes — i.e. the file changed since the note was last read
 * against it, so the note may be describing code that no longer exists.
 *
 * `readFile(relPath)` is injected (default: read from the real renderer tree)
 * so this stays a pure function of its inputs: a test can hand it a fixture
 * reader instead of touching the real tree, and — the one thing a stale-note
 * guard's own test must never do — this file's own tests do not hash a real
 * file and then compare that hash to itself.
 *
 * Only `route-scoped` entries carry a `hash`. An `always`-mounted entry's note
 * is a much weaker claim ("this is only ever called from routes/__root.tsx"),
 * true by construction of where the call sites live rather than a description
 * of what navigating away costs — and the always-mounted files here
 * (routes/__root.tsx, StatusBar, …) are touched often for unrelated UI work,
 * so hashing them would mostly generate noise on the debt list this exists to
 * keep honest, not on the structural fact those entries actually assert.
 */
export function staleNoteEntries(
  inventory = SUBSCRIBERS,
  readFile = (rel) => readFileSync(join(RENDERER, rel))
) {
  const stale = [];
  for (const [file, entry] of Object.entries(inventory)) {
    if (!entry.hash) continue;
    const currentHash = hashBytes(readFile(file));
    if (currentHash !== entry.hash) stale.push({ file, note: entry.note, currentHash });
  }
  return stale;
}

/**
 * Every violation, as human-readable lines. Empty means the invariant holds.
 *
 * Returned rather than printed so the check is testable without capturing
 * stdout or trapping `process.exit`.
 */
export function violations(
  inventory = SUBSCRIBERS,
  hooks,
  subscribers,
  namespaced = [],
  staleNotes = []
) {
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

  // The tripwire. A note is a claim about the code at a specific point in
  // time; a changed file invalidates that claim until a human re-reads it.
  // This fires on ANY byte change to the subscriber file — including a pure
  // rename, comment, or formatting tweak — by design: the failure mode this
  // guards is a note that silently outlives the code it describes (8 of the
  // 9 route-scoped notes here did exactly that), and the fix is always a
  // one-line hash update, cheap next to the cost of finding out a year late.
  if (staleNotes.length > 0) {
    problems.push(
      'These subscriber files changed since their note was last checked against them — the\n' +
        '  note may no longer describe the code. Re-read the file, fix the note if it drifted,\n' +
        '  then paste in the new hash shown below:\n' +
        staleNotes
          .map(
            ({ file, note, currentHash }) =>
              `    ${file}\n` + `      current note: "${note}"\n` + `      new hash: ${currentHash}`
          )
          .join('\n')
    );
  }

  return problems;
}

// Skipped when imported by the test file.
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const hooks = discoverSubscriptionHooks();
  const subscribers = discoverSubscribers(hooks);
  const staleNotes = staleNoteEntries(SUBSCRIBERS);
  const problems = violations(SUBSCRIBERS, hooks, subscribers, namespaceImporters(), staleNotes);

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
