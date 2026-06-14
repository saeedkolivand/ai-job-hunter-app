import { ArrowLeft, HelpCircle, UserPlus, Wand2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useRef, useState } from 'react';
import { useForm, useWatch } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';

import type { AutopilotFoundJob } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, transition } from '@ajh/ui';

import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { scoreToLevel } from '@/features/autopilot/lib/match-level';
import type { TemplateId } from '@/lib/generate';
import { useExtractText, useResolveJobUrl } from '@/services';
import { useSessionStore } from '@/store/session-store';

import { ReferralModal } from '../ReferralModal';
import { ApplicationQuestionsModal } from './ApplicationQuestionsModal';
import { GeneratingPanel } from './GeneratingPanel';
import { tailorWizardSchema } from './lib/tailor-schema';
import { buildTailorDefaults, type TailorWizardState } from './lib/tailor-state';
import { ResultsPanel } from './ResultsPanel';
import { TailorWizard } from './TailorWizard';
import { useApplicationAnswers } from './useApplicationAnswers';
import { useTailorGeneration } from './useTailorGeneration';

interface Props {
  job: AutopilotFoundJob;
  resumeText?: string;
  board: string;
  onBack: () => void;
}

/**
 * Full-page tailoring flow for a single found job. A derived stage machine
 * (configuring → generating → done) renders the RHF wizard, the streaming panel,
 * or the results panel. Output persistence lives in the generation store (keyed
 * `autopilot:<jobUrl>`), so the stage is derived, never stored; the wizard form +
 * step are persisted in the session store so configuring survives a remount.
 */
