import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { useForm, useWatch } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';

import type { AiGenerationRecord, AutopilotFoundJob } from '@ajh/shared';
import type { PipelineRunSummary } from '@ajh/shared/ipc';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { ErrorState, transition } from '@ajh/ui';

import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { useInterviewQuestions } from '@/hooks/use-interview-questions';
import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
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

/**
 * A terminal `ResumePipelineState` → the results panel's status banner.
 *
 * A COLD entry (a past run redisplayed from `latestGeneration`, never
 * started/reconnected in THIS session) leaves the machine at `idle` — its
 * own state has no opinion, but this posting's run list (`runs`, already
 * fetched) does: `runs[0]` is that same latest run's real, persisted status.
 * Falling back to a blind `'done'` there rendered a needsReview or failed
 * run as a clean success. Any OTHER non-terminal state (queued/drafting/…)
 * still reads as `'done'` — those only happen with a live session, which
 * `GeneratingPanel` owns instead.
 */
function toRunState(state: string, runs: PipelineRunSummary[]): TailorRunState {
  if (state === 'needsReview' || state === 'cancelled' || state === 'error') return state;
  if (state === 'idle') {
    const coldStatus = runs[0]?.status;
    if (coldStatus === 'needsReview') return 'needsReview';
    if (coldStatus === 'cancelled') return 'cancelled';
    if (coldStatus === 'failed') return 'error';
  }
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
    () => persistence.wizardForm ?? buildTailorDefaults(resumeText, supportsWebSearch, resumeDocId)
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
  // The form field: which saved document backs the text about to be GENERATED.
  // It must keep matching the visible text, because `useTailorPipeline` sends
  // `resumeText: ''` whenever it is set — a mismatched id would silently generate
  // from a different résumé than the one on screen.
  const pickedResumeDocId = useWatch({ control: methods.control, name: 'resumeDocId' });

  // The Score tab asks a DIFFERENT question, and the two answers are not always
  // the same document. Its own copy says "Scored against your saved résumé, not
  // the tailored version shown here", so it wants the saved document even when
  // the editor holds something else — and the autopilot apply path seeds
  // `ap.resumeText` (see `AutopilotPage`), a snapshot that can differ from the
  // document it was taken from, which is exactly when the strict field above must
  // stay unset. Reading that field alone left this tab permanently on "Save a
  // résumé to score" for every autopilot-originated application.
  //
  // Prefer an explicitly-picked document, fall back to the default — which is what
  // the Jobs page has always scored (`useDefaultResumeId`), and what this line's
  // previous comment already claimed to do.
  const defaultResumeId = useDefaultResumeId();
  const resumeId = pickedResumeDocId ?? defaultResumeId ?? undefined;

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
    targetLanguageConfident: gen.targetLanguageConfident,
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
    targetLanguageConfident: gen.targetLanguageConfident,
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

  const runState = toRunState(gen.state, gen.runs);

  // CR-7: a `role="status"` element that only enters the DOM once its
  // condition is already true (the cancelled-no-output hint below,
  // ResultsPanel's needsReview box) is unreliable — several screen readers
  // only announce a TEXT CHANGE inside an ALREADY-mounted live region, not
  // content that arrives in the same update as the region itself. This one
  // region stays mounted for TailorFlow's whole lifetime (every stage
  // transition); only its text changes. It's additive, not a replacement —
  // the two visual banners keep their own `role="status"` too, for AT/browser
  // combinations that DO handle a freshly-mounted status role; this is the
  // reliable fallback for the ones that don't. Same "announce the
  // TRANSITION, not the mount" posture as GeneratingPanel's per-step
  // announcer (H8).
  const [liveAnnouncement, setLiveAnnouncement] = useState('');
  const announcedKeyRef = useRef<'cancelled' | 'needsReview' | null>(null);
  useEffect(() => {
    const key =
      stage === 'configuring' && !gen.error && gen.state === 'cancelled'
        ? 'cancelled'
        : stage === 'done' && runState === 'needsReview'
          ? 'needsReview'
          : null;
    if (announcedKeyRef.current === key) return;
    announcedKeyRef.current = key;
    // CR-10: clearing to '' on the null branch (not leaving the previous
    // text in place) is the whole fix — for BOTH halves CodeRabbit raised.
    // Stale text: without this, a run that finishes cleanly after an
    // earlier cancel/needsReview kept exposing that old announcement in
    // the (still-mounted) region forever. Re-announcing an IDENTICAL
    // consecutive value: React bails out of a `setState` that's
    // `Object.is`-equal to the current value, so calling
    // `setLiveAnnouncement` with the SAME string twice in a row is not a
    // real DOM text mutation and may not re-announce — but `key` cannot
    // reach 'cancelled' (or 'needsReview') twice without passing through
    // `null` in between (starting a new run always moves `stage` off
    // 'configuring'/'done' first), so this clear always lands between two
    // occurrences of the same text, making the SECOND one a genuine ''→text
    // change again. No separate machinery needed for that half.
    if (key === 'cancelled') setLiveAnnouncement(t('autopilot.apply.cancelledNoOutput'));
    else if (key === 'needsReview') setLiveAnnouncement(t('pipeline.status.needsReview'));
    else setLiveAnnouncement('');
  }, [stage, gen.error, gen.state, runState, t]);

  // `AnimatePresence mode="wait"` swaps the whole stage subtree on every stage
  // change, which drops focus to `<body>` with nothing to restore it —
  // keyboard/AT users lose their place after every wizard step, generate, or
  // "Edit settings". Focus the (otherwise inert, tabIndex={-1}) stage body
  // itself on each transition, matching a route/modal-swap pattern.
  //
  // Two guards, both load-bearing:
  // - Skip the FIRST run (mount): the ref below only tracks CHANGES, so a
  //   fresh page load never steals focus into an unlabeled offscreen div.
  // - Bail when focus is already inside an open `ModalShell` dialog
  //   (`[aria-modal="true"]`) — Questions/Interview/Referral stay open
  //   across a `generating → done` transition (ApplicationDetailPage keeps
  //   them enabled while busy), and `useFocusTrap` only intercepts Tab, not
  //   a programmatic `.focus()` landing outside the trap. Pulling focus out
  //   from under an open dialog would leave Tab walking the background page.
  const stageBodyRef = useRef<HTMLDivElement>(null);
  const mountedStageRef = useRef<TailorFlowStage | null>(null);
  useEffect(() => {
    const isFirstRun = mountedStageRef.current === null;
    mountedStageRef.current = stage;
    if (isFirstRun) return;
    const activeEl = stageBodyRef.current?.ownerDocument.activeElement;
    if (activeEl instanceof Element && activeEl.closest('[aria-modal="true"]')) return;
    stageBodyRef.current?.focus();
  }, [stage]);

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

      {/* Cancelling BEFORE any text streamed leaves no output, so the stage
          derivation above falls back to the wizard with nothing else to say
          it happened — `session.cancel()` sets no `error`, and the banner
          above is gated on one. A one-line acknowledgement instead of dead
          silence; stays until the next `start()` moves `gen.state` off
          `cancelled`. */}
      {stage === 'configuring' && !gen.error && gen.state === 'cancelled' && (
        <div
          data-testid={TEST_IDS.documents.generationCancelled}
          role="status"
          className="mx-8 mb-4 shrink-0 rounded-lg border border-[var(--border-clear)] bg-foreground/[0.02] px-4 py-3 text-[11px] text-foreground/60"
        >
          {t('autopilot.apply.cancelledNoOutput')}
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
