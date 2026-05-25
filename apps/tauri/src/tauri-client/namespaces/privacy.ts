import { invoke } from '@tauri-apps/api/core';

export const privacy = {
  signOutAll: () => invoke('privacy_sign_out_all'),
  clearInteractions: () => invoke('privacy_clear_interactions'),
};
