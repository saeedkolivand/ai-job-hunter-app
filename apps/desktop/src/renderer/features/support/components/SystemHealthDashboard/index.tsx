import { useTranslation } from '@ajh/translations';

import { HealthCard } from '../HealthCard';

export function SystemHealthDashboard() {
  const { t } = useTranslation();

  return (
    <div className="@container space-y-6">
      <div className="grid gap-6 @sm:grid-cols-2 @lg:grid-cols-3">
        <HealthCard
          name={t('support.systemHealth.ollamaRuntime')}
          status="healthy"
          description={t('support.systemHealth.ollamaRuntimeDesc')}
        />
        <HealthCard
          name={t('support.systemHealth.embeddingsEngine')}
          status="healthy"
          description={t('support.systemHealth.embeddingsEngineDesc')}
        />
        <HealthCard
          name={t('support.systemHealth.sqliteDatabase')}
          status="healthy"
          description={t('support.systemHealth.sqliteDatabaseDesc')}
        />
        <HealthCard
          name={t('support.systemHealth.lanceDb')}
          status="warning"
          description={t('support.systemHealth.lanceDbDesc')}
        />
        <HealthCard
          name={t('support.systemHealth.ocrEngine')}
          status="healthy"
          description={t('support.systemHealth.ocrEngineDesc')}
        />
        <HealthCard
          name={t('support.systemHealth.scrapingRuntime')}
          status="healthy"
          description={t('support.systemHealth.scrapingRuntimeDesc')}
        />
        <HealthCard
          name={t('support.systemHealth.fileWatcher')}
          status="healthy"
          description={t('support.systemHealth.fileWatcherDesc')}
        />
        <HealthCard
          name={t('support.systemHealth.workerPool')}
          status="healthy"
          description={t('support.systemHealth.workerPoolDesc')}
        />
        <HealthCard
          name={t('support.systemHealth.vectorIndex')}
          status="healthy"
          description={t('support.systemHealth.vectorIndexDesc')}
        />
      </div>
    </div>
  );
}
