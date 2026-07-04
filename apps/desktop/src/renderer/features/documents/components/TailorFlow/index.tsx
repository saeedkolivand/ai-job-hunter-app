import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { useForm, useWatch } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';

import type { AiGenerationRecord, AutopilotFoundJob } from '@ajh/shared';
import { transition } from '@ajh/ui';

import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { useInterviewQuestions } from '@/hooks/use-interview-questions';
import type { TemplateId } from '@/lib/generate';
import { useResolveJobUrl } from '@/services';

import { ApplicationQuestionsModal } from './ApplicationQuestionsModal';
import { GeneratingPanel } from './GeneratingPanel';
import { InterviewQuestionsModal } from './InterviewQuestionsModal';
import { tailorWizardSchema } from './lib/tailor-schema';
import { buildTailorDefaults, type TailorWizardState } from './lib/tailor-state';
import { ReferralModal } from './ReferralModal';
import { ResultsPanel } from './ResultsPanel';
import { TailorWizard } from './TailorWizard';
import { useApplicationAnswers } from './useApplicationAnswers';
import { useJobAdSummary } from './useJobAdSummary';
import { useTailorGeneration } from './useTailorGeneration';

export type { TailorWizardState };

// A short carried description (e.g. an Adzuna API snippet, ~200–400 chars) is
// worth re-resolving: the URL fetch (which now follows the aggregator redirect)
// may reach the fuller ad. Re-resolve when the carried text is short OR empty,
// then prefer whichever description is longer.
// ponytail: 800-char floor separates aggregator snippets from full ads; raise if
// real full ads legitimately come in shorter.
const SHORT_DESC_FLOOR = 800;

type TailorFlowStage = 'configuring' | 'generating' | 'done';

/**
 * Imperative surface a host can drive: it reads the derived `stage` + the
 * application-question count, and triggers the two modals (which live INSIDE
 * TailorFlow so they can fully unmount without losing the user's picks).
 */
export interface TailorFlowController {
  stage: TailorFlowStage;
  questionsCount: number;
  interviewQuestionsCount: number;
  openQuestions: () => void;
  openReferral: () => void;
  openInterviewQuestions: () => void;
}

/**
 * Wizard / template / ATS persistence is INJECTED by the host so each surface
 * (autopilot apply, application detail) owns its own store slice. The flow stays
 * stateless about WHERE these values live.
 */
export interface TailorFlowPersistence {
  wizardStep: number;
  wizardForm: TailorWizardState | null;
  templateId: TemplateId;
  atsMode: boolean;
  setWizardStep: (v: number) => void;
  setWizardForm: (v: TailorWizardState) => void;
  setTemplateId: (v: TemplateId) => void;
  setAtsMode: (v: boolean) => void;
}

export interface TailorFlowProps {
  job: AutopilotFoundJob;
  resumeText?: string;
  board: string;
  /** Generation-store session key (e.g. `autopilot:<jobUrl>`). */
  contextId: string;
  /** Saved onto the AiGeneration record. */
  jobUrl: string;
  /**
   * Latest persisted generation for this job, if any. When the live generation
   * session is empty (e.g. a cold app start), the flow seeds itself from this
   * record so it opens on the results (`done`) stage instead of the wizard.
   */
  seedGeneration?: AiGenerationRecord;
  persistence: TailorFlowPersistence;
  onController?: (c: TailorFlowController) => void;
  /** The tracked Application id — used to persist the job summary onto the application record. */
  applicationId?: string;
  /** Persisted job summary from the application record (pre-seeds the summary panel). */
  initialSummary?: string;
  /**
   * Called whenever the user edits the job-ad textarea, in addition to the
   * internal `setJobDescOverride`. DocumentsTab uses this to debounce-persist
   * the edit back to `application.jobDescription` so other tabs (e.g. Interview
   * prep) can read the updated text without requiring a page reload.
   * Autopilot callers that don't pass this prop are unaffected.
   */
  onJobDescChange?: (text: string) => void;
}

