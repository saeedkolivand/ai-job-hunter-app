import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import {
  discoverSubscribers,
  discoverSubscriptionHooks,
  hashBytes,
  importedBindings,
  stripCommentsAndStrings,
  namespaceImporters,
  staleNoteEntries,
  SUBSCRIBERS,
  violations,
} from './check-event-subscriptions.mjs';

// Two kinds of test here, and the split matters.
//
// The functions are exercised against a synthetic renderer tree, so a case can
// state exactly what it is testing without depending on the real app's current
// shape. Then a handful of cases run against the REAL tree, because a guard
// whose discovery silently stops matching the actual source is the one failure
// mode that makes all of this theatre — and only the real tree can catch that.

const root = join(tmpdir(), `check-event-subs-test-${process.pid}`);
const renderer = join(root, 'renderer');
const services = join(renderer, 'services');

function write(rel, content) {
  const full = join(renderer, rel);
  mkdirSync(join(full, '..'), { recursive: true });
  writeFileSync(full, content);
}

/** A service hook that registers a listener — the shape discovery looks for. */
const SUBSCRIBING_HOOK = (name, ns, evt) => `
export const ${name} = (cb) => {
  const api = useAppClient();
  useEffect(() => api.${ns}.on${evt}(cb), [api, cb]);
};
`;

/** A service hook that does NOT subscribe — a query hook, the common case. */
const PLAIN_HOOK = (name) => `
export const ${name} = () => useQuery({ queryKey: ['${name}'], queryFn: () => api.${name}() });
`;

beforeEach(() => {
  rmSync(root, { recursive: true, force: true });
  mkdirSync(services, { recursive: true });
});

afterEach(() => rmSync(root, { recursive: true, force: true }));

