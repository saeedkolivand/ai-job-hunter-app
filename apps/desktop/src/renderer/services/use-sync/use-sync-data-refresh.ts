import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { keys } from '../query-client';
import { useSyncStatus } from './use-sync';

/** Mount in the app root so received records refresh every open route. */
export const useSyncDataRefresh = () => {
  const qc = useQueryClient();
  const { data } = useSyncStatus();
  const applied = data?.lastReport?.applied ?? 0;
  const syncedAt = data?.lastSyncAt;

  useEffect(() => {
    if (!syncedAt || applied === 0) return;
    // Refresh local-store reads, without invalidating AI probes, credentials
    // or model-list queries just because sync wrote a record.
    const prefixes = [
      keys.applications.all,
      keys.documents.all,
      keys.aiGenerations.all,
      keys.contactProfile.all,
      keys.jobPreferences.all,
      keys.ai.activeConfig,
      keys.ai.stageOverrides,
      keys.ai.spend,
      keys.autopilot.all,
      keys.postings.all,
      keys.referrals.all,
      keys.discovery.all,
      keys.pipeline.all,
    ];
    for (const queryKey of prefixes) void qc.invalidateQueries({ queryKey });
  }, [qc, syncedAt, applied]);
};
