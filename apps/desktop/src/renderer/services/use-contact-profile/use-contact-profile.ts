import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { ContactProfile } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

export const useContactProfile = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.contactProfile.all,
    queryFn: () => api.contactProfile.get(),
  });
};

export const useSaveContactProfile = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (profile: ContactProfile) => api.contactProfile.set(profile),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.contactProfile.all }),
  });
};

// `contactProfile.headerLine` (H, header seeding) has no React-component
// consumer — `generateResume` (a plain async lib function, not a hook context)
// calls the tauri client directly, matching that file's existing pattern for
// every other one-off IPC call it makes. No hook/query-key wraps it: adding
// one with no caller would just be dead surface (removed 2026-08 review).
