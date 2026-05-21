import { useTranslation } from '@/lib/i18n';

import { DiagnosticItem } from './DiagnosticItem';
import { IssueCard } from './IssueCard';

export function AIRuntimeDiagnostics() {
  const { t } = useTranslation();

  return (
    <div className="space-y-6">
      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">{t('support.aiRuntime.ollamaRuntimeStatus')}</h2>
        <div className="space-y-4">
          <DiagnosticItem
            name={t('support.aiRuntime.ollamaInstallation')}
            status="healthy"
            description={t('support.aiRuntime.ollamaInstallationDesc')}
          />
          <DiagnosticItem
            name={t('support.aiRuntime.ollamaService')}
            status="healthy"
            description={t('support.aiRuntime.ollamaServiceDesc')}
          />
          <DiagnosticItem
            name={t('support.aiRuntime.defaultModel')}
            status="healthy"
            description={t('support.aiRuntime.defaultModelDesc')}
          />
          <DiagnosticItem
            name={t('support.aiRuntime.gpuAcceleration')}
            status="warning"
            description={t('support.aiRuntime.gpuAccelerationDesc')}
            action={t('support.aiRuntime.enableGpu')}
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">
          {t('support.aiRuntime.modelLoadingDiagnostics')}
        </h2>
        <div className="space-y-4">
          <DiagnosticItem
            name={t('support.aiRuntime.contextWindow')}
            status="healthy"
            description={t('support.aiRuntime.contextWindowDesc')}
          />
          <DiagnosticItem
            name={t('support.aiRuntime.memoryUsage')}
            status="healthy"
            description={t('support.aiRuntime.memoryUsageDesc')}
          />
          <DiagnosticItem
            name={t('support.aiRuntime.modelLoadingTime')}
            status="healthy"
            description={t('support.aiRuntime.modelLoadingTimeDesc')}
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">{t('support.aiRuntime.commonIssues')}</h2>
        <div className="space-y-3">
          <IssueCard
            title={t('support.aiRuntime.modelFailedToLoadInsufficientMemory')}
            solutions={[
              t('support.aiRuntime.modelFailedToLoadSolution1'),
              t('support.aiRuntime.modelFailedToLoadSolution2'),
              t('support.aiRuntime.modelFailedToLoadSolution3'),
              t('support.aiRuntime.modelFailedToLoadSolution4'),
            ]}
          />
          <IssueCard
            title={t('support.aiRuntime.ollamaNotResponding')}
            solutions={[
              t('support.aiRuntime.ollamaNotRespondingSolution1'),
              t('support.aiRuntime.ollamaNotRespondingSolution2'),
              t('support.aiRuntime.ollamaNotRespondingSolution3'),
              t('support.aiRuntime.ollamaNotRespondingSolution4'),
            ]}
          />
          <IssueCard
            title={t('support.aiRuntime.timeoutDuringGeneration')}
            solutions={[
              t('support.aiRuntime.timeoutDuringGenerationSolution1'),
              t('support.aiRuntime.timeoutDuringGenerationSolution2'),
              t('support.aiRuntime.timeoutDuringGenerationSolution3'),
              t('support.aiRuntime.timeoutDuringGenerationSolution4'),
            ]}
          />
        </div>
      </div>
    </div>
  );
}
