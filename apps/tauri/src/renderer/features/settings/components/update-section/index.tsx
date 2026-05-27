import { CheckCircle2, Download, ExternalLink, Loader2, Sparkles } from 'lucide-react';

import { Button, GlassCard, IconBadge, RefreshButton, SectionLabel } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useAppVersion, useOpenExternal } from '@/services';
import { useUpdater } from '@/services/use-updater';

const NO_RELEASE_PATTERNS = [
  'valid release json',
  'release json',
  'failed to fetch',
  'network',
  '404',
];

function isNoReleaseError(msg: string) {
  const lower = msg.toLowerCase();
  return NO_RELEASE_PATTERNS.some((p) => lower.includes(p));
}

export function UpdateSection() {
  const { t } = useTranslation();
  const { data: versionRaw = '' } = useAppVersion();
  const version = versionRaw
    ? String(versionRaw).startsWith('v')
      ? String(versionRaw)
      : `v${versionRaw}`
    : '';
  const { status, check, download, install } = useUpdater();
  const openExternal = useOpenExternal();

  const noRelease = status.state === 'error' && isNoReleaseError(status.message);
  const GITHUB_RELEASES_URL =
    'https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases/latest';

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <IconBadge icon={Sparkles} size="sm" />
        <SectionLabel>{t('settings.update.title')}</SectionLabel>
      </div>

      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs text-foreground/50">{t('settings.update.currentVersion')}</p>
          <p className="mt-0.5 font-mono text-sm text-foreground/80">{version || '—'}</p>
        </div>

        {status.state === 'idle' || status.state === 'not-available' || status.state === 'error' ? (
          <RefreshButton onRefresh={check} variant="glass" size={12} className="shrink-0 gap-2">
            {t('settings.update.checkNow')}
          </RefreshButton>
        ) : status.state === 'checking' ? (
          <div className="flex items-center gap-2 text-xs text-foreground/40">
            <Loader2 size={13} className="animate-spin" />
            {t('settings.update.checking')}
          </div>
        ) : status.state === 'available' ? (
          <Button
            variant="glass"
            size="sm"
            onClick={() => void download()}
            className="gap-2 ring-1 ring-brand/20"
          >
            <Download size={12} />
            {t('settings.update.download', { version: status.version })}
          </Button>
        ) : status.state === 'downloading' ? (
          <div className="flex items-center gap-2 text-xs text-foreground/40">
            <Loader2 size={13} className="animate-spin" />
            {status.percent}%
          </div>
        ) : status.state === 'downloaded' ? (
          <RefreshButton
            onRefresh={install}
            variant="glass"
            size={12}
            className="gap-2 ring-1 ring-brand/20"
          >
            {t('settings.update.install')}
          </RefreshButton>
        ) : null}
      </div>

      {/* Status messages */}
      {(status.state === 'not-available' || noRelease) && (
        <div className="mt-3 flex items-center gap-2 text-xs text-emerald-400/70">
          <CheckCircle2 size={12} />
          {t('settings.update.upToDate')}
        </div>
      )}
      {status.state === 'error' && !noRelease && (
        <div className="mt-3 space-y-2">
          <div className="text-xs text-red-400/70">
            {t('settings.update.error')}: {status.message}
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => openExternal.mutate(GITHUB_RELEASES_URL)}
            className="gap-2 text-xs text-foreground/50 hover:text-foreground/80"
          >
            <ExternalLink size={12} />
            {t('settings.update.downloadFromGitHub')}
          </Button>
        </div>
      )}

      {/* Changelog */}
      {(status.state === 'available' ||
        status.state === 'downloading' ||
        status.state === 'downloaded') &&
        'releaseNotes' in status &&
        status.releaseNotes && (
          <div className="mt-4 space-y-1.5">
            <div className="text-[10px] font-medium uppercase tracking-[0.16em] text-foreground/30">
              {t('settings.update.whatsNew')} {'version' in status ? status.version : ''}
            </div>
            <div className="max-h-36 overflow-y-auto rounded-lg border border-white/[0.05] bg-white/[0.02] p-3 text-xs text-foreground/50 leading-relaxed whitespace-pre-wrap">
              {status.releaseNotes}
            </div>
          </div>
        )}
    </GlassCard>
  );
}