describe('discoverSubscriptionHooks', () => {
  it('finds a hook that registers a listener and ignores one that does not', () => {
    write(
      'services/use-jobs/use-jobs.ts',
      SUBSCRIBING_HOOK('useJobEvents', 'jobs', 'Event') + PLAIN_HOOK('useJobs')
    );

    expect(discoverSubscriptionHooks(services)).toEqual(['useJobEvents']);
  });

  it('attributes the listener to the RIGHT hook when several share a file', () => {
    // The bug this guards: a naive "does this file subscribe" check would mark
    // every exported hook in the file, so the plain hook's name would enter the
    // pattern and every file merely CALLING `useJobs` would be reported as a
    // subscriber. Discovery slices per declaration for exactly this reason.
    write(
      'services/use-jobs/use-jobs.ts',
      PLAIN_HOOK('useJobs') +
        SUBSCRIBING_HOOK('useJobEvents', 'jobs', 'Event') +
        PLAIN_HOOK('useJobDetail')
    );

    expect(discoverSubscriptionHooks(services)).toEqual(['useJobEvents']);
  });

  it('skips test files, so a fixture cannot invent a hook', () => {
    write('services/use-jobs/use-jobs.test.ts', SUBSCRIBING_HOOK('useFakeEvents', 'fake', 'Thing'));

    expect(discoverSubscriptionHooks(services)).toEqual([]);
  });

  it('counts a WRAPPER that composes a subscription hook instead of registering one', () => {
    // The real gap this closes: `useWorkerActivity` never touches `api.*.on*`, it
    // calls `useJobEvents`. Without the closure it is not a subscription hook, so
    // its callers — the always-mounted status bar and two live dashboards — were
    // invisible to the guard entirely.
    write('services/use-jobs/use-jobs.ts', SUBSCRIBING_HOOK('useJobEvents', 'jobs', 'Event'));
    write(
      'services/use-activity/use-activity.ts',
      `import { useJobEvents } from '../use-jobs/use-jobs';\nexport const useActivity = () => { useJobEvents(noop); };`
    );

    expect(discoverSubscriptionHooks(services)).toEqual(['useActivity', 'useJobEvents']);
  });

  it('closes a CHAIN of wrappers, not just one level', () => {
    // Why a fixed point rather than a single extra pass: the second wrapper is
    // only reachable once the first has been promoted.
    write('services/use-jobs/use-jobs.ts', SUBSCRIBING_HOOK('useJobEvents', 'jobs', 'Event'));
    write(
      'services/use-a/use-a.ts',
      `import { useJobEvents } from '../use-jobs/use-jobs';\nexport const useA = () => { useJobEvents(noop); };`
    );
    write(
      'services/use-b/use-b.ts',
      `import { useA } from '../use-a/use-a';\nexport const useB = () => { useA(); };`
    );

    expect(discoverSubscriptionHooks(services)).toEqual(['useA', 'useB', 'useJobEvents']);
  });

  it('follows an alias when a wrapper renames what it composes', () => {
    write('services/use-jobs/use-jobs.ts', SUBSCRIBING_HOOK('useJobEvents', 'jobs', 'Event'));
    write(
      'services/use-activity/use-activity.ts',
      `import { useJobEvents as useEv } from '../use-jobs/use-jobs';\nexport const useActivity = () => { useEv(noop); };`
    );

    expect(discoverSubscriptionHooks(services)).toEqual(['useActivity', 'useJobEvents']);
  });

  it("is not promoted by the NEXT hook's doc comment", () => {
    // The bug that shipped. A declaration slice runs to the next `export`, so the
    // doc comment belonging to the hook BELOW lands inside this one's body. The
    // real `services/use-jobs` has exactly this: a doc comment on `useJob` reading
    // "a `useJobEvents()` subscription MUST be mounted somewhere in the tree",
    // which promoted the plain query hook above it and dragged every file calling
    // that query into the inventory.
    write(
      'services/use-jobs/use-jobs.ts',
      PLAIN_HOOK('useJobQueue') +
        `\n/**\n * HARD REQUIREMENT: a \`useJobEvents()\` subscription MUST be mounted.\n */\n` +
        SUBSCRIBING_HOOK('useJobEvents', 'jobs', 'Event')
    );

    expect(discoverSubscriptionHooks(services)).toEqual(['useJobEvents']);
  });

  it('is not promoted by a commented-out call', () => {
    write('services/use-jobs/use-jobs.ts', SUBSCRIBING_HOOK('useJobEvents', 'jobs', 'Event'));
    write(
      'services/use-dead/use-dead.ts',
      `import { useJobEvents } from '../use-jobs/use-jobs';\nexport const useDead = () => {\n  // useJobEvents(noop);\n};`
    );

    expect(discoverSubscriptionHooks(services)).toEqual(['useJobEvents']);
  });

  it('is not promoted by a listener registration that only appears in a string', () => {
    write(
      'services/use-doc/use-doc.ts',
      `export const useDoc = () => logger.warn('call api.jobs.onEvent( to subscribe');`
    );

    expect(discoverSubscriptionHooks(services)).toEqual([]);
  });

  it('does not promote a hook that merely sits in a file next to a subscriber', () => {
    // Guards the closure against the sloppy version: "this FILE subscribes" would
    // promote every hook in it and cascade to their callers.
    write(
      'services/use-jobs/use-jobs.ts',
      SUBSCRIBING_HOOK('useJobEvents', 'jobs', 'Event') + PLAIN_HOOK('useJobs')
    );

    expect(discoverSubscriptionHooks(services)).toEqual(['useJobEvents']);
  });
});

