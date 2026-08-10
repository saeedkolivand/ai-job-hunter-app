import { useMutation } from '@tanstack/react-query';

import type { ContentReportPayload } from '@ajh/shared/ipc';
import type { ResumeValidateContentRequest } from '@ajh/shared/schemas';

import { useAppClient } from '@/providers/AppClientProvider';

/**
 * Deterministic content-quality check (factual accuracy, ATS structure,
 * AI-voice tells) on an already-generated résumé/letter — pure and fast, no AI
 * call, safe to call on every save. A mutation, not a query: the input is
 * transient generated text, not identity-addressable server state worth
 * caching (no `keys.resume` entry — nothing here is ever read back by key).
 */
export const useValidateContent = () => {
  const api = useAppClient();
  return useMutation<ContentReportPayload, Error, ResumeValidateContentRequest>({
    mutationFn: (req) => api.resume.validateContent(req),
  });
};
