import { useEffect, useRef, useState } from 'react';

import type { AiGenerationRecord } from '@ajh/shared/ipc';

import { PERSIST_DEBOUNCE_MS } from '@/lib/generate';
import { useUpdateAiGeneration } from '@/services/use-ai-generations';

/**
 * Local editing buffers keep typing smooth and own the edit truth for the card's
 * lifetime. The card is keyed by `gen.id` at the list (it remounts per record),
 * so drafts are seeded once from the record on mount; thereafter the optimistic
 * update hook patches the list cache (with rollback on failure) and we do NOT
 * re-sync the drafts from `gen`. A re-sync here would let the post-`onSettled`
 * refetch overwrite the buffer with debounce-stale text, clobbering keystrokes
 * typed during the 800ms debounce window.
 */
export function useGenerationDrafts(gen: AiGenerationRecord) {
  const updateAiGeneration = useUpdateAiGeneration();
  const [resumeDraft, setResumeDraft] = useState(gen.resumeText);
  const [coverDraft, setCoverDraft] = useState(gen.coverLetterText);

  // Debounced persistence — one timer per field; cleared on unmount.
  const persistTimers = useRef<{
    resume?: ReturnType<typeof setTimeout>;
    cover?: ReturnType<typeof setTimeout>;
  }>({});
  useEffect(() => {
    const timers = persistTimers.current;
    return () => {
      if (timers.resume) clearTimeout(timers.resume);
      if (timers.cover) clearTimeout(timers.cover);
    };
  }, []);

  const onEdit = (type: 'resume' | 'cover', text: string) => {
    if (type === 'resume') setResumeDraft(text);
    else setCoverDraft(text);
    const existing = persistTimers.current[type];
    if (existing) clearTimeout(existing);
    persistTimers.current[type] = setTimeout(() => {
      updateAiGeneration.mutate(
        type === 'resume' ? { id: gen.id, resumeText: text } : { id: gen.id, coverLetterText: text }
      );
    }, PERSIST_DEBOUNCE_MS);
  };

  return { resumeDraft, coverDraft, onEdit };
}