describe('discoverSubscribers', () => {
  beforeEach(() => {
    write('services/use-jobs/use-jobs.ts', SUBSCRIBING_HOOK('useJobEvents', 'jobs', 'Event'));
  });

  it('finds a feature file that calls a subscription hook', () => {
    write(
      'features/thing/index.tsx',
      `import { useJobEvents } from '@/services';\nuseJobEvents(x);`
    );

    expect(discoverSubscribers(['useJobEvents'], renderer)).toEqual(['features/thing/index.tsx']);
  });

  it('does not report the services/ definitions themselves', () => {
    expect(discoverSubscribers(['useJobEvents'], renderer)).toEqual([]);
  });

  it('does not match a longer name that merely contains a hook name', () => {
    write('features/thing/index.tsx', `const useJobEventsFormatter = () => {};`);

    expect(discoverSubscribers(['useJobEvents'], renderer)).toEqual([]);
  });

  it('follows an `as` alias — the rename that would otherwise walk past the guard', () => {
    // A guard that can be stepped around by renaming an import is worse than
    // none: this subscribes exactly as much as the unaliased form.
    write(
      'features/thing/index.tsx',
      `import { useJobEvents as useEvents } from '@/services';\nuseEvents(cb);`
    );

    expect(discoverSubscribers(['useJobEvents'], renderer)).toEqual(['features/thing/index.tsx']);
  });

  it('follows an alias inside a MULTI-LINE import block', () => {
    // The realistic shape — this repo wraps any import of more than a few names,
    // and a `[^}]` class that did not span newlines would miss every one.
    write(
      'features/thing/index.tsx',
      `import {\n  useJobs,\n  useJobEvents as useEvents,\n  useCancelJob,\n} from '@/services';\nuseEvents(cb);`
    );

    expect(discoverSubscribers(['useJobEvents'], renderer)).toEqual(['features/thing/index.tsx']);
  });

  it('ignores a same-named LOCAL function that was never imported', () => {
    // The false-positive direction. Reporting this file would push someone to
    // add an inventory entry for a subscription that does not exist.
    write('features/thing/index.tsx', `function useJobEvents() {}\nuseJobEvents();`);

    expect(discoverSubscribers(['useJobEvents'], renderer)).toEqual([]);
  });

  it('ignores a type-only import, which cannot call anything', () => {
    write(
      'features/thing/index.tsx',
      `import type { useJobEvents } from '@/services';\ndeclare const x: typeof useJobEvents;`
    );

    expect(discoverSubscribers(['useJobEvents'], renderer)).toEqual([]);
  });
});

describe('stripCommentsAndStrings', () => {
  it('preserves length and line structure so match offsets stay valid', () => {
    const src = `const a = 1; // note
/* block */ const b = 2;`;
    const out = stripCommentsAndStrings(src);

    const lines = (text) => text.split(/\r?\n/).length;
    expect(out).toHaveLength(src.length);
    expect(lines(out)).toBe(lines(src));
  });

  it('does not let a // inside a URL string eat the rest of the line', () => {
    // Why this is a scanner and not a regex. A naive `//.*$` strip would delete
    // the real call after the URL — a guard that silently MISSES a subscription,
    // which is the one failure direction it must not have.
    const src = `const u = 'https://x.dev'; api.jobs.onEvent(cb);`;

    expect(stripCommentsAndStrings(src)).toContain('api.jobs.onEvent(');
  });

  it('blanks a comment and a string but keeps the surrounding code', () => {
    const out = stripCommentsAndStrings(`useX(); /* useY() */ const s = 'useZ()';`);

    expect(out).toContain('useX()');
    expect(out).not.toContain('useY()');
    expect(out).not.toContain('useZ()');
  });
});

describe('importedBindings', () => {
  it('returns the local name, not the exported one, for an alias', () => {
    const src = `import { useJobEvents as useEvents, useJobs } from '@/services';`;

    expect([...importedBindings(src, ['useJobEvents', 'useJobs'])].sort()).toEqual([
      'useEvents',
      'useJobs',
    ]);
  });

  it('skips an inline `type` specifier inside a value import', () => {
    const src = `import { type useJobEvents, useJobs } from '@/services';`;

    expect([...importedBindings(src, ['useJobEvents', 'useJobs'])]).toEqual(['useJobs']);
  });
});