export function ApplyPage({ job, resumeText, board, onBack }: Props) {
  const { t } = useTranslation();
  const model = useSelectedModel();
  const { canUse, reason } = useCanUseAI();
  const extractTextMutation = useExtractText();

  const { autopilot, setAutopilot } = useSessionStore();
  const { applyWizardStep: step, applyWizardForm, applyTemplateId, applyAtsMode } = autopilot;
  const setStep = (v: number) => setAutopilot({ applyWizardStep: v });
  // Sticky render-time template/ATS preference (single source of truth shared by
  // the preview and the export — see useTailorGeneration). Render-time only.
  const setTemplateId = (v: TemplateId) => setAutopilot({ applyTemplateId: v });
  const setAtsMode = (v: boolean) => setAutopilot({ applyAtsMode: v });

  // RHF owns the live editing layer; `applyWizardForm` is a one-shot seed. Seed
  // `defaultValues` ONCE — written back on step-advance and on generate.
  const initialForm = useRef<TailorWizardState>(applyWizardForm ?? buildTailorDefaults(resumeText));
  const methods = useForm<TailorWizardState>({
    defaultValues: initialForm.current,
    resolver: zodResolver(tailorWizardSchema),
    mode: 'onChange',
  });

  // The research toggle is an RHF field; the hook needs its live value.
  const researchCompany = useWatch({ control: methods.control, name: 'researchCompany' });

  const [referralOpen, setReferralOpen] = useState(false);
  const [questionsOpen, setQuestionsOpen] = useState(false);
  // "Edit settings" forces the configuring stage even though output exists; cleared
  // when the next run starts (output is intentionally preserved underneath).
  const [forceConfiguring, setForceConfiguring] = useState(false);

  const initialDesc = (job.description ?? '').trim();
  const resolved = useResolveJobUrl(job.url, !initialDesc);
  const fetchedDesc = (resolved.data?.description ?? '').trim();
  const jobDesc = initialDesc || fetchedDesc;
  const hasDesc = jobDesc.length > 0;
  const fetchingDesc = !initialDesc && resolved.isLoading;

  const gen = useTailorGeneration({
    contextId: `autopilot:${job.url}`,
    jobDesc,
    model,
    canUse,
    hasDesc,
    jobUrl: job.url,
    board,
    researchCompany,
    templateId: applyTemplateId,
    atsMode: applyAtsMode,
  });

  // Lifted out of ResultsPanel so the modal can fully unmount on close without
  // losing the user's picks/answers (the hook holds non-rehydrated local state)
  // and so an in-flight generation keeps running while the modal is closed.
  const questions = useApplicationAnswers({
    resume: methods.getValues('resume'),
    jobDesc,
    model,
    researchCompany,
    meta: gen.meta,
    canUse,
    hasDesc,
    jobUrl: job.url,
    board,
  });

  const handleUpload = async (file: File) => {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const res = await extractTextMutation.mutateAsync({ name: file.name, bytes });
    const text = (res?.text ?? '').trim();
    if (text) methods.setValue('resume', text, { shouldValidate: true, shouldDirty: true });
  };

  // Persist the form snapshot to the session store (mirrors CreationWizard).
  const persistForm = (values: TailorWizardState) => setAutopilot({ applyWizardForm: values });

  const handleStep = (next: number) => {
    persistForm(methods.getValues());
    setStep(next);
  };

  // Persist the form, drop any "edit settings" override, and start a run. Shared
  // by the wizard's Generate (validated values) and the results Regenerate.
  const startGeneration = (values: TailorWizardState) => {
    persistForm(values);
    setForceConfiguring(false);
    void gen.generate(values.resume, values.outputType);
  };

  // Stage derivation: in-flight FIRST, then output, else the wizard. "Edit
  // settings" overrides to configuring while leaving the existing output intact.
  const hasOutput = !!(gen.resumeOut || gen.coverOut);
  const stage = gen.generating
    ? 'generating'
    : hasOutput && !forceConfiguring
      ? 'done'
      : 'configuring';

  // The target that produced (or is producing) the output. Persisted form value
  // is the source of truth once a run starts; falls back to the live form value.
  const generatedTarget = applyWizardForm?.outputType ?? methods.getValues('outputType');

  return (
    <div className="flex h-full flex-col">
      {/* Slim page header (persists across all stages) */}
      <div className="flex shrink-0 items-center gap-3 border-b border-white/[0.06] px-8 py-4">
        <Button
          onClick={onBack}
          variant="ghost"
          className="shrink-0 gap-1.5 text-foreground/50 hover:text-foreground/80"
        >
          <ArrowLeft size={14} /> {t('autopilot.apply.back')}
        </Button>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <Wand2 size={14} className="shrink-0 text-brand-soft" />
            <span className="truncate text-base font-semibold text-foreground/90">{job.title}</span>
            {typeof job.score === 'number' && (
              <span className="shrink-0 rounded-full bg-brand/10 px-2 py-0.5 text-[10px] font-medium text-brand-soft">
                {t(`autopilot.wizard.filter.matchLevel.${scoreToLevel(job.score)}`)}{' '}
                {t('autopilot.apply.match')}
              </span>
            )}
          </div>
          <div className="truncate text-[11px] text-foreground/40">
            {job.company}
            {job.location ? ` · ${job.location}` : ''}
          </div>
        </div>
        {stage === 'done' && (
          <Button
            variant="glass"
            onClick={() => setQuestionsOpen(true)}
            className="shrink-0 gap-1.5 text-brand-soft"
          >
            <HelpCircle size={13} /> {t('autopilot.apply.questions.title')}
            {questions.selected.size > 0 && (
              <span className="rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] text-brand-soft">
                {questions.selected.size}
              </span>
            )}
          </Button>
        )}
        <Button
          variant="glass"
          onClick={() => setReferralOpen(true)}
          className="shrink-0 gap-1.5 text-brand-soft"
        >
          <UserPlus size={13} /> {t('autopilot.referral.open')}
        </Button>
      </div>

      {/* Stage body */}
      <div className="min-h-0 flex-1">
        <AnimatePresence mode="wait">
          <motion.div
            key={stage}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={transition.fast}
            className="h-full"
          >
            {stage === 'configuring' && (
              <TailorWizard
                methods={methods}
                step={step}
                setStep={handleStep}
                jobDesc={jobDesc}
                hasDesc={hasDesc}
                fetchingDesc={fetchingDesc}
                jobUrl={job.url}
                onUpload={handleUpload}
                uploading={extractTextMutation.isPending}
                canUse={canUse}
                reason={reason}
                onGenerate={startGeneration}
              />
            )}

            {stage === 'generating' && (
              <GeneratingPanel
                target={generatedTarget}
                phase={gen.phase}
                phaseLabel={gen.phaseLabel}
                thinking={gen.thinking}
                output={gen.output}
                onCancel={() => gen.abort()}
              />
            )}

            {stage === 'done' && (
              <ResultsPanel
                target={generatedTarget}
                jobDesc={jobDesc}
                activeOut={gen.activeOut}
                setActiveOut={gen.setActiveOut}
                templateId={applyTemplateId}
                atsMode={applyAtsMode}
                onTemplateChange={setTemplateId}
                onAtsModeChange={setAtsMode}
                output={gen.output}
                onEdit={gen.editActiveOutput}
                meta={gen.meta}
                copied={gen.copied}
                onCopy={() => void gen.copy()}
                exportOpen={gen.exportOpen}
                setExportOpen={gen.setExportOpen}
                onExport={(fmt) => void gen.exportAs(fmt)}
                onRegenerate={() => startGeneration(methods.getValues())}
                onEditSettings={() => setForceConfiguring(true)}
                onReferral={() => setReferralOpen(true)}
              />
            )}
          </motion.div>
        </AnimatePresence>
      </div>

      {questionsOpen && (
        <ApplicationQuestionsModal {...questions} onClose={() => setQuestionsOpen(false)} />
      )}

      {referralOpen && (
        <ReferralModal
          job={job}
          resume={methods.getValues('resume')}
          onClose={() => setReferralOpen(false)}
        />
      )}
    </div>
  );
}