/**
 * The extracted BODY of the tailoring flow — a derived stage machine
 * (configuring → generating → done) rendering the RHF wizard, the streaming
 * panel, or the results panel, plus the Questions + Referral modals. The host
 * owns the slim header and the persistence slice; TailorFlow surfaces a
 * controller so the header can drive its modals and read the derived stage.
 *
 * Output persistence lives in the generation store (keyed by `contextId`), so
 * the stage is derived, never stored; the wizard form + step are persisted via
 * the injected `persistence` slice so configuring survives a remount.
 */
export function TailorFlow({
  job,
  resumeText,
  board,
  contextId,
  jobUrl,
  seedGeneration,
  persistence,
  onController,
  applicationId,
  initialSummary,
  onJobDescChange,
}: TailorFlowProps) {
  const model = useSelectedModel();
  const { canUse, reason } = useCanUseAI();

  const step = persistence.wizardStep;
  const setStep = persistence.setWizardStep;
  // Sticky render-time template/ATS preference (single source of truth shared by
  // the preview and the export — see useTailorGeneration). Render-time only.
  const setTemplateId = persistence.setTemplateId;
  const setAtsMode = persistence.setAtsMode;

  // RHF owns the live editing layer; `persistence.wizardForm` is a one-shot seed.
  // Seed `defaultValues` ONCE — written back on step-advance and on generate.
  const initialForm = useRef<TailorWizardState>(
    persistence.wizardForm ?? buildTailorDefaults(resumeText)
  );
  const methods = useForm<TailorWizardState>({
    defaultValues: initialForm.current,
    resolver: zodResolver(tailorWizardSchema),
    mode: 'onChange',
  });

  // The research toggle is an RHF field; the hook needs its live value.
  const researchCompany = useWatch({ control: methods.control, name: 'researchCompany' });

  const [referralOpen, setReferralOpen] = useState(false);
  const [questionsOpen, setQuestionsOpen] = useState(false);
  const [interviewOpen, setInterviewOpen] = useState(false);
  // "Edit settings" forces the configuring stage even though output exists; cleared
  // when the next run starts (output is intentionally preserved underneath).
  const [forceConfiguring, setForceConfiguring] = useState(false);

  const initialDesc = (job.description ?? '').trim();
  const resolved = useResolveJobUrl(job.url, initialDesc.length < SHORT_DESC_FLOOR);
  const fetchedDesc = (resolved.data?.description ?? '').trim();
  const [jobDescOverride, setJobDescOverride] = useState<string | null>(null);
  // Combine the local override with the optional host persist callback so the
  // host can react to edits (e.g. debounce-persist to application.jobDescription)
  // without TailorFlow caring about storage details.
  const handleJobDescEdit = (v: string) => {
    setJobDescOverride(v);
    onJobDescChange?.(v);
  };
  const jobDesc =
    jobDescOverride ?? (fetchedDesc.length > initialDesc.length ? fetchedDesc : initialDesc);
  const hasDesc = jobDesc.length > 0;
  // Show the loading state only when there's nothing to display yet (no snippet);
  // with a snippet present it renders immediately and upgrades silently on fetch.
  const fetchingDesc = !initialDesc && resolved.isLoading;

  const gen = useTailorGeneration({
    contextId,
    jobDesc,
    model,
    canUse,
    hasDesc,
    jobUrl,
    board,
    researchCompany,
    templateId: persistence.templateId,
    atsMode: persistence.atsMode,
  });

  // Lazy, résumé-independent AI summary of the job ad (shared by the wizard's
  // job-ad step and the results job-ad tab). Reuses the flow's detected meta.
  const jobAdSummary = useJobAdSummary({
    jobDesc,
    model,
    canUse,
    hasDesc,
    meta: gen.meta,
    applicationId,
    initialSummary,
  });

  // Cold-entry hydration: when a prior generation is persisted for this job and
  // the live session is empty, seed it so the flow opens on the results panel
  // (`done`) instead of the wizard. The store guards re-entry (no-op once a
  // session has output / a savedId / is generating), so this never clobbers
  // in-progress work; `gen.hydrate` is stable so the effect fires once per record.
  const hydrateSession = gen.hydrate;
  useEffect(() => {
    if (!seedGeneration) return;
    const { resumeText: savedResume, coverLetterText } = seedGeneration;
    if (!savedResume && !coverLetterText) return;
    hydrateSession({
      resumeOut: savedResume,
      coverOut: coverLetterText,
      savedId: seedGeneration.id,
      meta: {
        candidateName: seedGeneration.candidateName,
        jobTitle: seedGeneration.jobTitle,
        companyName: seedGeneration.companyName,
        resumeLanguage: seedGeneration.resumeLanguage,
        jobAdLanguage: seedGeneration.jobAdLanguage,
        mismatch: seedGeneration.mismatch,
        targetLanguage: seedGeneration.targetLanguage,
        topRequirements: seedGeneration.topRequirements,
      },
    });
  }, [seedGeneration, hydrateSession]);

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
    jobUrl,
    board,
    salaryMin: job.salaryMin,
    salaryMax: job.salaryMax,
    salaryCurrency: job.salaryCurrency,
  });

  // "Questions to ask the interviewer" — the second assistant. Same inputs; it
  // always gathers its own company/role research (not gated on the toggle).
  const interview = useInterviewQuestions({
    resume: methods.getValues('resume'),
    jobDesc,
    model,
    meta: gen.meta,
    canUse,
    hasDesc,
    jobUrl,
    board,
  });

  // Persist the form snapshot to the host's store (mirrors CreationWizard).
  const persistForm = (values: TailorWizardState) => persistence.setWizardForm(values);

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
  const stage: TailorFlowStage = gen.generating
    ? 'generating'
    : hasOutput && !forceConfiguring
      ? 'done'
      : 'configuring';

  // The target that produced (or is producing) the output. Persisted form value
  // is the source of truth once a run starts; falls back to the live form value.
  const generatedTarget = persistence.wizardForm?.outputType ?? methods.getValues('outputType');

  // Surface the imperative controller to the host (header triggers + derived stage).
  const questionsCount = questions.selected.size;
  const interviewQuestionsCount = interview.questions.length;
  useEffect(() => {
    onController?.({
      stage,
      questionsCount,
      interviewQuestionsCount,
      openQuestions: () => setQuestionsOpen(true),
      openReferral: () => setReferralOpen(true),
      openInterviewQuestions: () => setInterviewOpen(true),
    });
  }, [stage, questionsCount, interviewQuestionsCount, onController]);

  const stageRegistry: Record<TailorFlowStage, () => ReactNode> = {
    configuring: () => (
      <TailorWizard
        methods={methods}
        step={step}
        setStep={handleStep}
        jobDesc={jobDesc}
        onJobDescChange={handleJobDescEdit}
        hasDesc={hasDesc}
        fetchingDesc={fetchingDesc}
        jobUrl={job.url}
        canUse={canUse}
        reason={reason}
        onGenerate={startGeneration}
        jobAdSummary={jobAdSummary}
      />
    ),
    generating: () => (
      <GeneratingPanel
        target={generatedTarget}
        phase={gen.phase}
        phaseLabel={gen.phaseLabel}
        thinking={gen.thinking}
        output={gen.output}
        onCancel={() => gen.abort()}
      />
    ),
    done: () => (
      <ResultsPanel
        target={generatedTarget}
        jobDesc={jobDesc}
        onJobDescChange={handleJobDescEdit}
        hasDesc={hasDesc}
        fetchingDesc={fetchingDesc}
        jobUrl={job.url}
        jobAdSummary={jobAdSummary}
        activeOut={gen.activeOut}
        setActiveOut={gen.setActiveOut}
        templateId={persistence.templateId}
        atsMode={persistence.atsMode}
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
      />
    ),
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
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
            {stageRegistry[stage]()}
          </motion.div>
        </AnimatePresence>
      </div>

      {questionsOpen && (
        <ApplicationQuestionsModal
          {...questions}
          model={model}
          locale={gen.meta?.targetLanguage ?? 'en'}
          onClose={() => setQuestionsOpen(false)}
        />
      )}

      {interviewOpen && (
        <InterviewQuestionsModal {...interview} onClose={() => setInterviewOpen(false)} />
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
