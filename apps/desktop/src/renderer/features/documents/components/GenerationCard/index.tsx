import {
  Building2,
  Download,
  ExternalLink as ExternalLinkIcon,
  FileText,
  HelpCircle,
  Search,
  Trash2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import type { AiGenerationRecord } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { ActionMenu, Button, ConfirmModal, transition } from '@ajh/ui';

import { EditableOutput } from '@/components/generation/EditableOutput';
import { ExportActionIcon, ExportPicker } from '@/components/generation/ExportPicker';
import { type TemplateId, TEMPLATES } from '@/lib/generate';
import { useOpenExternal } from '@/services';
import { useRemoveAiGeneration } from '@/services/use-ai-generations';
import { useReferrals } from '@/services/use-referrals/use-referrals';

import { GenerationCardTitle } from './GenerationCardTitle';
import { ReferralSection, useReferralActions } from './ReferralSection';
import { Section } from './Section';
import { useGenerationDrafts } from './useGenerationDrafts';
import { useGenerationExport } from './useGenerationExport';

const TEMPLATE_OPTIONS: { id: TemplateId; label: string }[] = Object.values(TEMPLATES).map((t) => ({
  id: t.id,
  label: t.name,
}));

type SectionKey = 'resume' | 'cover' | 'jobAd' | 'brief' | 'answers' | 'referral';

interface GenerationCardProps {
  gen: AiGenerationRecord;
  selected?: boolean;
  onToggleSelect?: (id: string) => void;
}

export function GenerationCard({ gen, selected = false, onToggleSelect }: GenerationCardProps) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();
  const removeAiGeneration = useRemoveAiGeneration();
  const referrals = useReferrals(gen.jobUrl);
  const referralActions = useReferralActions();
  // Card collapses to its header row by default; click to reveal the body (#27).
  const [cardExpanded, setCardExpanded] = useState(false);
  const [expanded, setExpanded] = useState<SectionKey | null>(null);
  const [showExportModal, setShowExportModal] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const { resumeDraft, coverDraft, onEdit } = useGenerationDrafts(gen);

  const handleDelete = () => {
    setConfirmDelete(false);
    removeAiGeneration.mutate(gen.id);
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

  const exportState = useGenerationExport({ meta, resumeDraft, coverDraft });
  const { exporting, doExport } = exportState;

  const contacts = referrals.data ?? [];
  const hasOutput = Boolean(resumeDraft || coverDraft);

  return (
    <>
      <div className="surface-card rounded-xl overflow-hidden p-0">
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
                className="h-4 w-4 cursor-pointer accent-[color:var(--color-brand)] rounded border border-[var(--border-clear)]"
              />
            </div>
          )}

          <GenerationCardTitle
            gen={gen}
            expanded={cardExpanded}
            onToggle={() => setCardExpanded((v) => !v)}
          />

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
                <div className="border-t border-[var(--border-clear)] px-5 py-4">
                  <span className="mb-2 block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
                    {t('resumes.generated.keywords')}
                  </span>
                  <div className="flex flex-wrap gap-1.5">
                    {gen.topRequirements.map((req) => (
                      <span
                        key={req}
                        className="rounded-full border border-[var(--border-clear)] bg-muted px-2.5 py-1 text-[10px] text-foreground/55"
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
                <ReferralSection
                  contacts={contacts}
                  open={expanded === 'referral'}
                  onToggle={() => setExpanded(expanded === 'referral' ? null : 'referral')}
                  actions={referralActions}
                />
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Export — moved off the row into a modal (#28). */}
      <ExportPicker
        open={showExportModal}
        onClose={() => setShowExportModal(false)}
        format={exportState.format}
        onFormatChange={exportState.setFormat}
        templateId={exportState.template}
        onTemplateChange={exportState.setTemplate}
        templateOptions={TEMPLATE_OPTIONS}
        accent={exportState.accent}
        onAccentChange={exportState.setAccent}
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
            className="flex h-auto items-center gap-1.5 rounded-lg border-[var(--border-clear)] bg-muted px-3 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground"
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
