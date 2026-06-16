import {
  Building2,
  Calendar,
  Check,
  ChevronDown,
  Copy,
  Download,
  ExternalLink as ExternalLinkIcon,
  FileText,
  HelpCircle,
  Search,
  Send,
  Trash2,
  UserPlus,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import type { AiGenerationRecord, ReferralChannel, ReferralContact } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import {
  ActionMenu,
  Button,
  cn,
  ConfirmModal,
  GlassCard,
  transition,
  useNotification,
} from '@ajh/ui';

import { EditableOutput } from '@/components/generation/EditableOutput';
import {
  ExportActionIcon,
  type ExportFormat,
  ExportPicker,
} from '@/components/generation/ExportPicker';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  PERSIST_DEBOUNCE_MS,
  type TemplateId,
  TEMPLATES,
} from '@/lib/generate';
import { useOpenExternal } from '@/services';
import { useRemoveAiGeneration, useUpdateAiGeneration } from '@/services/use-ai-generations';
import { useReferrals, useUpsertReferral } from '@/services/use-referrals/use-referrals';

import { Section } from './Section';

const TEMPLATE_OPTIONS: { id: TemplateId; label: string }[] = Object.values(TEMPLATES).map((t) => ({
  id: t.id,
  label: t.name,
}));

type SectionKey = 'resume' | 'cover' | 'jobAd' | 'brief' | 'answers' | 'referral';

/** The persisted draft text for a contact depends on the chosen channel. */
function referralDraft(contact: ReferralContact): string {
  if (contact.channel === 'email') return contact.emailDraft ?? '';
  if (contact.channel === 'linkedin_message') return contact.messageDraft ?? '';
  return contact.inviteNoteDraft ?? '';
}

interface GenerationCardProps {
  gen: AiGenerationRecord;
  selected?: boolean;
  onToggleSelect?: (id: string) => void;
}

