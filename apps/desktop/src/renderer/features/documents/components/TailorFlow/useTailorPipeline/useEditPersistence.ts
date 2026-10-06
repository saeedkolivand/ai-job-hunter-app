import { useCallback, useEffect, useRef } from 'react';

import { PERSIST_DEBOUNCE_MS } from '@/lib/generate';
import { useUpdateAiGeneration } from '@/services/use-ai-generations';

type EditField = 'resume' | 'cover';

/**
 * Debounced write-through of a hand-edit to the job's `ai_generations` aggregate.
 *
 * CR-2: unmount previously CLEARED these timers without flushing — leaving
 * a tab (or a host remount of `DocumentsTab`) inside the debounce window
 * silently dropped the hand-edit; the host's overrides die with the unmount
 * too, so nothing recovers it. Matches DocumentsTab's own `flushJd` posture
 * (`ApplicationDetailPage/index.tsx`) exactly: the pending PAYLOAD lives in a
 * ref (captured at schedule time, not re-read at flush time), one
 * `flushPersist` function is the sole place a write is fired — the debounce
 * timeout and the unmount cleanup both just call it — and a ref indirection
 * keeps the unmount effect's empty deps honest without a stale closure over
 * `flushPersist`/`mutate`.
 */
export function useEditPersistence() {
  const updateAiGeneration = useUpdateAiGeneration();
  const persistTimers = useRef<Partial<Record<EditField, ReturnType<typeof setTimeout>>>>({});
  const pendingEdits = useRef<Partial<Record<EditField, { id: string; text: string }>>>({});
  const mutateAiGenerationRef = useRef(updateAiGeneration.mutate);
  mutateAiGenerationRef.current = updateAiGeneration.mutate;

  const flushPersist = useCallback((field: EditField) => {
    const timer = persistTimers.current[field];
    if (timer) {
      clearTimeout(timer);
      persistTimers.current[field] = undefined;
    }
    const pending = pendingEdits.current[field];
    if (!pending) return;
    pendingEdits.current[field] = undefined;
    mutateAiGenerationRef.current(
      field === 'resume'
        ? { id: pending.id, resumeText: pending.text }
        : { id: pending.id, coverLetterText: pending.text }
    );
  }, []);

  const flushPersistRef = useRef(flushPersist);
  flushPersistRef.current = flushPersist;
  useEffect(
    () => () => {
      flushPersistRef.current('resume');
      flushPersistRef.current('cover');
    },
    []
  );

  return (field: EditField, id: string, text: string) => {
    // Captured NOW, read at flush time (debounce fire OR unmount) instead of
    // closing over `text`/`id` — mirrors DocumentsTab's `pendingJd` exactly.
    pendingEdits.current[field] = { id, text };
    const existing = persistTimers.current[field];
    if (existing) clearTimeout(existing);
    persistTimers.current[field] = setTimeout(() => flushPersist(field), PERSIST_DEBOUNCE_MS);
  };
}
