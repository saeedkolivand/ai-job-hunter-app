import { Check, Copy, Send, UserPlus } from 'lucide-react';
import { useState } from 'react';

import type { ReferralContact } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { Button, useNotification } from '@ajh/ui';

import { COPY_FEEDBACK_LONG_MS } from '@/lib/timings';
import { useUpsertReferral } from '@/services/use-referrals/use-referrals';

import { Section } from './Section';

/** The persisted draft text for a contact depends on the chosen channel. */
function referralDraft(contact: ReferralContact): string {
  if (contact.channel === 'email') return contact.emailDraft ?? '';
  if (contact.channel === 'linkedin_message') return contact.messageDraft ?? '';
  return contact.inviteNoteDraft ?? '';
}

/** Copy / mark-as-sent actions for referral contacts; state lives with the card, not the section. */
export function useReferralActions() {
  const { t } = useTranslation();
  const notify = useNotification();
  const upsertReferral = useUpsertReferral();
  const [copiedReferral, setCopiedReferral] = useState<string | null>(null);

  const copyReferralDraft = async (contact: ReferralContact) => {
    const draft = referralDraft(contact);
    if (!draft) return;
    await navigator.clipboard.writeText(draft);
    setCopiedReferral(contact.id);
    notify.success({ message: t('resumes.generated.referralCopied') });
    setTimeout(
      () => setCopiedReferral((id) => (id === contact.id ? null : id)),
      COPY_FEEDBACK_LONG_MS
    );
  };

  // Mark a referral as sent. The backend upsert overwrites the whole row by id
  // (only `created_at` is preserved), so we re-send every field with the status
  // flipped — passing a partial payload would blank the other columns.
  const markReferralSent = (contact: ReferralContact) => {
    upsertReferral.mutate(
      {
        id: contact.id,
        jobUrl: contact.jobUrl,
        companyName: contact.companyName,
        personName: contact.personName,
        personRole: contact.personRole,
        linkedinUrl: contact.linkedinUrl,
        emailDraft: contact.emailDraft,
        messageDraft: contact.messageDraft,
        inviteNoteDraft: contact.inviteNoteDraft,
        channel: contact.channel,
        status: 'sent',
        notes: contact.notes,
      },
      {
        onSuccess: () => notify.success({ message: t('resumes.generated.referralMarkedSent') }),
      }
    );
  };

  return {
    copiedReferral,
    copyReferralDraft,
    markReferralSent,
    markingSent: upsertReferral.isPending,
  };
}

interface ReferralSectionProps {
  contacts: ReferralContact[];
  open: boolean;
  onToggle: () => void;
  actions: ReturnType<typeof useReferralActions>;
}

/** Referral requests — display-joined to the generation by job URL. */
export function ReferralSection({ contacts, open, onToggle, actions }: ReferralSectionProps) {
  const { t } = useTranslation();
  const { copiedReferral, copyReferralDraft, markReferralSent, markingSent } = actions;
  return (
    <Section
      label={t('resumes.generated.referralTitle')}
      icon={UserPlus}
      badge={contacts.length}
      open={open}
      onToggle={onToggle}
    >
      <div className="max-h-80 select-text space-y-2.5 overflow-y-auto px-5 pb-5">
        {contacts.map((contact) => {
          const draft = referralDraft(contact);
          return (
            <div key={contact.id} className="surface-card space-y-2.5 rounded-lg px-3.5 py-3">
              <div className="flex flex-wrap items-start justify-between gap-2">
                <div className="min-w-0">
                  <p className="truncate text-[12px] font-medium text-foreground/85">
                    {contact.personName}
                    {contact.personRole ? (
                      <span className="font-normal text-foreground/45">
                        {' '}
                        · {contact.personRole}
                      </span>
                    ) : null}
                  </p>
                  <p className="mt-0.5 flex flex-wrap items-center gap-x-1.5 text-[10px] text-foreground/45">
                    <span>{t(`resumes.generated.referralChannel.${contact.channel}`)}</span>
                    <span className="text-foreground/25">·</span>
                    <span>{t(`resumes.generated.referralStatus.${contact.status}`)}</span>
                  </p>
                </div>
                <div className="flex shrink-0 items-center gap-1.5">
                  <Button
                    disabled={!draft}
                    onClick={() => void copyReferralDraft(contact)}
                    title={t('resumes.generated.referralCopyDraft')}
                    className="flex h-auto items-center gap-1.5 rounded-lg border-transparent bg-white/5 px-2.5 py-1.5 text-[10px] text-foreground/60 transition-colors hover:text-foreground"
                  >
                    {copiedReferral === contact.id ? <Check size={11} /> : <Copy size={11} />}
                    {t('resumes.generated.referralCopyDraft')}
                  </Button>
                  {contact.status !== 'sent' && (
                    <Button
                      disabled={markingSent}
                      onClick={() => markReferralSent(contact)}
                      title={t('resumes.generated.referralMarkSent')}
                      className="flex h-auto items-center gap-1.5 rounded-lg border-brand/20 bg-brand/10 px-2.5 py-1.5 text-[10px] text-brand-soft transition-colors hover:bg-brand/20"
                    >
                      <Send size={11} />
                      {t('resumes.generated.referralMarkSent')}
                    </Button>
                  )}
                </div>
              </div>

              {draft ? (
                <pre className="max-h-40 select-text overflow-y-auto whitespace-pre-wrap font-mono text-[10px] leading-relaxed text-foreground/55">
                  {draft}
                </pre>
              ) : (
                <p className="text-[10px] italic text-foreground/35">
                  {t('resumes.generated.referralNoDraft')}
                </p>
              )}
            </div>
          );
        })}
      </div>
    </Section>
  );
}
