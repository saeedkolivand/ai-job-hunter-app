import { createContext, type ReactNode, useCallback, useContext, useRef, useState } from 'react';

const CONCURRENCY = 1;

interface ScoringSchedulerContextValue {
  activeSet: ReadonlySet<string>;
  enqueue: (jobId: string) => void;
  release: (jobId: string) => void;
  remove: (jobId: string) => void;
}

const ScoringSchedulerContext = createContext<ScoringSchedulerContextValue | null>(null);

export function ScoringSchedulerProvider({ children }: { children: ReactNode }) {
  const queueRef = useRef<string[]>([]); // ordered list of jobIds waiting
  const [activeSet, setActiveSet] = useState<ReadonlySet<string>>(new Set());

  const tryAdvance = useCallback((current: Set<string>) => {
    // activate pending items up to CONCURRENCY limit
    const pending = queueRef.current.filter((id) => !current.has(id));
    const toActivate = pending.slice(0, CONCURRENCY - current.size);
    if (toActivate.length === 0) return current;
    return new Set([...current, ...toActivate]);
  }, []);

  const enqueue = useCallback(
    (jobId: string) => {
      if (queueRef.current.includes(jobId)) return; // idempotent
      queueRef.current = [...queueRef.current, jobId];
      setActiveSet((prev) => {
        const next = new Set(prev);
        return tryAdvance(next);
      });
    },
    [tryAdvance]
  );

  const release = useCallback(
    (jobId: string) => {
      queueRef.current = queueRef.current.filter((id) => id !== jobId);
      setActiveSet((prev) => {
        const next = new Set(prev);
        next.delete(jobId);
        return tryAdvance(next);
      });
    },
    [tryAdvance]
  );

  const remove = useCallback(
    (jobId: string) => {
      queueRef.current = queueRef.current.filter((id) => id !== jobId);
      setActiveSet((prev) => {
        const next = new Set(prev);
        next.delete(jobId);
        return tryAdvance(next);
      });
    },
    [tryAdvance]
  );

  return (
    <ScoringSchedulerContext.Provider value={{ activeSet, enqueue, release, remove }}>
      {children}
    </ScoringSchedulerContext.Provider>
  );
}

export function useScoringScheduler(): ScoringSchedulerContextValue {
  const ctx = useContext(ScoringSchedulerContext);
  if (!ctx) throw new Error('useScoringScheduler must be used within ScoringSchedulerProvider');
  return ctx;
}
