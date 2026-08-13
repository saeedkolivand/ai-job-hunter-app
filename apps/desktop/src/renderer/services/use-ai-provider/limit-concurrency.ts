/**
 * A minimal in-flight cap for fan-out queries.
 *
 * `useQueries` has no concurrency option — it starts every query at once. That
 * is fine against a cloud API and rude against a local Ollama server, which is
 * also serving the user's generation: twenty installed models means twenty
 * simultaneous `/api/show` loads on the same process.
 *
 * FIFO, so the first model in the list is also the first answered.
 */

/** Rejection used when a queued call is abandoned before it ever started. */
export class LimitAbortError extends Error {
  constructor() {
    super('Queued call aborted before it started');
    this.name = 'LimitAbortError';
  }
}

export function createConcurrencyLimit(
  max: number
): <T>(fn: () => Promise<T>, signal?: AbortSignal) => Promise<T> {
  let active = 0;
  const waiting: Array<() => void> = [];

  /**
   * Hand the finished task's slot to the next waiter WITHOUT releasing it.
   *
   * Decrementing first was a real over-subscription: the woken waiter only
   * increments when its microtask resumes, so every call arriving in that
   * window saw a free slot and started too — two probes under a cap of one.
   * The slot therefore stays counted; only an empty queue gives it back.
   */
  const release = () => {
    const next = waiting.shift();
    if (next) next();
    else active -= 1;
  };

  return async <T>(fn: () => Promise<T>, signal?: AbortSignal): Promise<T> => {
    if (signal?.aborted) throw new LimitAbortError();

    if (active >= max) {
      // No `active += 1` on this path: `release` kept the slot reserved for
      // whichever waiter it woke, which is this one.
      await new Promise<void>((resolve) => waiting.push(resolve));
      // Queued work can sit here for a while. If the caller gave up meanwhile
      // (the advisor closed, so React Query aborted its query), don't spend the
      // slot on a result nobody will read — pass it straight on.
      if (signal?.aborted) {
        release();
        throw new LimitAbortError();
      }
    } else {
      active += 1;
    }

    try {
      return await fn();
    } finally {
      release();
    }
  };
}
