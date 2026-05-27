import { useTranslation } from '@/lib/i18n';

import { DiagnosticItem } from '../DiagnosticItem';
import { IssueCard } from '../IssueCard';
import { ProviderCard } from '../ProviderCard';

export function ScrapingDiagnostics() {
  const { t } = useTranslation();
  return (
    <div className="space-y-6">
      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">{t('support.diagnostics.providerHealth')}</h2>
        <div className="space-y-4">
          <ProviderCard
            name={t('support.diagnostics.providerLinkedIn')}
            status="healthy"
            lastScrape={t('support.diagnostics.lastScrape2HoursAgo')}
            successRate="98%"
            responseTime="1.2s"
          />
          <ProviderCard
            name={t('support.diagnostics.providerIndeed')}
            status="healthy"
            lastScrape={t('support.diagnostics.lastScrape1HourAgo')}
            successRate="95%"
            responseTime="0.8s"
          />
          <ProviderCard
            name={t('support.diagnostics.providerGreenhouse')}
            status="warning"
            lastScrape={t('support.diagnostics.lastScrape5HoursAgo')}
            successRate="82%"
            responseTime="2.5s"
            issue={t('support.diagnostics.rateLimitingDetected')}
          />
          <ProviderCard
            name={t('support.diagnostics.providerWorkday')}
            status="healthy"
            lastScrape={t('support.diagnostics.lastScrape3HoursAgo')}
            successRate="94%"
            responseTime="1.8s"
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">
          {t('support.diagnostics.scrapingSessionStatus')}
        </h2>
        <div className="space-y-4">
          <DiagnosticItem
            name={t('support.diagnostics.sessionTokens')}
            status="healthy"
            description={t('support.diagnostics.sessionTokensDesc')}
          />
          <DiagnosticItem
            name={t('support.diagnostics.cookieStorage')}
            status="healthy"
            description={t('support.diagnostics.cookieStorageDesc')}
          />
          <DiagnosticItem
            name={t('support.diagnostics.rateLimitTracker')}
            status="healthy"
            description={t('support.diagnostics.rateLimitTrackerDesc')}
          />
          <DiagnosticItem
            name={t('support.diagnostics.proxyConfiguration')}
            status="disabled"
            description={t('support.diagnostics.proxyConfigurationDesc')}
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">
          {t('support.diagnostics.commonScrapingIssues')}
        </h2>
        <div className="space-y-3">
          <IssueCard
            title={t('support.diagnostics.rateLimitingIssue')}
            solutions={[
              t('support.diagnostics.rateLimitingSolution1'),
              t('support.diagnostics.rateLimitingSolution2'),
              t('support.diagnostics.rateLimitingSolution3'),
              t('support.diagnostics.rateLimitingSolution4'),
            ]}
          />
          <IssueCard
            title={t('support.diagnostics.captchaIssue')}
            solutions={[
              t('support.diagnostics.captchaSolution1'),
              t('support.diagnostics.captchaSolution2'),
              t('support.diagnostics.captchaSolution3'),
              t('support.diagnostics.captchaSolution4'),
            ]}
          />
          <IssueCard
            title={t('support.diagnostics.sessionExpiredIssue')}
            solutions={[
              t('support.diagnostics.sessionExpiredSolution1'),
              t('support.diagnostics.sessionExpiredSolution2'),
              t('support.diagnostics.sessionExpiredSolution3'),
              t('support.diagnostics.sessionExpiredSolution4'),
            ]}
          />
        </div>
      </div>
    </div>
  );
}
