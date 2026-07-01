import { useTranslation } from '@ajh/translations';

import {
  useClearEmbeddingsCache,
  useClearOcrCache,
  useClearScrapingQueue,
  useRebuildVectorIndexes,
  useReindexAllDocuments,
  useReloadAiRuntime,
  useResetAllSessions,
  useResetModelConfiguration,
  useResetVectorDatabase,
  useUnloadAllModels,
} from '@/services';

import { RecoveryAction } from '../RecoveryAction';

export function RecoveryTools() {
  const { t } = useTranslation();

  const rebuildIndexes = useRebuildVectorIndexes();
  const clearCache = useClearEmbeddingsCache();
  const resetDatabase = useResetVectorDatabase();
  const reloadRuntime = useReloadAiRuntime();
  const unloadModels = useUnloadAllModels();
  const resetConfig = useResetModelConfiguration();
  const clearOcrCache = useClearOcrCache();
  const reindexDocs = useReindexAllDocuments();
  const resetSessions = useResetAllSessions();
  const clearQueue = useClearScrapingQueue();

  return (
    <div className="space-y-6">
      <div className="glass-card rounded-2xl p-6 border-amber-400/20 bg-amber-400/5">
        <h2 className="text-lg font-semibold mb-2 text-amber-200">
          {t('support.recovery.destructiveActions')}
        </h2>
        <p className="text-sm text-foreground/55 mb-4">
          {t('support.recovery.destructiveActionsDesc')}
        </p>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">
          {t('support.recovery.vectorDatabaseRecovery')}
        </h2>
        <div className="space-y-3">
          <RecoveryAction
            title={t('support.recovery.rebuildIndexesTitle')}
            description={t('support.recovery.rebuildIndexesDesc')}
            destructive={true}
            action="rebuildIndexes"
            successMessage="Vector indexes rebuilt."
            onAction={() => void rebuildIndexes.mutateAsync()}
          />
          <RecoveryAction
            title={t('support.recovery.clearCacheTitle')}
            description={t('support.recovery.clearCacheDesc')}
            destructive={true}
            action="clearCache"
            successMessage="Embeddings cache cleared."
            onAction={() => void clearCache.mutateAsync()}
          />
          <RecoveryAction
            title={t('support.recovery.resetDatabaseTitle')}
            description={t('support.recovery.resetDatabaseDesc')}
            destructive={true}
            action="resetDatabase"
            successMessage="Vector database reset."
            onAction={() => void resetDatabase.mutateAsync()}
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">{t('support.recovery.aiRuntimeRecovery')}</h2>
        <div className="space-y-3">
          <RecoveryAction
            title={t('support.recovery.reloadRuntimeTitle')}
            description={t('support.recovery.reloadRuntimeDesc')}
            destructive={false}
            action="reloadRuntime"
            successMessage="AI runtime reloaded."
            onAction={() => void reloadRuntime.mutateAsync()}
          />
          <RecoveryAction
            title={t('support.recovery.unloadModelsTitle')}
            description={t('support.recovery.unloadModelsDesc')}
            destructive={false}
            action="unloadModels"
            successMessage="All models unloaded."
            onAction={() => void unloadModels.mutateAsync()}
          />
          <RecoveryAction
            title={t('support.recovery.resetConfigTitle')}
            description={t('support.recovery.resetConfigDesc')}
            destructive={false}
            action="resetConfig"
            successMessage="Model configuration reset."
            onAction={() => void resetConfig.mutateAsync()}
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">{t('support.recovery.documentOcrRecovery')}</h2>
        <div className="space-y-3">
          <RecoveryAction
            title={t('support.recovery.clearOcrCacheTitle')}
            description={t('support.recovery.clearOcrCacheDesc')}
            destructive={true}
            action="clearOcrCache"
            successMessage="OCR cache cleared."
            onAction={() => void clearOcrCache.mutateAsync()}
          />
          <RecoveryAction
            title={t('support.recovery.reindexDocsTitle')}
            description={t('support.recovery.reindexDocsDesc')}
            destructive={false}
            action="reindexDocs"
            successMessage="Documents reindexed."
            onAction={() => void reindexDocs.mutateAsync()}
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">{t('support.recovery.scrapingRecovery')}</h2>
        <div className="space-y-3">
          <RecoveryAction
            title={t('support.recovery.resetSessionsTitle')}
            description={t('support.recovery.resetSessionsDesc')}
            destructive={true}
            action="resetSessions"
            successMessage="All sessions reset."
            onAction={() => void resetSessions.mutateAsync()}
          />
          <RecoveryAction
            title={t('support.recovery.clearQueueTitle')}
            description={t('support.recovery.clearQueueDesc')}
            destructive={false}
            action="clearQueue"
            successMessage="Scraping queue cleared."
            onAction={() => void clearQueue.mutateAsync()}
          />
        </div>
      </div>
    </div>
  );
}
