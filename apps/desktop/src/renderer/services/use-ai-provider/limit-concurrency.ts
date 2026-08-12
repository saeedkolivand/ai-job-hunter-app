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
export function createConcurrencyLimit(max: number): <T>(fn: () => Promise<T>) => Promise<T> {
  let active = 0;
  const waiting: Array<() => void> = [];

  const release = () => {
    active -= 1;
    waiting.shift()?.();
  };

  return async <T>(fn: () => Promise<T>): Promise<T> => {
    if (active >= max) await new Promise<void>((resolve) => waiting.push(resolve));
    active += 1;
    try {
      return await fn();
    } finally {
      release();
    }
  };
}
