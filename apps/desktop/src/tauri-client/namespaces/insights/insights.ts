import { invoke } from '@tauri-apps/api/core';

import type { InsightsOutcomesReport } from '@ajh/shared';

export const insights = {
  outcomes: () => invoke<InsightsOutcomesReport>('insights_outcomes'),
};
