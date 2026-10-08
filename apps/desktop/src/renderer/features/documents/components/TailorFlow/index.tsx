import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useEffect, useState } from 'react';

import type { AiGenerationRecord, AutopilotFoundJob } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { transition } from '@ajh/ui';

import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import type { LetterLayoutId, TemplateId } from '@/lib/generate';

import { ApplicationQuestionsModal } from './ApplicationQuestionsModal';
import { ConfiguringNotices } from './ConfiguringNotices';
import { GeneratingPanel } from './GeneratingPanel';
import { InterviewQuestionsModal } from './InterviewQuestionsModal';
import { type TailorFlowStage, toRunState } from './lib/tailor-stage';
import type { TailorWizardState } from './lib/tailor-state';
import { ReferralModal } from './ReferralModal';
import { ResultsPanel } from './ResultsPanel';
import { TailorWizard } from './TailorWizard';
import { useJobDescription } from './useJobDescription';
import { useLiveAnnouncement, useStageFocus } from './useStageEffects';
import { useTailorAssistants } from './useTailorAssistants';
import { useTailorForm } from './useTailorForm';
import { useTailorPipeline } from './useTailorPipeline';
import { WeakAnalysisNotice } from './WeakAnalysisNotice';

export type { TailorWizardState };

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
  /** The saved résumé backing `resumeText`, unedited — see `tailor-state.ts`'s
   *  doc on `resumeDocId`. Undefined when the caller can't vouch that
   *  `resumeText` IS that document's text (a generated/one-shot seed). */
  resumeDocId?: string;
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
  resumeDocId,
  board,
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
  // the preview and the export — see useTailorPipeline). Render-time only.
  const setTemplateId = persistence.setTemplateId;
  const setAtsMode = persistence.setAtsMode;
  const setAccent = persistence.setAccent;
  const setLetterLayoutId = persistence.setLetterLayoutId;

  const { methods, researchCompany, resumeId } = useTailorForm(
    persistence.wizardForm,
    resumeText,
    resumeDocId
  );

  const [referralOpen, setReferralOpen] = useState(false);
  const [questionsOpen, setQuestionsOpen] = useState(false);
  const [interviewOpen, setInterviewOpen] = useState(false);
  // "Edit settings" forces the configuring stage even though output exists; cleared
  // when the next run starts (output is intentionally preserved underneath).
  const [forceConfiguring, setForceConfiguring] = useState(false);

  const { jobDesc, hasDesc, fetchingDesc, handleJobDescEdit } = useJobDescription(
    job,
    onJobDescChange
  );

  // The target that produced (or is producing) the output. Persisted form value
  // is the source of truth once a run starts; falls back to the live form value.
  // Declared HERE, above the hook, because the hook needs it too: it is what
  // decides which document the results panel opens on, including on a cold
  // remount where no `start()` call survives to have seeded it.
  const generatedTarget = persistence.wizardForm?.outputType ?? methods.getValues('outputType');

  const gen = useTailorPipeline({
    jobDesc,
    // The résumé the wizard tailors FROM — the quality panel's "Re-check" needs
    // it as validation context (RHF owns the live value; `start` passes its
    // own validated copy per run).
    sourceResume: methods.getValues('resume'),
    jobUrl,
    jobTitle: job.title,
    companyName: job.company,
    jobLocation: job.location,
    board,
    canUse,
    hasDesc,
    target: generatedTarget,
    templateId: persistence.templateId,
    atsMode: persistence.atsMode,
    accent: persistence.accent,
    letterLayoutId: persistence.letterLayoutId,
    latestGeneration: seedGeneration,
    initialRunId: persistence.runId,
    initialJobId: persistence.runJobId,
    onRunStarted: persistence.setRun,
  });

  const { jobAdSummary, questions, interview } = useTailorAssistants({
    job,
    resume: methods.getValues('resume'),
    jobDesc,
    model,
    researchCompany,
    gen,
    canUse,
    hasDesc,
    jobUrl,
    board,
    applicationId,
    initialSummary,
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

  const runState = toRunState(gen.state, gen.runs);

  const liveAnnouncement = useLiveAnnouncement({
    stage,
    error: gen.error,
    state: gen.state,
    runState,
  });
  const stageBodyRef = useStageFocus(stage);

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
        resumeId={resumeId}
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
        runStartedAt={gen.runStartedAt}
        thinking={gen.thinking}
        // Whichever document is currently streaming: the letter stage's own
        // buffer once it has content, the résumé's before/otherwise. Nothing
        // streams past it (validate/repair/humanize make no visible calls),
        // so this is the right live text for every later step too.
        output={gen.letterDraft || gen.draft}
        // `letterDraft` alone is not enough: a cover-only run skips the
        // `draft` stage entirely, so there is no résumé stream to precede the
        // letter's first token and the pane spent the whole analyze → strategy
        // warm-up labelled "Resume".
        streamingTarget={gen.letterDraft || generatedTarget === 'cover' ? 'cover' : 'resume'}
        onCancel={gen.cancel}
      />
    ),
    done: () => (
      <ResultsPanel
        target={generatedTarget}
        hasResume={!!gen.resumeOut}
        jobDesc={jobDesc}
        onJobDescChange={handleJobDescEdit}
        hasDesc={hasDesc}
        fetchingDesc={fetchingDesc}
        jobUrl={job.url}
        resumeId={resumeId}
        jobAdSummary={jobAdSummary}
        activeOut={gen.activeOut}
        setActiveOut={gen.setActiveOut}
        templateId={persistence.templateId}
        atsMode={persistence.atsMode}
        accent={persistence.accent}
        letterLayoutId={persistence.letterLayoutId}
        market={gen.market}
        onTemplateChange={setTemplateId}
        onAtsModeChange={setAtsMode}
        onAccentChange={setAccent}
        onLetterLayoutChange={setLetterLayoutId}
        output={gen.output}
        onEdit={gen.editActiveOutput}
        meta={gen.meta}
        report={gen.report}
        pipelineReview={gen.pipelineReview}
        openClaims={gen.openClaimsTotal}
        onRecheck={gen.recheck}
        rechecking={gen.rechecking}
        copied={gen.copied}
        onCopy={() => void gen.copy()}
        exportOpen={gen.exportOpen}
        setExportOpen={gen.setExportOpen}
        onExport={(fmt) => void gen.exportAs(fmt)}
        runState={runState}
        // No live/reconnected session — a cold redisplay from `latestGeneration`,
        // so there is no interactive fix/resolve UI wired for it (see
        // `pipelineReview`'s `runId` gate). Tells the needsReview hint to say
        // so honestly instead of pointing at controls that don't exist here.
        cold={gen.state === 'idle'}
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
      {/* CR-7: persistently-mounted announcer — see the doc comment on
          `liveAnnouncement` above for why this exists alongside the visual
          banners' own `role="status"` rather than instead of them. */}
      <span
        data-testid={TEST_IDS.documents.liveAnnouncer}
        role="status"
        aria-live="polite"
        className="sr-only"
      >
        {liveAnnouncement}
      </span>

      {gen.weakAnalysis && (stage === 'generating' || stage === 'done') && <WeakAnalysisNotice />}

      {/* Stage body */}
      <div className="min-h-0 flex-1">
        <AnimatePresence mode="wait">
          <motion.div
            key={stage}
            ref={stageBodyRef}
            tabIndex={-1}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={transition.fast}
            className="h-full outline-none"
          >
            {stageRegistry[stage]()}
          </motion.div>
        </AnimatePresence>
      </div>

      {stage === 'configuring' && (
        <ConfiguringNotices
          error={gen.error}
          cancelled={gen.state === 'cancelled'}
          failed={gen.state === 'error'}
          stoppedReason={gen.stoppedReason}
        />
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
