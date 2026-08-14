import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { useForm, useWatch } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';

import type { AiGenerationRecord, AutopilotFoundJob } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { ErrorState, transition } from '@ajh/ui';

import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { useInterviewQuestions } from '@/hooks/use-interview-questions';
import type { LetterLayoutId, TemplateId } from '@/lib/generate';
import { shouldSeedResearchDefault } from '@/lib/research-company-default';
import { useActiveModelCapabilities, useResolveJobUrl } from '@/services';

import { ApplicationQuestionsModal } from './ApplicationQuestionsModal';
import { GeneratingPanel } from './GeneratingPanel';
import { InterviewQuestionsModal } from './InterviewQuestionsModal';
import { tailorWizardSchema } from './lib/tailor-schema';
import { buildTailorDefaults, type TailorWizardState } from './lib/tailor-state';
import { ReferralModal } from './ReferralModal';
import { ResultsPanel, type TailorRunState } from './ResultsPanel';
import { TailorWizard } from './TailorWizard';
import { useApplicationAnswers } from './useApplicationAnswers';
import { useJobAdSummary } from './useJobAdSummary';
import { useTailorPipeline } from './useTailorPipeline';

export type { TailorWizardState };

// A short carried description (e.g. an Adzuna API snippet, ~200–400 chars) is
// worth re-resolving: the URL fetch (which now follows the aggregator redirect)
// may reach the fuller ad. Re-resolve when the carried text is short OR empty,
// then prefer whichever description is longer.
// ponytail: 800-char floor separates aggregator snippets from full ads; raise if
// real full ads legitimately come in shorter.
const SHORT_DESC_FLOOR = 800;

type TailorFlowStage = 'configuring' | 'generating' | 'done';

/** A terminal `ResumePipelineState` → the results panel's status banner. Any
 *  non-terminal/idle state (including a cold-entry redisplay of a PAST run,
 *  which never ran in this session) reads as a clean `'done'` — there is
 *  nothing more to say about it here. */
function toRunState(state: string): TailorRunState {
  if (state === 'needsReview' || state === 'cancelled' || state === 'error') return state;
  return 'done';
}

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
  /** Per-export document accent (6-hex); undefined = template palette. */
  accent?: string;
  /** Per-export cover-letter layout; undefined → the backend renders classic. */
  letterLayoutId?: LetterLayoutId;
  /** The staged run this session started/reconnected to — reconnect target
   *  for `useTailorPipeline`, survives navigating away and back. */
  runId: string | null;
  runJobId: string | null;
  setWizardStep: (v: number) => void;
  setWizardForm: (v: TailorWizardState) => void;
  setTemplateId: (v: TemplateId) => void;
  setAtsMode: (v: boolean) => void;
  setAccent: (v: string | undefined) => void;
  setLetterLayoutId: (v: LetterLayoutId) => void;
  /** Persists (or clears, with `null`) the reconnect target as ONE atomic
   *  write — a single host-store update instead of two, so a host that
   *  keys the write on the ids together (see `ApplicationApplySlice.applyRun`)
   *  never observes a run/job id pair from two different applications. */
  setRun: (ids: { runId: string; jobId: string } | null) => void;
}

