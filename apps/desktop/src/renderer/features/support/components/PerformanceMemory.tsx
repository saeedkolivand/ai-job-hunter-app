import { useTranslation } from '@/lib/i18n';
import { MemoryBar } from './MemoryBar';
import { MetricCard } from './MetricCard';
import { OptimizationCard } from './OptimizationCard';

export function PerformanceMemory() {
  const { t } = useTranslation();
  return (
    <div className="space-y-6">
      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">{t('support.performance.memoryUsage')}</h2>
        <div className="space-y-4">
          <MemoryBar
            label={t('support.performance.ramUsage')}
            used="4.2GB"
            total="16GB"
            percentage={26}
            status="healthy"
          />
          <MemoryBar
            label={t('support.performance.modelMemory')}
            used="2.4GB"
            total="8GB"
            percentage={30}
            status="healthy"
          />
          <MemoryBar
            label={t('support.performance.vectorDatabase')}
            used="1.8GB"
            total="4GB"
            percentage={45}
            status="warning"
          />
          <MemoryBar
            label={t('support.performance.embeddingsCache')}
            used="512MB"
            total="2GB"
            percentage={25}
            status="healthy"
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">{t('support.performance.systemResources')}</h2>
        <div className="grid gap-4 md:grid-cols-2">
          <MetricCard
            label={t('support.performance.documentsIndexed')}
            value="1,247"
            trend="+23"
            status="healthy"
          />
          <MetricCard
            label={t('support.performance.embeddingsGenerated')}
            value="8,432"
            trend="+156"
            status="healthy"
          />
          <MetricCard
            label={t('support.performance.activeWorkers')}
            value="2"
            trend="0"
            status="healthy"
          />
          <MetricCard
            label={t('support.performance.queueSize')}
            value="0"
            trend="-5"
            status="healthy"
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">
          {t('support.performance.optimizationRecommendations')}
        </h2>
        <div className="space-y-3">
          <OptimizationCard
            title={t('support.performance.unloadInactiveModels')}
            description={t('support.performance.unloadInactiveModelsDesc')}
            action={t('support.performance.unloadModels')}
            priority="high"
          />
          <OptimizationCard
            title={t('support.performance.rebuildVectorIndexes')}
            description={t('support.performance.rebuildVectorIndexesDesc')}
            action={t('support.performance.rebuildIndexes')}
            priority="medium"
          />
          <OptimizationCard
            title={t('support.performance.clearEmbeddingsCache')}
            description={t('support.performance.clearEmbeddingsCacheDesc')}
            action={t('support.performance.clearCache')}
            priority="low"
          />
        </div>
      </div>
    </div>
  );
}
