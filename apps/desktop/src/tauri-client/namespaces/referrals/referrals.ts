import { invoke } from '@tauri-apps/api/core';

import type { ReferralUpsertRequest } from '@ajh/shared';

export const referrals = {
  list: (jobUrl?: string) => invoke('referrals_list', { jobUrl }),
  upsert: (req: ReferralUpsertRequest) => invoke('referrals_upsert', { req }),
  remove: (id: string) => invoke('referrals_remove', { id }),
};
