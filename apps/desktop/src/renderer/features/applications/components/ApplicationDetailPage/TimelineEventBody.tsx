import { Check, Mail, Undo2, X } from 'lucide-react';

import type { StatusEvent } from '@ajh/shared';
import type { TFunction } from '@ajh/translations';
import { Button, Tag } from '@ajh/ui';

import { isCorrectionEvent, isProvisionalEvent } from './detail-format';

interface TimelineEventBodyProps {
  event: StatusEvent;
  t: TFunction;
  statusLabel: (status: string) => string;
  onAccept: () => void;
  onReject: () => void;
  acceptPending: boolean;
  rejectPending: boolean;
}

/**
 * One Timeline row's content. Three renderings, driven entirely by
 * `event.source`/`event.confirmed` (never a fabricated confidence number —
 * nothing in the payload carries one):
 *  - provisional (unconfirmed email write): reads as a guess awaiting review
 *    ("we think this happened — is that right?"), with Accept/Reject.
 *  - correction (`email_reject`'s reversal row): reads as a correction in the
 *    trail, not a normal user transition.
 *  - everything else (user-sourced, or an accepted email write): today's
 *    plain transition row.
 */
export function TimelineEventBody({
  event,
  t,
  statusLabel,
  onAccept,
  onReject,
  acceptPending,
  rejectPending,
}: TimelineEventBodyProps) {
  const provisional = isProvisionalEvent(event);
  const correction = isCorrectionEvent(event);

  const transitionText =
    event.fromStatus && event.fromStatus !== event.toStatus ? (
      <>
        <span className="text-foreground/55">{statusLabel(event.fromStatus)}</span>
        <span className="text-foreground/30">→</span>
        <span className="font-medium text-foreground/85">{statusLabel(event.toStatus)}</span>
      </>
    ) : (
      <span className="font-medium text-foreground/85">{statusLabel(event.toStatus)}</span>
    );

  // Descriptive, not a bare "Accept"/"Reject" repeated down the list — names
  // WHICH transition the action resolves, for the accessible name below.
  const transitionDesc =
    event.fromStatus && event.fromStatus !== event.toStatus
      ? t('applications.detail.timeline.transitionDesc', {
          from: statusLabel(event.fromStatus),
          to: statusLabel(event.toStatus),
        })
      : statusLabel(event.toStatus);

  return (
    <>
      {provisional && (
        <Tag color="warning" icon={<Mail size={9} />} className="mb-1 text-[9px]">
          {t('applications.detail.timeline.provisionalBadge')}
        </Tag>
      )}
      {correction && (
        <Tag color="default" icon={<Undo2 size={9} />} className="mb-1 text-[9px]">
          {t('applications.detail.timeline.correctionBadge')}
        </Tag>
      )}
      <span className="flex items-center gap-1.5">{transitionText}</span>
      {/* The backend writes a fixed, non-localized English literal into
          `note` for BOTH the auto-write itself ("email-derived
          (unconfirmed)") and its reversal ("reverted: email-derived status
          change rejected by the user") — never render either verbatim; the
          badge + localized hint below say the same thing translated. */}
      {event.note && !provisional && !correction && (
        <span className="mt-0.5 block text-[11px] text-foreground/55">{event.note}</span>
      )}
      {provisional && (
        <p className="mt-0.5 text-[11px] text-foreground/50">
          {t('applications.detail.timeline.provisionalHint')}
        </p>
      )}
      {correction && (
        <p className="mt-0.5 text-[11px] text-foreground/50">
          {t('applications.detail.timeline.correctionHint')}
        </p>
      )}
      {provisional && (
        <div className="mt-1.5 flex items-center gap-1.5">
          <Button
            variant="success"
            size="sm"
            loading={acceptPending}
            disabled={acceptPending || rejectPending}
            onClick={onAccept}
            aria-label={t('applications.detail.timeline.acceptAria', {
              transition: transitionDesc,
            })}
          >
            <Check size={11} />
            {t('applications.detail.timeline.accept')}
          </Button>
          <Button
            variant="danger"
            size="sm"
            loading={rejectPending}
            disabled={acceptPending || rejectPending}
            onClick={onReject}
            aria-label={t('applications.detail.timeline.rejectAria', {
              transition: transitionDesc,
            })}
          >
            <X size={11} />
            {t('applications.detail.timeline.reject')}
          </Button>
        </div>
      )}
    </>
  );
}
