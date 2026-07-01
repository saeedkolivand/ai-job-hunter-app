import { invoke } from '@tauri-apps/api/core';

import type { ContactProfile } from '@ajh/shared/ipc';

export const contactProfile = {
  get: () => invoke<ContactProfile>('contact_profile_get'),
  set: (profile: ContactProfile) =>
    invoke<{ success?: boolean; error?: string }>('contact_profile_set', { profile }),
};
