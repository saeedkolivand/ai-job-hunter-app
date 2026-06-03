import { Loader2, Sparkles, Wand2, X } from 'lucide-react';
import { useState } from 'react';

import type { AutopilotFoundJob } from '@ajh/shared';
import { Button, ModalShell, SegmentedControl } from '@ajh/ui';

import { ResumeInputCard } from '@/components/resume/ResumeInputCard';
import { ModelSelector, useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { ThinkingBubble } from '@/features/ai-generate/components/ThinkingBubble';
import { useTranslation } from '@/lib/i18n';
import { useExtractText, useResolveJobUrl } from '@/services';

import { ApplicationQuestions } from './ApplicationQuestions';
import { GenerationOutput } from './GenerationOutput';
import { JobDescriptionPanel } from './JobDescriptionPanel';
import { type TailorTarget, useTailorGeneration } from './useTailorGeneration';

interface Props {
  job: AutopilotFoundJob;
  /** The resume the autopilot used — pre-fills the resume input. */
  resumeText?: string;
  /** The autopilot's base cover letter — reused as the starting point the
   *  cover-letter generation tailors to this job. */
  baseCoverLetter?: string;
  /** The board this autopilot searches — stored on the saved application record. */
  board: string;
  onClose: () => void;
}

/**
 * Tailor a resume / cover letter for a single autopilot-found job, inline.
 * Reuses the AI Generate primitives so the user never leaves the Autopilot page.
 */
export function ApplyJobModal({ job, resumeText, baseCoverLetter, board, onClose }: Props) {
  const { t } = useTranslation();
  const model = useSelectedModel();
  const { canUse, reason } = useCanUseAI();
  const extractTextMutation = useExtractText();

  const [resume, setResume] = useState(resumeText ?? '');
  const [target, setTarget] = useState<TailorTarget>('both');
  const [uploading, setUploading] = useState(false);
  // Opt-in company research — default off, so no extra web/LLM call unless asked.
  const [researchCompany, setResearchCompany] = useState(false);

  // Fetch the description on demand when the board's list scrape omitted it.
  const initialDesc = (job.description ?? '').trim();
  const resolved = useResolveJobUrl(job.url, !initialDesc);
  const fetchedDesc = (resolved.data?.description ?? '').trim();
  const jobDesc = initialDesc || fetchedDesc;
  const hasDesc = jobDesc.length > 0;
  const fetchingDesc = !initialDesc && resolved.isLoading;

  // Per-job session key — generation lives in the store under this id, so closing
  // and reopening the modal (or leaving the page) preserves the result.
  const gen = useTailorGeneration({
    contextId: `autopilot:${job.url}`,
    jobDesc,
    model,
    canUse,
    hasDesc,
    jobUrl: job.url,
    board,
    researchCompany,
    baseCoverLetter,
  });

  // Closing the modal no longer cancels — generation finishes in the background
  // and is restored on reopen. Use the Cancel button to abort explicitly.
  const close = () => onClose();

  const handleUpload = async (file: File) => {
    setUploading(true);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const res = await extractTextMutation.mutateAsync({
        name: file.name,
        bytes,
      });
      const text = (res?.text ?? '').trim();
      if (text) setResume(text);
    } finally {
      setUploading(false);
    }
  };

  const targets: { id: TailorTarget; label: string }[] = [
    { id: 'resume', label: t('autopilot.apply.target.resume') },
    { id: 'cover', label: t('autopilot.apply.target.cover') },
    { id: 'both', label: t('autopilot.apply.target.both') },
  ];

  return (
    <ModalShell open onClose={close} maxWidth="max-w-2xl">
      <div className="flex max-h-[85vh] flex-col">
        {/* Header */}
        <div className="flex items-start justify-between gap-3 border-b border-white/[0.08] px-5 py-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Wand2 size={14} className="shrink-0 text-brand-soft" />
              <span className="truncate text-sm font-semibold text-foreground/85">{job.title}</span>
            </div>
            <div className="mt-0.5 truncate text-[11px] text-foreground/40">
              {job.company}
              {job.location ? ` · ${job.location}` : ''}
            </div>
          </div>
          <Button
            onClick={close}
            className="h-auto shrink-0 border-transparent bg-transparent p-0 text-foreground/30 hover:text-foreground/60"
          >
            <X size={16} />
          </Button>
        </div>

        {/* Body */}
        <div className="flex-1 space-y-3 overflow-y-auto px-5 py-4">
          <JobDescriptionPanel
            jobDesc={jobDesc}
            hasDesc={hasDesc}
            fetchingDesc={fetchingDesc}
            jobUrl={job.url}
          />

          {/* Resume input */}
          <ResumeInputCard
            value={resume}
            onChange={setResume}
            onUpload={handleUpload}
            uploading={uploading}
          />

          {/* Model */}
          <ModelSelector />

          {/* Opt-in company research — improves the cover letter AND company-specific
              application answers, so it's offered regardless of the tailor target. */}
          <label className="flex cursor-pointer items-start gap-2 rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-2">
            <input
              type="checkbox"
              checked={researchCompany}
              onChange={(e) => setResearchCompany(e.target.checked)}
              className="mt-0.5 accent-brand"
            />
            <span className="min-w-0">
              <span className="block text-[11px] font-medium text-foreground/80">
                {t('autopilot.apply.research.label')}
              </span>
              <span className="block text-[10px] text-foreground/40">
                {t('autopilot.apply.research.hint')}
              </span>
            </span>
          </label>

          {/* Target + generate */}
          <div className="flex items-center justify-between gap-2">
            <SegmentedControl<TailorTarget>
              ariaLabel={t('autopilot.apply.target.label')}
              value={target}
              onChange={setTarget}
              options={targets.map(({ id, label }) => ({ value: id, label }))}
            />
            <div className="flex items-center gap-2">
              {gen.generating && (
                <Button
                  variant="glass"
                  size="sm"
                  onClick={() => gen.abort()}
                  className="border-red-400/20 text-red-300/80 hover:text-red-200"
                >
                  {t('autopilot.apply.cancel')}
                </Button>
              )}
              <Button
                variant="glass"
                size="sm"
                loading={gen.generating}
                disabled={!canUse || !hasDesc || gen.generating || !resume.trim()}
                onClick={() => void gen.generate(resume, target)}
              >
                {!gen.generating && <Sparkles size={13} />}
                {gen.generating ? t('autopilot.apply.generating') : t('autopilot.apply.generate')}
              </Button>
            </div>
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

          {/* Generation progress */}
          {gen.generating && (
            <div className="flex items-center gap-2 rounded-lg border border-brand/20 bg-brand/5 px-3 py-2 text-[11px] text-brand-soft">
              <Loader2 size={12} className="animate-spin" />
              {gen.phaseLabel}
              {target === 'both' && gen.phase !== 'analyzing' && (
                <span className="text-foreground/40">
                  · {gen.phase === 'resume' ? '1/2' : '2/2'}
                </span>
              )}
            </div>
          )}

          {/* Model reasoning — same box as AI Generate; self-hides when empty */}
          <ThinkingBubble thinking={gen.thinking} done={!gen.generating} />

          {/* Output */}
          {(gen.resumeOut || gen.coverOut) && (
            <GenerationOutput
              target={target}
              activeOut={gen.activeOut}
              setActiveOut={gen.setActiveOut}
              output={gen.output}
              onEdit={gen.editActiveOutput}
              editable={!gen.generating}
              meta={gen.meta}
              copied={gen.copied}
              onCopy={gen.copy}
              exportOpen={gen.exportOpen}
              setExportOpen={gen.setExportOpen}
              onExport={gen.exportAs}
            />
          )}

          {/* Optional application-questions assistant — résumé-grounded answers. */}
          <ApplicationQuestions
            resume={resume}
            jobDesc={jobDesc}
            model={model}
            researchCompany={researchCompany}
            meta={gen.meta}
            canUse={canUse}
            hasDesc={hasDesc}
            jobUrl={job.url}
            board={board}
          />
        </div>
      </div>
    </ModalShell>
  );
}
