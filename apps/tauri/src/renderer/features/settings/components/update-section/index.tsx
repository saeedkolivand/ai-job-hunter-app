import {
  CheckCircle2,
  ChevronDown,
  Download,
  ExternalLink,
  History,
  Loader2,
  Sparkles,
} from 'lucide-react';
import { useState } from 'react';

import { compareSemver } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, cn, MarkdownMessage, RefreshButton, SettingsSection } from '@ajh/ui';

import { useAppVersion, useOpenExternal } from '@/services';
import { useChangelog, useUpdater } from '@/services/use-updater';

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
  const [showChangelog, setShowChangelog] = useState(false);
  const currentVersion = String(versionRaw).replace(/^v/, '');

  // An update is in flight (available, downloading, or staged). While it is, we
  // surface the *accumulated* notes — not just the latest tag — so a user several
  // versions behind sees everything they're about to get.
  const updateInProgress =
    status.state === 'available' || status.state === 'downloading' || status.state === 'downloaded';

  // Fetch the release history as soon as an update is in flight (not only when the
  // user expands "View changelog") so the "What's new" box can aggregate it.
  const changelog = useChangelog(showChangelog || updateInProgress);

  // Every stable release strictly newer than the installed version — i.e. the set
  // of changes this update actually delivers. `releases` is newest-first; the
  // newest is the available version, so a `> currentVersion` filter is the range.
  const pendingReleases =
    updateInProgress && currentVersion
      ? (changelog.data?.releases ?? []).filter(
          (r) => !r.prerelease && r.body && compareSemver(r.version, currentVersion) > 0
        )
      : [];

  const noRelease = status.state === 'error' && isNoReleaseError(status.message);
  const GITHUB_RELEASES_URL =
    'https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases/latest';

  return (
    <SettingsSection icon={Sparkles} label={t('settings.update.title')}>
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

      {/* What's new in the available update — every release since the installed
          version, falling back to the latest tag's note when the history is
          unavailable (offline / GitHub error / version unknown). */}
      {updateInProgress &&
        (pendingReleases.length > 0 || ('releaseNotes' in status && status.releaseNotes)) && (
          <div className="mt-4 space-y-1.5">
            <div className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/55">
              {pendingReleases.length > 0 && currentVersion
                ? t('settings.update.changesSince', { version: currentVersion })
                : `${t('settings.update.whatsNew')} ${'version' in status ? status.version : ''}`}
            </div>
            <div className="max-h-60 overflow-y-auto rounded-lg border border-white/[0.05] bg-white/[0.02] p-3">
              {pendingReleases.length > 0 ? (
                <div className="space-y-3">
                  {pendingReleases.map((release) => (
                    <div key={release.version} className="space-y-1">
                      <span className="font-mono text-xs text-foreground/80">
                        v{release.version}
                      </span>
                      <MarkdownMessage
                        content={release.body ?? ''}
                        className="text-xs text-foreground/60"
                        onLinkClick={(url) => openExternal.mutate(url)}
                      />
                    </div>
                  ))}
                </div>
              ) : (
                <MarkdownMessage
                  content={'releaseNotes' in status ? (status.releaseNotes ?? '') : ''}
                  className="text-xs text-foreground/60"
                  onLinkClick={(url) => openExternal.mutate(url)}
                />
              )}
            </div>
          </div>
        )}

      {/* Changelog history (current + previous versions) */}
      <div className="mt-4 border-t border-white/[0.05] pt-3">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setShowChangelog((v) => !v)}
          className="gap-2 text-xs text-foreground/50 hover:text-foreground/80"
        >
          <History size={12} />
          {showChangelog ? t('settings.update.hideChangelog') : t('settings.update.viewChangelog')}
          <ChevronDown
            size={12}
            className={cn('transition-transform', showChangelog && 'rotate-180')}
          />
        </Button>

        {showChangelog && (
          <div className="mt-3 max-h-80 space-y-4 overflow-y-auto pr-1">
            {changelog.isPending && (
              <div className="flex items-center gap-2 text-xs text-foreground/40">
                <Loader2 size={13} className="animate-spin" />
                {t('settings.update.loadingChangelog')}
              </div>
            )}

            {changelog.data?.error && (
              <div className="space-y-2 text-xs text-foreground/50">
                <p>{t('settings.update.changelogError')}</p>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => openExternal.mutate(GITHUB_RELEASES_URL)}
                  className="gap-2 text-foreground/50 hover:text-foreground/80"
                >
                  <ExternalLink size={12} />
                  {t('settings.update.downloadFromGitHub')}
                </Button>
              </div>
            )}

            {!changelog.isPending &&
              !changelog.data?.error &&
              changelog.data?.releases?.length === 0 && (
                <p className="text-xs text-foreground/40">{t('settings.update.changelogEmpty')}</p>
              )}

            {changelog.data?.releases?.map((release) => {
              const isCurrent = release.version === currentVersion;
              return (
                <div key={release.version} className="space-y-1.5">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-xs text-foreground/80">v{release.version}</span>
                    {isCurrent && (
                      <span className="rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-brand-soft">
                        {t('settings.update.current')}
                      </span>
                    )}
                    {release.publishedAt && (
                      <span className="text-[10px] text-foreground/30">
                        {new Date(release.publishedAt).toLocaleDateString()}
                      </span>
                    )}
                  </div>
                  {release.body && (
                    <div className="rounded-lg border border-white/[0.05] bg-white/[0.02] p-3">
                      <MarkdownMessage
                        content={release.body}
                        className="text-xs text-foreground/60"
                        onLinkClick={(url) => openExternal.mutate(url)}
                      />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </SettingsSection>
  );
}
