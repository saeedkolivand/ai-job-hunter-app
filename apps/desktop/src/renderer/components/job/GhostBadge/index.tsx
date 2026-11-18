import { Clock, Repeat2 } from 'lucide-react';

import type { AutopilotGhostSignals } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Tag } from '@ajh/ui';

/**
 * Muted, advisory chips for a posting's ghost-job signals ("open for a long
 * time" / "reposted"). Renders nothing when neither flag is set. A hint, never
 * a verdict, so the tooltip says so.
 *
 * Non-interactive by design: every host row is itself a `<Button>`, so a
 * focusable popover trigger here would nest buttons. The meaning rides a native
 * `title` plus an sr-only suffix (same precedent as `TrustBadge interactive={false}`).
 */
export function GhostBadge({ signals }: { signals?: AutopilotGhostSignals }) {
  const { t } = useTranslation();
  if (!signals || (!signals.openLong && !signals.reposted)) return null;

  const chips = [
    signals.openLong && {
      key: 'openLong',
      icon: <Clock size={12} aria-hidden="true" />,
      label: t('jobs.ghost.openLong', { days: signals.openDays }),
      detail: t('jobs.ghost.openLongDetail', { days: signals.openDays }),
    },
    signals.reposted && {
      key: 'reposted',
      icon: <Repeat2 size={12} aria-hidden="true" />,
      label: t('jobs.ghost.reposted', { times: signals.repostCount }),
      detail: t('jobs.ghost.repostedDetail', { times: signals.repostCount }),
    },
  ].filter((chip) => chip !== false);

  return (
    <>
      {chips.map(({ key, icon, label, detail }) => {
        const title = `${detail} ${t('jobs.ghost.hint')}`;
        return (
          <span key={key} className="inline-flex items-center" title={title}>
            <Tag color="default" icon={icon} className="rounded-full px-2 py-0.5 text-[11px]">
              {label}
            </Tag>
            <span className="sr-only">: {title}</span>
          </span>
        );
      })}
    </>
  );
}
