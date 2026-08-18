import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import {
  discoverSubscribers,
  discoverSubscriptionHooks,
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

  it('fails loudly when subscriber discovery finds nothing', () => {
    const problems = violations(ok, hooks, []);

    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain('compared against nothing');
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
});
