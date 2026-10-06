import { Bookmark, CircleCheck, Copy, ExternalLink, Eye, MapPin, Save, Wand2 } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { ActionMenu, Button, SourceBadge, Tag, transition } from '@ajh/ui';

import { AgencyChip } from '@/components/job/AgencyChip';
import { ClusterSourceChips } from '@/components/job/ClusterSourceChips';
import { RowMatchScore } from '@/features/jobs/components/RowMatchScore';
import type { usePostingActions } from '@/features/jobs/hooks/usePostingActions';
import { getWorkTypeBadge } from '@/features/jobs/lib/work-type-badge';
import type { Posting } from '@/features/jobs/types';
import { TrustBadge } from '@/lib/trust-badge';

type Actions = Pick<
  ReturnType<typeof usePostingActions>,
  | 'has'
  | 'saved'
  | 'pending'
  | 'handleView'
  | 'handleSave'
  | 'handleTailor'
  | 'handleOpen'
  | 'handleCopyLink'
>;

// Shared className for status Tag pills — applied/saved in the header.
const statusTagCls = 'rounded-full px-1.5 py-0.5 text-fine-print uppercase tracking-wider';

export function DetailHeader({
  posting,
  formatRelativeTime,
  actions: {
    has,
    saved,
    pending,
    handleView,
    handleSave,
    handleTailor,
    handleOpen,
    handleCopyLink,
  },
}: {
  posting: Posting;
  formatRelativeTime: (timestamp?: number) => string;
  actions: Actions;
}) {
  const { t } = useTranslation();
  const workTypeBadge = getWorkTypeBadge(posting);

  return (
    /* Header — flush with hairline bottom divider; no outer card margin.
       `@container`: the header decides its own layout from the DETAIL PANE's
       width, not the viewport (docs/PATTERNS.md §15) — the pane is a
       width-varying panel, so a viewport breakpoint would be wrong here.
       Note the container is THIS element, so `@2xl` (42rem) is measured
       against its CONTENT box — the pane minus this `px-5`. Measured: the
       content box reaches 714px (42rem at the app's 17px rem root) when the
       pane is ~757px, and that is where the action cluster below goes
       inline. */
    <div className="@container shrink-0 border-b border-[var(--border-clear)] px-5 pb-4 pt-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        {/* LEFT: title + meta + match score + status tags */}
        <div className="min-w-0 flex-1">
          <h2 className="text-body-strong text-foreground/95">{posting.title}</h2>
          {/* fold 10: bump metadata row from /60 to /70 (contrast floor at <14px) */}
          <div className="mt-1 flex flex-wrap items-center gap-2 text-fine-print text-foreground/70">
            <span className="font-semibold text-foreground/80">{posting.company}</span>
            {posting.location && (
              <span className="flex items-center gap-1">
                <MapPin size={9} /> {posting.location}
              </span>
            )}
            {workTypeBadge && (
              <Tag color={workTypeBadge.color} className={statusTagCls}>
                {t(workTypeBadge.key)}
              </Tag>
            )}
            <span role="presentation">
              <SourceBadge source={posting.source} url={posting.url} />
            </span>
            {posting.isAgency && <AgencyChip className={statusTagCls} />}
            <ClusterSourceChips
              members={posting.clusterMembers}
              selfKey={posting.clusterId}
              selfUrl={posting.url}
            />
            {posting.postedAt && <span>· {formatRelativeTime(posting.postedAt)}</span>}
          </div>
          <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
            <RowMatchScore jobId={posting.id} />
            {/* Status badges — viewed + applied + saved */}
            {(has('opened') || has('viewed')) && (
              <Tag color="blue" icon={<Eye size={8} />} className={statusTagCls}>
                {t('jobs.viewed')}
              </Tag>
            )}
            {has('applied') && (
              <Tag color="purple" icon={<CircleCheck size={8} />} className={statusTagCls}>
                {t('jobs.applied')}
              </Tag>
            )}
            {has('bookmarked') && (
              <Tag color="warning" icon={<Bookmark size={8} />} className={statusTagCls}>
                {t('jobs.saved')}
              </Tag>
            )}
            <TrustBadge trust={posting.trust} className={statusTagCls} />
          </div>
        </div>

        {/* RIGHT: action cluster — Save/View, Tailor, ActionMenu.
            `shrink-0` used to pin this at max-content, so `flex-wrap` never
            fired and the trailing actions were clipped by the pane's
            overflow-hidden on a narrow window. `min-w-0` lets it shrink below
            max-content so the wrap actually happens; under a ~757px pane (see
            the header's container note) it takes its own full-width row
            beneath the title instead of fighting it for a single line. */}
        <div className="@2xl:w-auto @2xl:justify-end flex w-full min-w-0 flex-wrap items-center gap-2">
          <motion.div layout transition={transition.fast} className="shrink-0">
            <Button
              variant="primary"
              onClick={saved ? handleView : handleSave}
              disabled={pending}
              loading={pending}
              title={saved ? t('jobs.view') : t('applications.saveToTracking')}
            >
              {saved ? <Eye size={11} /> : <Save size={11} />}{' '}
              {saved ? t('jobs.view') : t('applications.save')}
            </Button>
          </motion.div>
          <Button variant="glass" onClick={() => void handleTailor()} title={t('jobs.tailorHint')}>
            <Wand2 size={11} /> {t('jobs.tailor')}
          </Button>
          <ActionMenu
            label={t('jobs.actions')}
            items={[
              { label: t('jobs.open'), icon: <ExternalLink size={14} />, onSelect: handleOpen },
              {
                label: t('jobs.copyLink'),
                icon: <Copy size={14} />,
                onSelect: () => void handleCopyLink(),
              },
            ]}
          />
        </div>
      </div>
    </div>
  );
}
