import { useState, useCallback } from 'react';

interface AsyncActionState {
  loading: boolean;
  error: string | null;
}

type AsyncActionResult<T> = AsyncActionState & {
  run: (fn: () => Promise<T>) => Promise<T | undefined>;
  reset: () => void;
};

/**
 * Wraps an async operation with loading/error state tracking.
 * Normalizes IPC errors into human-readable strings.
 *
 * Usage:
 *   const { run, loading, error } = useAsyncAction();
 *
 *   const save = () => run(async () => {
 *     await window.api.credentials.set(data);
 *   });
 */
export function useAsyncAction<T = void>(): AsyncActionResult<T> {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (fn: () => Promise<T>): Promise<T | undefined> => {
    setLoading(true);
    setError(null);
    try {
      const result = await fn();
      return result;
    } catch (err) {
      const message = normalizeError(err);
      setError(message);
      return undefined;
    } finally {
      setLoading(false);
    }
  }, []);

  const reset = useCallback(() => {
    setError(null);
    setLoading(false);
  }, []);

  return { loading, error, run, reset };
}

function normalizeError(err: unknown): string {
  if (typeof err === 'string') return err;
  if (err instanceof Error) return err.message;
  if (err && typeof err === 'object' && 'message' in err) {
    return String((err as { message: unknown }).message);
  }
  return 'An unexpected error occurred.';
}
