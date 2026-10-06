import { MessageSquarePlus } from 'lucide-react';
import { useRef, useState } from 'react';

import type { Application, StatusEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Timeline, useNotification } from '@ajh/ui';

import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { useAcceptStatusEvent, useRejectStatusEvent } from '@/services';

import { formatEventDate, timelineEventColor } from './detail-format';
import { TabScroll } from './DetailChrome';
import { TimelineEventBody } from './TimelineEventBody';

interface TimelineTabProps {
  application: Application;
  events: StatusEvent[];
  /** Ask the page (which outlives a refetch) to open the optional-note prompt. */
  onNotePrompt: (status: string, changed: boolean) => void;
}

/**
 * In-flight eventIds for one mutation kind. `begin(id)` adds the id (before the
 * mutate call, so the pending row never has a gap) and returns the settle
 * callback that clears it.
 */
function useInFlightIds() {
  const [ids, setIds] = useState<Set<number>>(() => new Set());
  const begin = (eventId: number) => {
    setIds((prev) => new Set(prev).add(eventId));
    return () =>
      setIds((prev) => {
        const next = new Set(prev);
        next.delete(eventId);
        return next;
      });
  };
  return [ids, begin] as const;
}

/**
 * Timeline tab — its own component (like {@link BriefTab}/{@link DocumentsTab},
 * not inlined in `ApplicationDetailLoaded`) so `useNotification()` and the
 * accept/reject mutations are only reached once this tab actually mounts.
 * Every other tab — and every test that never visits Timeline — stays clear
 * of the "must be used within NotificationProvider" requirement those calls
 * carry.
 */
export function TimelineTab({ application, events, onNotePrompt }: TimelineTabProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const formatRelative = useFormatRelativeTime(t, 'resumes.relativeTime');
  const acceptStatusEvent = useAcceptStatusEvent();
  const rejectStatusEvent = useRejectStatusEvent();

  // A provisional row's Accept/Reject buttons vanish once the mutation
  // resolves (the row re-renders as settled). Move focus to the stable
  // Timeline heading beforehand so it never falls back to `document.body`.
  const timelineHeadingRef = useRef<HTMLSpanElement>(null);

  // `acceptStatusEvent`/`rejectStatusEvent` are each ONE `useMutation()`
  // instance shared by every row, so `.variables`/`.isPending` reflect only
  // the MOST RECENT `mutate()` call — they cannot represent two concurrent
  // in-flight rows. Track in-flight eventIds ourselves instead: cleared in
  // `onSettled` (fires on success OR error, unlike `onSuccess`/`onError`
  // alone). Accept/reject get separate sets so the correct button shows its
  // own spinner even if a row somehow has both in flight.
  const [acceptingEventIds, beginAccepting] = useInFlightIds();
  const [rejectingEventIds, beginRejecting] = useInFlightIds();

  // Both take the SPECIFIC row's `eventId` as a param — never a shared,
  // zero-arg closure. Two provisional rows can coexist (a confirmation email,
  // then a later rejection email, both still unreviewed); resolving "the
  // pending row" any other way let a click on one row's button act on a
  // DIFFERENT row entirely. See `StatusEvent.eventId`'s doc.
  const handleAcceptEvent = (eventId: number) => {
    const settle = beginAccepting(eventId);
    acceptStatusEvent.mutate(
      { id: application.id, eventId },
      {
        // `applications_accept_status_event` returns `Value`, not `Result` —
        // a backend failure resolves as `{ error }` and `invoke` FULFILS, so
        // `onError` never fires for it. Check `data.error` here, same as the
        // contact-write handlers, or a transient DB failure would
        // still show the success toast while the row stays provisional.
        onSuccess: (data) => {
          if (data.error) {
            notify.error({ message: t('applications.detail.timeline.acceptError') });
            return;
          }
          timelineHeadingRef.current?.focus();
          notify.success({ message: t('applications.detail.timeline.acceptSuccess') });
        },
        onError: () => notify.error({ message: t('applications.detail.timeline.acceptError') }),
        onSettled: settle,
      }
    );
  };
  const handleRejectEvent = (eventId: number) => {
    const settle = beginRejecting(eventId);
    rejectStatusEvent.mutate(
      { id: application.id, eventId },
      {
        // Same `{ error }`-on-resolve shape as accept above — check it before
        // ever showing the success toast.
        onSuccess: (data) => {
          if (data.error) {
            notify.error({ message: t('applications.detail.timeline.rejectError') });
            return;
          }
          timelineHeadingRef.current?.focus();
          // Deliberately NOT "reverted" — the compare-and-set may have lost
          // (the user changed the status by hand meanwhile), in which case
          // this only dismissed the provisional row. The rendered timeline
          // (a correction row iff the CAS won) is the source of truth.
          notify.success({ message: t('applications.detail.timeline.rejectSuccess') });
        },
        onError: () => notify.error({ message: t('applications.detail.timeline.rejectError') }),
        onSettled: settle,
      }
    );
  };

  // `events()` orders by `at ASC, rowid ASC`; `Array#sort` is stable, so a
  // bare `b.at - a.at` keeps ASCENDING insertion order for any pair sharing
  // one `at` while every surrounding pair is descending. That's reachable
  // here: a reject appends its reversal row immediately after its
  // compare-and-set wins, so a correction and the provisional row it
  // resolves can share a millisecond. `eventId` IS the rowid — the same
  // descending direction as `at` keeps the backend's tie order intact.
  const orderedEvents = [...events].sort((a, b) => b.at - a.at || b.eventId - a.eventId);
  const statusLabel = (status: string) =>
    status ? t(`applications.status.${status}` as const) : t('applications.detail.created');

  return (
    <TabScroll>
      <div className="flex items-center justify-between gap-2">
        <span
          ref={timelineHeadingRef}
          tabIndex={-1}
          className="block rounded text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45 focus-visible:ring-2 focus-visible:ring-brand/50"
        >
          {t('applications.detail.timelineTitle')}
        </span>
        <Button
          variant="glass"
          size="sm"
          className="gap-1.5"
          onClick={() => onNotePrompt(application.status, false)}
        >
          <MessageSquarePlus size={12} />
          {t('applications.note.add')}
        </Button>
      </div>
      {orderedEvents.length === 0 ? (
        <p className="text-xs text-foreground/45">{t('applications.detail.timelineEmpty')}</p>
      ) : (
        <Timeline
          items={orderedEvents.map((e) => ({
            color: timelineEventColor(e),
            label: <span title={formatRelative(e.at)}>{formatEventDate(e.at)}</span>,
            children: (
              <TimelineEventBody
                event={e}
                t={t}
                statusLabel={statusLabel}
                // Per-row closures — each captures THIS row's `eventId`,
                // never a shared handler (see `handleAcceptEvent`).
                onAccept={() => handleAcceptEvent(e.eventId)}
                onReject={() => handleRejectEvent(e.eventId)}
                // Own in-flight tracking, not `.isPending`/`.variables` on
                // the shared mutation hook (see `useInFlightIds`).
                acceptPending={acceptingEventIds.has(e.eventId)}
                rejectPending={rejectingEventIds.has(e.eventId)}
              />
            ),
          }))}
        />
      )}
    </TabScroll>
  );
}