export interface TailorFlowProps {
  job: AutopilotFoundJob;
  resumeText?: string;
  board: string;
  /** Session key for the host's own bookkeeping (e.g. `autopilot:<jobUrl>`) —
   *  TailorFlow itself no longer keys any session state on it (the staged
   *  pipeline's own session lives in `persistence.runId`/`runJobId`), but
   *  callers still pass it for logging/future use. */
  contextId: string;
  /** Saved onto the AiGeneration record. */
  jobUrl: string;
  /**
   * Latest persisted generation for this job, if any — supplies the letter
   * text (and, on a cold entry with no reconnected run, the résumé text and
   * quality report too; see `useTailorPipeline`).
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
 * (configuring → generating → done) rendering the RHF wizard, the staged
 * quality pipeline's 4-step checklist, or the results panel, plus the
 * Questions + Referral modals. The host owns the slim header and the
 * persistence slice; TailorFlow surfaces a controller so the header can drive
 * its modals and read the derived stage.
 *
 * Output lives in the staged run record + the job's aggregate document (see
 * `useTailorPipeline`), so the stage is derived, never stored here; the
 * wizard form + step + run-reconnect ids are persisted via the injected
 * `persistence` slice so configuring (and an in-flight run) survive a remount.
 */
export function TailorFlow({
  job,
  resumeText,
  board,
  jobUrl,
  seedGeneration,
  persistence,
  onController,
  applicationId,
  initialSummary,
  onJobDescChange,
}: TailorFlowProps) {
  const { t } = useTranslation();
  const model = useSelectedModel();
  const { canUse, reason } = useCanUseAI();

  const step = persistence.wizardStep;
  const setStep = persistence.setWizardStep;
  // Sticky render-time template/ATS preference (single source of truth shared by
  // the preview and the export — see useTailorPipeline). Render-time only.
  const setTemplateId = persistence.setTemplateId;
  const setAtsMode = persistence.setAtsMode;
  const setAccent = persistence.setAccent;
  const setLetterLayoutId = persistence.setLetterLayoutId;

  // Capability-driven default for the "search company" toggle: default ON when
  // the active model can web-search. Read from the Rust capability matrix (never
  // a TS mirror), so a new provider needs no change here.
  const caps = useActiveModelCapabilities();
  const supportsWebSearch = caps.data?.supportsWebSearch ?? false;

  // RHF owns the live editing layer; `persistence.wizardForm` is a one-shot seed.
  // Seed `defaultValues` ONCE — written back on step-advance and on generate.
  // Only a FRESH (unpersisted) form takes the capability-driven research default;
  // a restored form keeps the user's saved choice. Lazy `useState` initializer so
  // `buildTailorDefaults` runs once, not on every render.
  const startedFresh = useRef(persistence.wizardForm == null);
  const [initialForm] = useState<TailorWizardState>(
    () => persistence.wizardForm ?? buildTailorDefaults(resumeText, supportsWebSearch)
  );
  const methods = useForm<TailorWizardState>({
    defaultValues: initialForm,
    resolver: zodResolver(tailorWizardSchema),
    mode: 'onChange',
  });

  // The research toggle is an RHF field; the questions/interview assistants
  // below need its live value (the staged run itself takes no such field —
  // see the shared schema's doc comment on `ResumePipelineRunRequest`).
  const researchCompany = useWatch({ control: methods.control, name: 'researchCompany' });

  // Keep the "search company" default in sync with the active model's capability:
  // seed it when the capability resolves after a cold-cache seed, and RE-seed it
  // on a mid-session model switch that flips the capability — but only for a fresh
  // form and only until the user touches the toggle (RHF's dirty flag guards the
  // override, so an explicit choice is never clobbered). The DECISION is the shared
  // `shouldSeedResearchDefault` helper; RHF owns the state. `lastSeededResearch`
  // tracks the last-seeded capability (seeded from the fresh form's construction
  // value) so a no-change resolve is a no-op.
  const researchDirty = !!methods.formState.dirtyFields.researchCompany;
  const lastSeededResearch = useRef<boolean | null>(
    startedFresh.current ? (caps.isSuccess ? supportsWebSearch : null) : null
  );
  useEffect(() => {
    if (!startedFresh.current) return;
    const { seed, value } = shouldSeedResearchDefault({
      capabilityResolved: caps.isSuccess,
      supportsWebSearch,
      userTouched: researchDirty,
      lastSeededValue: lastSeededResearch.current,
    });
    if (!seed) return;
    lastSeededResearch.current = value;
    methods.setValue('researchCompany', value, { shouldDirty: false });
  }, [caps.isSuccess, supportsWebSearch, researchDirty, methods]);

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

  const gen = useTailorPipeline({
    jobDesc,
    // The résumé the wizard tailors FROM — the quality panel's "Re-check" needs
    // it as validation context (RHF owns the live value; `start` passes its
    // own validated copy per run).
    sourceResume: methods.getValues('resume'),
    jobUrl,
    jobTitle: job.title,
    companyName: job.company,
    board,
    canUse,
    hasDesc,
    templateId: persistence.templateId,
    atsMode: persistence.atsMode,
    accent: persistence.accent,
    letterLayoutId: persistence.letterLayoutId,
    latestGeneration: seedGeneration,
    initialRunId: persistence.runId,
    initialJobId: persistence.runJobId,
    onRunStarted: persistence.setRun,
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
    void gen.start(values);
  };

  // Stage derivation: in-flight FIRST, then output, else the wizard. "Edit
  // settings" overrides to configuring while leaving the existing output intact.
  const stage: TailorFlowStage = gen.busy
    ? 'generating'
    : gen.hasOutput && !forceConfiguring
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
        currentStep={gen.currentStep}
        stageLabel={gen.stageLabel}
        thinking={gen.thinking}
        // Whichever document is currently streaming: the letter stage's own
        // buffer once it has content, the résumé's before/otherwise. Nothing
        // streams past it (validate/repair/humanize make no visible calls),
        // so this is the right live text for every later step too.
        output={gen.letterDraft || gen.draft}
        onCancel={gen.cancel}
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
        accent={persistence.accent}
        letterLayoutId={persistence.letterLayoutId}
        onTemplateChange={setTemplateId}
        onAtsModeChange={setAtsMode}
        onAccentChange={setAccent}
        onLetterLayoutChange={setLetterLayoutId}
        output={gen.output}
        onEdit={gen.editActiveOutput}
        meta={gen.meta}
        report={gen.report}
        pipelineReview={gen.pipelineReview}
        onRecheck={gen.recheck}
        rechecking={gen.rechecking}
        copied={gen.copied}
        onCopy={() => void gen.copy()}
        exportOpen={gen.exportOpen}
        setExportOpen={gen.setExportOpen}
        onExport={(fmt) => void gen.exportAs(fmt)}
        runState={toRunState(gen.state)}
        error={gen.error}
        stoppedReason={gen.stoppedReason}
        runs={gen.runs}
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

      {/* A start failure falls back to the wizard above — surface WHY, not
          silence. A terminal needsReview/cancelled/error is NOT shown here —
          ResultsPanel's own status banner (`stage === 'done'`) owns that. */}
      {stage === 'configuring' && gen.error && (
        <div data-testid={TEST_IDS.documents.generationError} className="mx-8 mb-4 shrink-0">
          <ErrorState
            title={t('autopilot.apply.error')}
            description={gen.error}
            className="rounded-xl border border-red-400/20 bg-red-400/5 py-6"
          />
        </div>
      )}

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
