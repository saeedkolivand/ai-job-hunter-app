import { Check, Copy, Save, ShieldCheck, Sparkles, UserPlus, X } from 'lucide-react';
import { useState } from 'react';

import type { AutopilotFoundJob } from '@ajh/shared';
import type { ReferralChannel } from '@ajh/shared/ipc';
import { Button, Input, ModalShell, SegmentedControl, StreamingText, TextArea } from '@ajh/ui';

import { ModelSelector, useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { CONNECTION_NOTE_LIMIT } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useReferrals, useUpsertReferral } from '@/services';

import { ReferralList } from './ReferralList';
import { useReferralDraft } from './useReferralDraft';

interface Props {
  job: AutopilotFoundJob;
  /** The résumé in scope — the only source of factual claims for the draft. */
  resume: string;
  onClose: () => void;
}

const CHANNELS: ReferralChannel[] = ['email', 'linkedin_message', 'connection_note'];

/** Map the chosen channel to the matching persisted draft field. */
function draftField(channel: ReferralChannel): 'emailDraft' | 'messageDraft' | 'inviteNoteDraft' {
  if (channel === 'email') return 'emailDraft';
  if (channel === 'linkedin_message') return 'messageDraft';
  return 'inviteNoteDraft';
}

/**
 * "Ask for a referral" helper (F3a). The user types the person's details — there
 * is NO LinkedIn scraping or profile fetch — picks a channel, drafts a short,
 * honest, résumé-grounded referral message, and saves it locally. Connection
 * notes are hard-capped at {@link CONNECTION_NOTE_LIMIT} characters (LinkedIn's
 * invite-note limit), enforced both in the prompt and here.
 */
