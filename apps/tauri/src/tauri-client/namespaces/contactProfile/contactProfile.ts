import { invoke } from '@tauri-apps/api/core';

export const contactProfile = {
  get: () => invoke('contact_profile_get'),
  set: (profile: unknown) => invoke('contact_profile_set', { profile }),
};