describe('namespaceImporters', () => {
  it('reports a services namespace import rather than silently missing it', () => {
    // Nothing does this today. Supporting it means resolving member expressions;
    // until then the blind spot is surfaced, because a guard with SILENT blind
    // spots is theatre.
    write(
      'features/thing/index.tsx',
      `import * as services from '@/services';\nservices.useJobEvents(cb);`
    );

    expect(namespaceImporters(renderer)).toEqual(['features/thing/index.tsx']);
  });

  it('does not report an ordinary named import', () => {
    write('features/thing/index.tsx', `import { useJobEvents } from '@/services';`);

    expect(namespaceImporters(renderer)).toEqual([]);
  });
});

describe('hashBytes', () => {
  it('matches a hand-written expected value, not its own output', () => {
    // 'hello' is a well-known sha256 test vector
    // (2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824), typed
    // in here independently of the implementation — the repo's own lesson from
    // #998-#1003 is that comparing a hash to itself proves nothing.
    expect(hashBytes('hello')).toBe('2cf24dba5fb0');
  });

  it('changes when a single byte changes', () => {
    expect(hashBytes('hello')).not.toBe(hashBytes('hellp'));
  });
});

describe('staleNoteEntries', () => {
  const inventory = {
    'features/thing/index.tsx': {
      mount: 'route-scoped',
      hash: hashBytes('original content'),
      note: 'x',
    },
    'always/thing.ts': { mount: 'always', note: 'no hash — exempt' },
  };

  it('reports nothing when the stored hash matches the file', () => {
    const readFile = () => 'original content';
    expect(staleNoteEntries(inventory, readFile)).toEqual([]);
  });

  it('flags an entry whose subscriber file content changed since the hash was recorded', () => {
    // The mutation this guards against: someone edits the behavior a note
    // describes without re-reading (or updating) the note.
    const readFile = (rel) =>
      rel === 'features/thing/index.tsx' ? 'MUTATED content' : 'original content';
    const stale = staleNoteEntries(inventory, readFile);

    expect(stale).toHaveLength(1);
    expect(stale[0].file).toBe('features/thing/index.tsx');
    expect(stale[0].note).toBe('x');
    expect(stale[0].currentHash).toBe(hashBytes('MUTATED content'));
  });

  it('skips entries with no stored hash (the always-mounted exemption)', () => {
    // The route-scoped entry legitimately drifts here too (readFile always
    // returns something other than 'original content') — the point is that
    // the hash-less always-mounted entry is never even checked, let alone
    // flagged, no matter what its file contains.
    const readFile = () => 'anything at all, never checked';
    const stale = staleNoteEntries(inventory, readFile);

    expect(stale.map((s) => s.file)).not.toContain('always/thing.ts');
  });
});

