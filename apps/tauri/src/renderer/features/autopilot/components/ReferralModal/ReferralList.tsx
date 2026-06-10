import { Trash2 } from 'lucide-react';

import type { ReferralContact, ReferralStatus } from '@ajh/shared/ipc';
import { Button, SegmentedControl } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useRemoveReferral, useUpsertReferral } from '@/services';

interface Props {
  contacts: ReferralContact[];
}

const STATUSES: ReferralStatus[] = ['draft', 'sent', 'replied'];

/**
 * The existing referral contacts saved for this job: person/role/company, the
 * channel, an inline status control (draft → sent → replied), notes, and delete.
 * Edits persist through {@link useUpsertReferral}; delete is optimistic via
 * {@link useRemoveReferral}.
 */
export function ReferralList({ contacts }: Props) {
  const { t } = useTranslation();
  const upsert = useUpsertReferral();
  const remove = useRemoveReferral();

  if (contacts.length === 0) return null;

  const channelLabel = (channel: ReferralContact['channel']) =>
    t(`autopilot.referral.channel.${channel}`);

  return (
    <div className="space-y-2">
      <p className="text-[11px] font-medium text-foreground/70">
        {t('autopilot.referral.savedTitle')} ({contacts.length})
      </p>
      {contacts.map((c) => (
        <div
          key={c.id}
          className="space-y-2 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2.5"
        >
          <div className="flex items-start justify-between gap-2">
            <div className="min-w-0">
              <p className="truncate text-[12px] font-medium text-foreground/85">
                {c.personName}
                {c.personRole ? (
                  <span className="font-normal text-foreground/45"> · {c.personRole}</span>
                ) : null}
              </p>
              <p className="truncate text-[10px] text-foreground/40">
                {channelLabel(c.channel)} · {c.companyName}
              </p>
            </div>
            <Button
              variant="unstyled"
              type="button"
              onClick={() => remove.mutate(c.id)}
              title={t('autopilot.referral.delete')}
              aria-label={t('autopilot.referral.delete')}
              className="shrink-0 rounded p-1 text-foreground/30 transition-colors hover:text-red-300/80"
            >
              <Trash2 size={13} />
            </Button>
          </div>

          <SegmentedControl<ReferralStatus>
            size="sm"
            tone="brand"
            ariaLabel={t('autopilot.referral.status.label')}
            value={c.status}
            onChange={(status) =>
              // Full-row upsert: the backend overwrites every column by id (only
              // createdAt is preserved), so a partial { id, status } would blank
              // personName/company/drafts/notes. Re-send the whole contact.
              upsert.mutate({
                id: c.id,
                jobUrl: c.jobUrl,
                companyName: c.companyName,
                personName: c.personName,
                personRole: c.personRole,
                linkedinUrl: c.linkedinUrl,
                emailDraft: c.emailDraft,
                messageDraft: c.messageDraft,
                inviteNoteDraft: c.inviteNoteDraft,
                channel: c.channel,
                status,
                notes: c.notes,
              })
            }
            options={STATUSES.map((s) => ({
              value: s,
              label: t(`autopilot.referral.status.${s}`),
            }))}
          />

          {c.notes ? <p className="text-[10px] text-foreground/50">{c.notes}</p> : null}
        </div>
      ))}
    </div>
  );
}