export function GenerationCard({ gen, selected = false, onToggleSelect }: GenerationCardProps) {
  const { t } = useTranslation();
  const formatRelative = useFormatRelativeTime(t, 'resumes.relativeTime');
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const removeAiGeneration = useRemoveAiGeneration();
  const updateAiGeneration = useUpdateAiGeneration();
  const referrals = useReferrals(gen.jobUrl);
  const upsertReferral = useUpsertReferral();
  // Card collapses to its header row by default; click to reveal the body (#27).
  const [cardExpanded, setCardExpanded] = useState(false);
  const [expanded, setExpanded] = useState<SectionKey | null>(null);
  const [showExportModal, setShowExportModal] = useState(false);
  const [copiedReferral, setCopiedReferral] = useState<string | null>(null);
  const [exportFormat, setExportFormat] = useState<ExportFormat>('pdf');
  const [exportTemplate, setExportTemplate] = useState<TemplateId>('modern');
  const [exporting, setExporting] = useState<'resume' | 'cover' | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  // Local editing buffers keep typing smooth and own the edit truth for the card's
  // lifetime. The card is keyed by `gen.id` at the list (it remounts per record),
  // so drafts are seeded once from the record on mount; thereafter the optimistic
  // update hook patches the list cache (with rollback on failure) and we do NOT
  // re-sync the drafts from `gen`. A re-sync here would let the post-`onSettled`
  // refetch overwrite the buffer with debounce-stale text, clobbering keystrokes
  // typed during the 800ms debounce window.
  const [resumeDraft, setResumeDraft] = useState(gen.resumeText);
  const [coverDraft, setCoverDraft] = useState(gen.coverLetterText);

  // Debounced persistence — one timer per field; flushed on unmount.
  const persistTimers = useRef<{
    resume?: ReturnType<typeof setTimeout>;
    cover?: ReturnType<typeof setTimeout>;
  }>({});
  useEffect(() => {
    const timers = persistTimers.current;
    return () => {
      if (timers.resume) clearTimeout(timers.resume);
      if (timers.cover) clearTimeout(timers.cover);
    };
  }, []);

  const persistEdit = (type: 'resume' | 'cover', text: string) => {
    const existing = persistTimers.current[type];
    if (existing) clearTimeout(existing);
    persistTimers.current[type] = setTimeout(() => {
      updateAiGeneration.mutate(
        type === 'resume' ? { id: gen.id, resumeText: text } : { id: gen.id, coverLetterText: text }
      );
    }, PERSIST_DEBOUNCE_MS);
  };

  const onEdit = (type: 'resume' | 'cover', text: string) => {
    if (type === 'resume') setResumeDraft(text);
    else setCoverDraft(text);
    persistEdit(type, text);
  };

  const handleDelete = () => {
    setConfirmDelete(false);
    removeAiGeneration.mutate(gen.id);
  };

  const copyReferralDraft = async (contact: ReferralContact) => {
    const draft = referralDraft(contact);
    if (!draft) return;
    await navigator.clipboard.writeText(draft);
    setCopiedReferral(contact.id);
    notify.success({ message: t('resumes.generated.referralCopied') });
    setTimeout(() => setCopiedReferral((id) => (id === contact.id ? null : id)), 1800);
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

  const meta = {
    candidateName: gen.candidateName,
    jobTitle: gen.jobTitle,
    companyName: gen.companyName,
    resumeLanguage: gen.resumeLanguage,
    jobAdLanguage: gen.jobAdLanguage,
    targetLanguage: gen.targetLanguage,
    topRequirements: gen.topRequirements,
    mismatch: gen.mismatch,
  };

  const doExport = async (type: 'resume' | 'cover') => {
    const text = type === 'resume' ? resumeDraft : coverDraft;
    if (!text) return;
    const docType = type === 'resume' ? 'resume' : 'cover-letter';
    const filename = buildFilename(meta, docType, exportFormat);
    setExporting(type);
    try {
      if (exportFormat === 'pdf') {
        await exportPDF(text, filename.replace('.pdf', ''), docType, meta, exportTemplate);
      } else if (exportFormat === 'docx') {
        await exportDOCX(text, filename.replace('.docx', ''), docType, meta, exportTemplate);
      } else {
        exportTXT(text, filename.replace('.txt', ''));
      }
    } finally {
      setExporting(null);
    }
  };

  const generatedDate = new Date(gen.createdAt).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });

  const channelLabel = (channel: ReferralChannel) =>
    t(`resumes.generated.referralChannel.${channel}`);
  const contacts = referrals.data ?? [];
  const hasOutput = Boolean(resumeDraft || coverDraft);

  return (
    <>
      <GlassCard className="rounded-xl overflow-hidden p-0">
        {/* Header row — collapsed by default; click the title area to expand (#27).
            Low-value actions (open posting / export / delete) live in the 3-dots
            overflow menu (#28/#32/#33). */}
        <div className="flex items-start gap-4 p-5">
          {onToggleSelect && (
            <div className="flex shrink-0 items-center self-center">
              <input
                type="checkbox"
                checked={selected}
                onChange={() => onToggleSelect(gen.id)}
                aria-label={t('resumes.select.selectItem')}
                className="h-4 w-4 cursor-pointer accent-[color:var(--color-brand)] rounded border border-white/20"
              />
            </div>
          )}

          <Button
            variant="unstyled"
            onClick={() => setCardExpanded((v) => !v)}
            aria-expanded={cardExpanded}
            className="flex min-w-0 flex-1 items-start gap-4 text-left"
          >
            <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-brand/10">
              <Wand2 size={16} className="text-brand-soft" />
            </span>

            <span className="min-w-0 flex-1 space-y-2">
              <span className="block min-w-0">
                <span className="block truncate text-[15px] font-semibold leading-tight text-foreground/90">
                  {gen.jobTitle || t('resumes.unknownPosition')}
                </span>
                {gen.companyName && (
                  <span className="mt-1 flex items-center gap-1.5 truncate text-xs text-foreground/55">
                    <Building2 size={11} className="shrink-0 text-foreground/35" />
                    {gen.companyName}
                  </span>
                )}
              </span>

              <span className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[11px] text-foreground/50">
                {gen.candidateName && <span>{gen.candidateName}</span>}
                <span className="flex items-center gap-1 text-foreground/40">
                  <Calendar size={11} />
                  <span title={formatRelative(gen.createdAt)}>{generatedDate}</span>
                </span>
                <span className="rounded-full border border-brand/20 bg-brand/8 px-2 py-0.5 text-[9px] uppercase tracking-wider text-brand-soft">
                  {gen.mode}
                </span>
                {gen.board && (
                  <span className="rounded-full border border-white/[0.06] bg-white/[0.03] px-2 py-0.5 text-[9px] uppercase tracking-wider text-foreground/55">
                    {gen.board}
                  </span>
                )}
                {/* A linked job means this generation was an application. */}
                {gen.jobUrl && (
                  <span className="flex items-center gap-1 rounded-full border border-emerald-400/20 bg-emerald-400/10 px-2 py-0.5 text-[9px] uppercase tracking-wider text-emerald-300">
                    <Check size={9} /> {t('resumes.generated.applied')}
                  </span>
                )}
              </span>
            </span>

            <ChevronDown
              size={16}
              className={cn(
                'mt-1 shrink-0 text-foreground/30 transition-transform',
                cardExpanded && 'rotate-180'
              )}
            />
          </Button>

          <div className="flex shrink-0 items-center self-center">
            <ActionMenu
              label={t('resumes.generated.actions')}
              items={[
                ...(gen.jobUrl
                  ? [
                      {
                        label: t('resumes.generated.openPosting'),
                        icon: <ExternalLinkIcon size={14} />,
                        onSelect: () => void openExternal.mutate(gen.jobUrl),
                      },
                    ]
                  : []),
                ...(hasOutput
                  ? [
                      {
                        label: t('resumes.generated.export'),
                        icon: <Download size={14} />,
                        onSelect: () => setShowExportModal(true),
                      },
                    ]
                  : []),
                {
                  label: t('resumes.generated.delete'),
                  icon: <Trash2 size={14} />,
                  destructive: true,
                  onSelect: () => setConfirmDelete(true),
                },
              ]}
            />
          </div>
        </div>

        {/* Body — only when the card is expanded (#27). */}
        <AnimatePresence initial={false}>
          {cardExpanded && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={transition.normal}
              className="overflow-hidden"
            >
              {/* Extracted keywords — on top, labelled (#29). */}
              {gen.topRequirements.length > 0 && (
                <div className="border-t border-white/[0.04] px-5 py-4">
                  <span className="mb-2 block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
                    {t('resumes.generated.keywords')}
                  </span>
                  <div className="flex flex-wrap gap-1.5">
                    {gen.topRequirements.map((req) => (
                      <span
                        key={req}
                        className="rounded-full border border-white/[0.06] bg-white/[0.03] px-2.5 py-1 text-[10px] text-foreground/55"
                      >
                        {req}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {/* Expandable sections. Resume + cover letter are editable (F1 + inline
                  rewrite); the job ad and company brief stay read-only references. */}
              {(
                [
                  {
                    key: 'resume' as const,
                    label: t('resumes.generated.resume'),
                    text: resumeDraft,
                    icon: FileText,
                    editType: 'resume' as const,
                    docType: 'resume' as const,
                  },
                  {
                    key: 'cover' as const,
                    label: t('resumes.generated.coverLetter'),
                    text: coverDraft,
                    icon: FileText,
                    editType: 'cover' as const,
                    docType: 'cover-letter' as const,
                  },
                  {
                    key: 'jobAd' as const,
                    label: t('resumes.generated.jobAd'),
                    text: gen.jobAd,
                    icon: Building2,
                    editType: null,
                    docType: null,
                  },
                  {
                    key: 'brief' as const,
                    label: t('resumes.generated.companyResearch'),
                    text: gen.companyBrief,
                    icon: Search,
                    editType: null,
                    docType: null,
                  },
                ] as const
              )
                .filter((s) => s.text)
                .map(({ key, label, text, icon: SectionIcon, editType, docType }) => (
                  <Section
                    key={key}
                    label={label}
                    icon={SectionIcon}
                    open={expanded === key}
                    onToggle={() => setExpanded(expanded === key ? null : key)}
                  >
                    {editType && docType ? (
                      <div className="flex h-72 flex-col px-5 pb-5">
                        <EditableOutput
                          value={text}
                          onChange={(v) => onEdit(editType, v)}
                          docType={docType}
                          meta={meta}
                          className="flex h-full flex-col overflow-hidden"
                          textAreaClassName="h-full w-full bg-transparent font-mono text-[11px] leading-relaxed text-foreground/65 placeholder:text-foreground/20"
                        />
                      </div>
                    ) : (
                      <pre className="max-h-64 select-text overflow-y-auto whitespace-pre-wrap px-5 pb-5 font-mono text-[11px] leading-relaxed text-foreground/55">
                        {text}
                      </pre>
                    )}
                  </Section>
                ))}

              {/* Application answers — structured Q/A from the questions assistant. */}
              {gen.applicationAnswers.length > 0 && (
                <Section
                  label={t('resumes.generated.applicationAnswers')}
                  icon={HelpCircle}
                  badge={gen.applicationAnswers.length}
                  open={expanded === 'answers'}
                  onToggle={() => setExpanded(expanded === 'answers' ? null : 'answers')}
                >
                  <div className="max-h-72 select-text space-y-3 overflow-y-auto px-5 pb-5">
                    {gen.applicationAnswers.map((qa) => (
                      <div key={qa.id}>
                        <p className="text-[11px] font-medium text-foreground/70">{qa.question}</p>
                        <p className="mt-0.5 whitespace-pre-wrap text-[11px] leading-relaxed text-foreground/55">
                          {qa.answer}
                        </p>
                      </div>
                    ))}
                  </div>
                </Section>
              )}

              {/* Referral requests — these live in their own table keyed by job URL, so
                  we display-join them here by `gen.jobUrl`. Each contact exposes copy
                  and mark-as-sent quick actions. */}
              {contacts.length > 0 && (
                <Section
                  label={t('resumes.generated.referralTitle')}
                  icon={UserPlus}
                  badge={contacts.length}
                  open={expanded === 'referral'}
                  onToggle={() => setExpanded(expanded === 'referral' ? null : 'referral')}
                >
                  <div className="max-h-80 select-text space-y-2.5 overflow-y-auto px-5 pb-5">
                    {contacts.map((contact) => {
                      const draft = referralDraft(contact);
                      return (
                        <div
                          key={contact.id}
                          className="space-y-2.5 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3.5 py-3"
                        >
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
                                <span>{channelLabel(contact.channel)}</span>
                                <span className="text-foreground/25">·</span>
                                <span>
                                  {t(`resumes.generated.referralStatus.${contact.status}`)}
                                </span>
                              </p>
                            </div>
                            <div className="flex shrink-0 items-center gap-1.5">
                              <Button
                                disabled={!draft}
                                onClick={() => void copyReferralDraft(contact)}
                                title={t('resumes.generated.referralCopyDraft')}
                                className="flex h-auto items-center gap-1.5 rounded-lg border-transparent bg-white/5 px-2.5 py-1.5 text-[10px] text-foreground/60 transition-colors hover:text-foreground"
                              >
                                {copiedReferral === contact.id ? (
                                  <Check size={11} />
                                ) : (
                                  <Copy size={11} />
                                )}
                                {t('resumes.generated.referralCopyDraft')}
                              </Button>
                              {contact.status !== 'sent' && (
                                <Button
                                  disabled={upsertReferral.isPending}
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
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </GlassCard>

      {/* Export — moved off the row into a modal (#28). */}
      <ExportPicker
        open={showExportModal}
        onClose={() => setShowExportModal(false)}
        format={exportFormat}
        onFormatChange={setExportFormat}
        templateId={exportTemplate}
        onTemplateChange={setExportTemplate}
        templateOptions={TEMPLATE_OPTIONS}
      >
        {resumeDraft && (
          <Button
            variant="primary"
            disabled={exporting === 'resume'}
            onClick={() => void doExport('resume')}
            className="flex h-auto items-center gap-1.5 px-3 py-1.5 text-[11px]"
          >
            <ExportActionIcon loading={exporting === 'resume'} />
            {t('resumes.generated.exportResume')}
          </Button>
        )}
        {coverDraft && (
          <Button
            disabled={exporting === 'cover'}
            onClick={() => void doExport('cover')}
            className="flex h-auto items-center gap-1.5 rounded-lg border-white/[0.06] bg-white/5 px-3 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground"
          >
            <ExportActionIcon loading={exporting === 'cover'} />
            {t('resumes.generated.exportCoverLetter')}
          </Button>
        )}
      </ExportPicker>

      <ConfirmModal
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        onConfirm={handleDelete}
        title={t('resumes.generated.deleteTitle')}
        description={t('resumes.generated.deleteDescription')}
        confirmText={t('resumes.generated.delete')}
        variant="danger"
        isConfirming={removeAiGeneration.isPending}
      />
    </>
  );
}