describe('violations', () => {
  const hooks = ['a', 'b', 'c', 'd', 'e'];
  const subs = ['f1', 'f2', 'f3', 'f4', 'f5'];
  const ok = Object.fromEntries(subs.map((f) => [f, { mount: 'always', note: 'root' }]));

  it('passes when every subscriber is declared', () => {
    expect(violations(ok, hooks, subs)).toEqual([]);
  });

  it('reports an undeclared subscriber by name', () => {
    const missing = Object.fromEntries(Object.entries(ok).filter(([f]) => f !== 'f5'));
    const problems = violations(missing, hooks, subs);

    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('f5');
    expect(problems[0]).toContain('not declared in SUBSCRIBERS');
  });

  it('reports a stale entry for a file that no longer subscribes', () => {
    const problems = violations({ ...ok, gone: { mount: 'always', note: 'x' } }, hooks, subs);

    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('gone');
    expect(problems[0]).toContain('no longer subscribing');
  });

  it('rejects a route-scoped entry whose note explains nothing', () => {
    const lazy = { ...ok, f5: { mount: 'route-scoped', note: 'todo' } };
    const problems = violations(lazy, hooks, subs);

    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('f5');
    expect(problems[0]).toContain('what is dropped');
  });

  it('rejects an unrecognized mount value instead of treating it as always', () => {
    const typo = {
      ...ok,
      f5: { mount: 'root', note: 'a note long enough to pass the length test' },
    };

    expect(violations(typo, hooks, subs)[0]).toContain('f5');
  });

  // ── The vacuity guards ────────────────────────────────────────────────────
  // Every check above is a set comparison, and a set comparison against an empty
  // discovery passes while checking nothing.

  it('fails loudly when hook discovery finds nothing, instead of passing', () => {
    const problems = violations(ok, [], subs);

    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('vacuous');
  });

  it('reports a namespace importer as a blind spot rather than ignoring it', () => {
    const problems = violations(ok, hooks, subs, ['features/thing/index.tsx']);

    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('features/thing/index.tsx');
    expect(problems[0]).toContain('namespace');
  });

  it('fails loudly when subscriber discovery finds nothing', () => {
    const problems = violations(ok, hooks, []);

    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('compared against nothing');
  });

  // ── The hash tripwire ─────────────────────────────────────────────────────

  it('passes staleNotes through untouched when empty', () => {
    expect(violations(ok, hooks, subs, [], [])).toEqual([]);
  });

  it('fails and prints the note + file when a subscriber file drifted from its hash', () => {
    const drifted = [{ file: 'f3', note: 'what f3 loses', currentHash: 'deadbeef0000' }];
    const problems = violations(ok, hooks, subs, [], drifted);

    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('f3');
    expect(problems[0]).toContain('what f3 loses');
    expect(problems[0]).toContain('deadbeef0000');
  });
});

describe('against the real renderer', () => {
  // The synthetic cases above prove the LOGIC. These prove the logic is still
  // pointed at the real source — the failure this guard cannot survive is
  // discovery quietly matching nothing while every assertion stays green.

  const hooks = discoverSubscriptionHooks();
  const subscribers = discoverSubscribers(hooks);

  it('still recognises the real service idiom', () => {
    expect(hooks).toContain('useJobEvents');
    expect(hooks).toContain('useAutopilotStepEvents');
  });

  it('still finds real subscribing files', () => {
    expect(subscribers).toContain('routes/__root.tsx');
  });

  it('holds the invariant', () => {
    expect(violations(SUBSCRIBERS, hooks, subscribers)).toEqual([]);
  });

  it('every route-scoped note hash matches its real subscriber file right now', () => {
    // The end-to-end proof for "with correct hashes, the check passes": this
    // hashes the ACTUAL files on disk and compares against what is committed
    // in SUBSCRIBERS, then runs that result through violations() exactly like
    // the CLI entrypoint does.
    const staleNotes = staleNoteEntries(SUBSCRIBERS);
    expect(staleNotes).toEqual([]);
    expect(violations(SUBSCRIBERS, hooks, subscribers, [], staleNotes)).toEqual([]);
  });

  it('DETECTS a mutated subscriber file end-to-end, via a temp fixture', () => {
    // Proves the tripwire fires through the full violations() pipeline, not
    // just the pure staleNoteEntries() unit above — without touching the real
    // tree. A subscriber file changes; its note (unedited) is now a claim
    // about code that no longer exists, and the guard must say so.
    const inventory = {
      'features/thing/index.tsx': {
        mount: 'route-scoped',
        hash: hashBytes('export function Thing() { return null; }'),
        note: 'a note describing the ORIGINAL behavior, now stale',
      },
    };
    const mutatedRead = () => 'export function Thing() { return <div>mutated!</div>; }';

    const staleNotes = staleNoteEntries(inventory, mutatedRead);
    const problems = violations(inventory, hooks, subscribers, [], staleNotes);

    expect(staleNotes).toHaveLength(1);
    expect(problems.some((p) => p.includes('features/thing/index.tsx'))).toBe(true);
    expect(problems.some((p) => p.includes('now stale'))).toBe(true);
  });
});