export function ReferralModal({ job, resume, onClose }: Props) {
  const { t } = useTranslation();
  const model = useSelectedModel();
  const { canUse, reason } = useCanUseAI();

  const [personName, setPersonName] = useState('');
  const [personRole, setPersonRole] = useState('');
  const [linkedinUrl, setLinkedinUrl] = useState('');
  const [notes, setNotes] = useState('');
  const [channel, setChannel] = useState<ReferralChannel>('linkedin_message');
  const [copied, setCopied] = useState(false);
  const [saved, setSaved] = useState(false);

  const contacts = useReferrals(job.url);
  const upsert = useUpsertReferral();
  const gen = useReferralDraft({
    personName,
    personRole,
    companyName: job.company,
    jobTitle: job.title,
    resume,
    channel,
    model,
    canUse,
  });

  const isNote = channel === 'connection_note';
  const overLimit = isNote && gen.draft.length > CONNECTION_NOTE_LIMIT;

  const copy = async () => {
    if (!gen.draft || overLimit) return;
    await navigator.clipboard.writeText(gen.draft);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const save = () => {
    const draft = gen.draft.trim();
    if (!personName.trim() || !draft || overLimit) return;
    upsert.mutate(
      {
        jobUrl: job.url,
        companyName: job.company,
        personName: personName.trim(),
        personRole: personRole.trim() || undefined,
        linkedinUrl: linkedinUrl.trim() || undefined,
        notes: notes.trim() || undefined,
        channel,
        status: 'draft',
        [draftField(channel)]: draft,
      },
      {
        onSuccess: () => {
          setSaved(true);
          setTimeout(() => setSaved(false), 1500);
        },
      }
    );
  };

  const channelLabel = (c: ReferralChannel) => t(`autopilot.referral.channel.${c}`);
  const canSave = personName.trim().length > 0 && gen.draft.trim().length > 0 && !overLimit;

  return (
    <ModalShell
      open
      onClose={onClose}
      maxWidth="max-w-lg"
      // Second-layer modal opened from ApplyJobModal — sit above the default
      // modal layer (600) so it never renders under its parent.
      zIndex={650}
      ariaLabelledby="referral-modal-title"
    >
      <div className="flex max-h-[85vh] flex-col">
        {/* Header */}
        <div className="flex items-start justify-between gap-3 border-b border-white/[0.08] px-5 py-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <UserPlus size={14} className="shrink-0 text-brand-soft" />
              <span
                id="referral-modal-title"
                className="truncate text-sm font-semibold text-foreground/85"
              >
                {t('autopilot.referral.title')}
              </span>
            </div>
            <div className="mt-0.5 truncate text-[11px] text-foreground/40">
              {job.title} · {job.company}
            </div>
          </div>
          <Button
            onClick={onClose}
            aria-label={t('autopilot.referral.close')}
            className="h-auto shrink-0 border-transparent bg-transparent p-0 text-foreground/30 hover:text-foreground/60"
          >
            <X size={16} />
          </Button>
        </div>

        {/* Body */}
        <div className="flex-1 space-y-3 overflow-y-auto px-5 py-4">
          {/* Privacy note — this stores another person's details. */}
          <div className="flex items-start gap-2 rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-2">
            <ShieldCheck size={13} className="mt-0.5 shrink-0 text-brand-soft" />
            <p className="text-[10px] leading-relaxed text-foreground/55">
              {t('autopilot.referral.privacy')}
            </p>
          </div>

          {/* Person details — all typed by the user; no LinkedIn fetch. */}
          <div className="space-y-2">
            <label className="block">
              <span className="mb-1 block text-[11px] font-medium text-foreground/70">
                {t('autopilot.referral.personName')}
              </span>
              <Input
                value={personName}
                onChange={(e) => setPersonName(e.target.value)}
                placeholder={t('autopilot.referral.personNamePlaceholder')}
              />
            </label>
            <label className="block">
              <span className="mb-1 block text-[11px] font-medium text-foreground/70">
                {t('autopilot.referral.personRole')}
              </span>
              <Input
                value={personRole}
                onChange={(e) => setPersonRole(e.target.value)}
                placeholder={t('autopilot.referral.personRolePlaceholder')}
              />
            </label>
            <label className="block">
              <span className="mb-1 block text-[11px] font-medium text-foreground/70">
                {t('autopilot.referral.linkedinUrl')}
              </span>
              <Input
                value={linkedinUrl}
                onChange={(e) => setLinkedinUrl(e.target.value)}
                placeholder={t('autopilot.referral.linkedinUrlPlaceholder')}
              />
            </label>
          </div>

          {/* Channel / format picker */}
          <div className="space-y-1.5">
            <span className="block text-[11px] font-medium text-foreground/70">
              {t('autopilot.referral.channel.label')}
            </span>
            <SegmentedControl<ReferralChannel>
              variant="grid"
              ariaLabel={t('autopilot.referral.channel.label')}
              value={channel}
              onChange={setChannel}
              options={CHANNELS.map((c) => ({ value: c, label: channelLabel(c) }))}
            />
          </div>

          {/* Model */}
          <ModelSelector />

          {/* Generate */}
          <div className="flex items-center justify-end gap-2">
            {gen.generating && (
              <Button
                variant="glass"
                size="sm"
                onClick={() => gen.abort()}
                className="border-red-400/20 text-red-300/80 hover:text-red-200"
              >
                {t('autopilot.referral.cancel')}
              </Button>
            )}
            <Button
              variant="glass"
              size="sm"
              loading={gen.generating}
              disabled={!gen.canGenerate}
              onClick={() => void gen.generate()}
            >
              {!gen.generating && <Sparkles size={13} />}
              {gen.generating
                ? t('autopilot.referral.generating')
                : t('autopilot.referral.generate')}
            </Button>
          </div>

          {!canUse && (
            <p className="text-[11px] text-amber-300/70">
              {reason === 'addApiKey'
                ? t('autopilot.apply.addApiKey')
                : reason === 'installCli'
                  ? t('autopilot.apply.installCli')
                  : t('autopilot.apply.selectModel')}
            </p>
          )}
          {gen.error && <p className="text-[11px] text-red-300/80">{gen.error}</p>}

          {/* Draft output */}
          {(gen.draft || gen.generating) && (
            <div className="space-y-1.5 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2.5">
              <StreamingText text={gen.draft} isStreaming={gen.generating} />
              <div className="flex items-center justify-between gap-2 pt-1">
                {isNote ? (
                  <span
                    className={
                      overLimit
                        ? 'text-[10px] font-medium text-red-300/90'
                        : 'text-[10px] text-foreground/40'
                    }
                  >
                    {gen.draft.length}/{CONNECTION_NOTE_LIMIT}
                    {overLimit ? ` · ${t('autopilot.referral.overLimit')}` : ''}
                  </span>
                ) : (
                  <span />
                )}
                <div className="flex items-center gap-2">
                  <Button
                    variant="glass"
                    size="sm"
                    disabled={!gen.draft || overLimit || gen.generating}
                    onClick={() => void copy()}
                  >
                    {copied ? <Check size={12} /> : <Copy size={12} />}
                    {copied ? t('autopilot.referral.copied') : t('autopilot.referral.copy')}
                  </Button>
                  <Button
                    variant="glass"
                    size="sm"
                    loading={upsert.isPending}
                    disabled={!canSave || upsert.isPending}
                    onClick={save}
                  >
                    {saved ? <Check size={12} /> : <Save size={12} />}
                    {saved ? t('autopilot.referral.saved') : t('autopilot.referral.save')}
                  </Button>
                </div>
              </div>
            </div>
          )}

          {/* Optional notes saved with the contact */}
          <label className="block">
            <span className="mb-1 block text-[11px] font-medium text-foreground/70">
              {t('autopilot.referral.notes')}
            </span>
            <TextArea
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={2}
              variant="glass"
              placeholder={t('autopilot.referral.notesPlaceholder')}
            />
          </label>

          {/* Existing contacts for this job */}
          <ReferralList contacts={contacts.data ?? []} />
        </div>
      </div>
    </ModalShell>
  );
}
