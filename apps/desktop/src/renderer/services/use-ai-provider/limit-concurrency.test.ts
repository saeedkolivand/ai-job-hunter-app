import { describe, expect, it } from 'vitest';

import { createConcurrencyLimit } from './limit-concurrency';

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

  it('releases the slot when a call REJECTS, so the queue cannot wedge', async () => {
    const limit = createConcurrencyLimit(1);
    const first = limit(() => Promise.reject(new Error('boom')));

    await expect(first).rejects.toThrow('boom');
    // A permanently-held slot would leave this pending forever.
    await expect(limit(() => Promise.resolve('after'))).resolves.toBe('after');
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
