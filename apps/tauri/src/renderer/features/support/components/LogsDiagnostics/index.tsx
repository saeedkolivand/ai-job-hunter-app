import { Button } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import { LogCategoryCard } from './LogCategoryCard';
import { LogEntry } from './LogEntry';

export function LogsDiagnostics() {
  const { t } = useTranslation();

  return (
    <div className="space-y-6">
      <div className="glass-card rounded-2xl p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">{t('support.logs.logViewer')}</h2>
          <div className="flex gap-2">
            <Button size="sm" variant="ghost" className="text-xs">
              {t('support.logs.filter')}
            </Button>
            <Button size="sm" variant="ghost" className="text-xs">
              {t('support.logs.export')}
            </Button>
            <Button size="sm" variant="ghost" className="text-xs">
              {t('support.logs.clear')}
            </Button>
          </div>
        </div>
        <div className="space-y-2 mb-4">
          <LogEntry
            timestamp={t('support.logs.exampleTimestamp1')}
            level="info"
            source="ai_runtime"
            message={t('support.logs.exampleMessage1')}
          />
          <LogEntry
            timestamp={t('support.logs.exampleTimestamp2')}
            level="warning"
            source="scraping"
            message={t('support.logs.exampleMessage2')}
          />
          <LogEntry
            timestamp={t('support.logs.exampleTimestamp3')}
            level="error"
            source="document_parser"
            message={t('support.logs.exampleMessage3')}
          />
          <LogEntry
            timestamp={t('support.logs.exampleTimestamp4')}
            level="info"
            source="indexing"
            message={t('support.logs.exampleMessage4')}
          />
          <LogEntry
            timestamp={t('support.logs.exampleTimestamp5')}
            level="info"
            source="embeddings"
            message={t('support.logs.exampleMessage5')}
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">{t('support.logs.logCategories')}</h2>
        <div className="grid gap-4 md:grid-cols-3">
          <LogCategoryCard
            name={t('support.logs.aiRuntime')}
            count={1247}
            errors={3}
            warnings={12}
          />
          <LogCategoryCard name={t('support.logs.scraping')} count={892} errors={8} warnings={45} />
          <LogCategoryCard
            name={t('support.logs.documentParser')}
            count={456}
            errors={2}
            warnings={7}
          />
          <LogCategoryCard name={t('support.logs.indexing')} count={634} errors={0} warnings={5} />
          <LogCategoryCard name={t('support.logs.ocr')} count={234} errors={1} warnings={8} />
          <LogCategoryCard
            name={t('support.logs.workerPool')}
            count={567}
            errors={0}
            warnings={3}
          />
        </div>
      </div>
    </div>
  );
}
