import { useMemo } from 'react';

import { useTranslation } from '@ajh/translations';

/**
 * Shared job-kind → localized label map.
 *
 * Lives outside any feature so it can be consumed by `services/` (passed in as
 * a param to keep services i18n-free), the footer (`components/layout`), the
 * dashboard, and monitoring — without any cross-feature import.
 */
export function useKindLabelMap(): Record<string, string> {
  const { t } = useTranslation();
  return useMemo(
    () => ({
      'ai.generate': t('monitoring.jobKinds.aiGenerate'),
      'ai.embed': t('monitoring.jobKinds.aiEmbed'),
      'document.import': t('monitoring.jobKinds.documentImport'),
      'document.ocr': t('monitoring.jobKinds.documentOcr'),
      'document.chunk': t('monitoring.jobKinds.documentChunk'),
      'document.index': t('monitoring.jobKinds.documentIndex'),
      'scrape.board': t('monitoring.jobKinds.scrapeBoard'),
      'scrape.url': t('monitoring.jobKinds.scrapeUrl'),
      'persist.job': t('monitoring.jobKinds.persistJob'),
      'match.resume': t('monitoring.jobKinds.matchResume'),
      'autopilot.run': t('monitoring.jobKinds.autopilotRun'),
    }),
    [t]
  );
}
