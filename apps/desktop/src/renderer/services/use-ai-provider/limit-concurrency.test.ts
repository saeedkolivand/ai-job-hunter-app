import { describe, expect, it, vi } from 'vitest';

import { createConcurrencyLimit, LimitAbortError } from './limit-concurrency';

/** A promise plus the handle to settle it, so a test controls the timing. */
function deferred() {
  let resolve!: (v: string) => void;
  let reject!: (e: Error) => void;
  const promise = new Promise<string>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('createConcurrencyLimit', () => {
  it('never runs more than `max` at once', async () => {
    const limit = createConcurrencyLimit(2);
    const gates = [deferred(), deferred(), deferred()];
    let active = 0;
    let peak = 0;

    const runs = gates.map((gate) =>
      limit(async () => {
        active += 1;
        peak = Math.max(peak, active);
        const value = await gate.promise;
        active -= 1;
        return value;
      })
    );

    await Promise.resolve();
    expect(peak).toBe(2);

    gates.forEach((g, i) => g.resolve(`done-${i}`));
    await expect(Promise.all(runs)).resolves.toEqual(['done-0', 'done-1', 'done-2']);
    expect(peak).toBe(2);
  });

  it('never exceeds the cap across a COMPLETION boundary', async () => {
    // The window a count-after-settle assertion cannot see: a task finishes and
    // wakes a waiter, but the waiter only resumes on a later microtask. A call
    // arriving in between must NOT be let through on the slot the waiter has
    // already been promised.
    const limit = createConcurrencyLimit(1);
    let active = 0;
    let peak = 0;
    // A gate every task waits on. `open` also covers tasks that START later,
    // so draining at the end cannot deadlock on a gate created after the drain.
    let open = false;
    const gates: Array<() => void> = [];
    const openAll = () => {
      open = true;
      while (gates.length) gates.shift()?.();
    };
    const run = () =>
      limit(async () => {
        active += 1;
        peak = Math.max(peak, active);
        await new Promise<void>((resolve) => (open ? resolve() : gates.push(resolve)));
        active -= 1;
      });

    const first = run();
    const second = run(); // queued behind `first`

    // Two hops from "gate resolved" to "slot released": the task body resumes
    // and returns (1), then the limiter's own `await fn()` resumes and runs its
    // `finally` (2). The woken waiter resumes on hop 3 — so a call made right
    // here is exactly the one that must not be let through.
    gates.shift()?.();
    await null;
    await null;
    const third = run();

    await null;
    await null;
    expect(peak).toBe(1);

    openAll();
    await Promise.all([first, second, third]);
    expect(peak).toBe(1);
  });

  it('releases the slot when a call REJECTS, so the queue cannot wedge', async () => {
    const limit = createConcurrencyLimit(1);
    const first = limit(() => Promise.reject(new Error('boom')));

    await expect(first).rejects.toThrow('boom');
    // A permanently-held slot would leave this pending forever.
    await expect(limit(() => Promise.resolve('after'))).resolves.toBe('after');
  });

  it('drops a queued call whose caller gave up, and passes the slot on', async () => {
    const limit = createConcurrencyLimit(1);
    const controller = new AbortController();
    const started: string[] = [];
    let openFirst!: () => void;

    const first = limit(async () => {
      started.push('first');
      await new Promise<void>((resolve) => (openFirst = resolve));
    });
    const abandoned = limit(async () => {
      started.push('abandoned');
    }, controller.signal);
    const after = limit(async () => {
      started.push('after');
    });

    controller.abort();
    openFirst();

    await expect(abandoned).rejects.toBeInstanceOf(LimitAbortError);
    await Promise.all([first, after]);

    // The abandoned probe never ran, and its slot went to the next in line
    // instead of wedging the queue.
    expect(started).toEqual(['first', 'after']);
  });

  it('rejects immediately when the caller is already gone', async () => {
    const limit = createConcurrencyLimit(4);
    const fn = vi.fn().mockResolvedValue('x');

    await expect(limit(fn, AbortSignal.abort())).rejects.toBeInstanceOf(LimitAbortError);
    expect(fn).not.toHaveBeenCalled();
  });

  it('runs waiters in FIFO order', async () => {
    const limit = createConcurrencyLimit(1);
    const order: number[] = [];
    const gate = deferred();

    const runs = [
      limit(async () => {
        order.push(0);
        await gate.promise;
      }),
      limit(() => Promise.resolve(order.push(1))),
      limit(() => Promise.resolve(order.push(2))),
    ];

    gate.resolve('go');
    await Promise.all(runs);

    expect(order).toEqual([0, 1, 2]);
  });
});
