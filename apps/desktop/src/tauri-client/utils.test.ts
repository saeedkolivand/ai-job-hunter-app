import { describe, expect, it, vi } from 'vitest';

import { asyncUnsub } from './utils';

describe('asyncUnsub', () => {
  it('cancels the listener once the setup promise resolves', async () => {
    const cancel = vi.fn();
    let resolveSetup: (fn: () => void) => void = () => {};
    const setup = vi.fn(
      () =>
        new Promise<() => void>((resolve) => {
          resolveSetup = resolve;
        })
    );

    const unsub = asyncUnsub(setup);
    expect(setup).toHaveBeenCalledOnce();

    // Resolving after unsub() must immediately cancel.
    unsub();
    resolveSetup(cancel);
    await Promise.resolve();
    expect(cancel).toHaveBeenCalledOnce();
  });

  it('cancels via the returned handle when resolved before unsub', async () => {
    const cancel = vi.fn();
    const unsub = asyncUnsub(() => Promise.resolve(cancel));
    await Promise.resolve();
    expect(cancel).not.toHaveBeenCalled();
    unsub();
    expect(cancel).toHaveBeenCalledOnce();
  });
});
