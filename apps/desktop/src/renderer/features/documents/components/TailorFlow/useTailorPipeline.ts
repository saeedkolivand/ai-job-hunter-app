import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { type AiGenerationRecord, detectLanguage, toLanguageCode } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import type { QualityPipelineReview } from '@/components/generation/QualityReportPanel';
import { useQualityRecheck } from '@/hooks/use-quality-recheck';
import {
  type ResumePipelineSession,
  useResumePipelineSession,
} from '@/hooks/use-resume-pipeline-session';
import { errorClass, errorDetail } from '@/lib/error-class';
import {
  buildFilename,
  buildSectionVerdicts,
  countryFromLocation,
  exportDOCX,
  exportPDF,
  exportTXT,
  type GenerationMeta,
  type LetterLayoutId,
  parseFabrications,
  parseQualityReport,
  PERSIST_DEBOUNCE_MS,
  type QualityReport,
  resolveMarket,
  type TemplateId,
  unresolvedCount,
} from '@/lib/generate';
import { RESUME_PIPELINE_BUSY_STATES } from '@/lib/machines/resume-pipeline.machine';
import { COPY_FEEDBACK_MS } from '@/lib/timings';
import { keys } from '@/services/query-client';
import { useUpdateAiGeneration } from '@/services/use-ai-generations';
import {
  usePipelineRunsForJob,
  useRegenerateSection,
  useResolveFabrication,
} from '@/services/use-resume-pipeline';

import { pipelineStepForStage } from './lib/pipeline-steps';
import type { TailorWizardState } from './lib/tailor-state';

interface Params {
  jobDesc: string;
  /** The résumé the wizard is tailoring FROM — context for the quality panel's
   *  "Re-check" (the run itself takes it per call via `start`). */
  sourceResume: string;
  /** The found job's URL — empty for an unlinked (pasted/no-URL) posting. Sent
   *  through honestly; never fabricated (see the plan's gotcha on this). */
  jobUrl: string;
  jobTitle: string;
  companyName: string;
  /** Free-text job location as written on the ad (e.g. "New York, NY, US",
   *  "Köln, Deutschland") — feeds {@link countryFromLocation} for the export
   *  market. Empty/absent when the posting doesn't state one. */
  jobLocation?: string;
  /** The board the job came from (e.g. "linkedin"), text-path posting identity. */
  board: string;
  canUse: boolean;
  hasDesc: boolean;
  templateId: TemplateId;
  atsMode: boolean;
  accent?: string;
  letterLayoutId?: LetterLayoutId;
  /**
   * This job's aggregate `ai_generations` record, if any — the staged pipeline
   * writes résumé AND letter text onto it directly, so this is where the
   * LETTER text comes from (the résumé instead comes from the run detail,
   * `PipelineRunDetail.resumeText`, which the run's own `get` keeps current).
   * Recomputed live by the host every render (same prop TailorFlow already
   * threads through as `seedGeneration`) — never cached here.
   */
  latestGeneration?: AiGenerationRecord;
  /** Reconnect target — the `{runId, jobId}` persisted for this contextId. */
  initialRunId?: string | null;
  initialJobId?: string | null;
  /** Called once a run starts (or reconnects) so the host can persist the ids
   *  for next time (survives navigation away and back). */
  onRunStarted?: (ids: { runId: string; jobId: string }) => void;
}

/** {@link resolveTargetLanguage}'s result: the language to actually use for
 *  THIS run, plus whether that answer is confident. `language` is always a
 *  real 2-letter code (every downstream consumer — the prompt, the date
 *  formatter, `resolveMarket` — needs one); `confident` is the separate axis
 *  that decides what may be REMEMBERED. See the function doc comment. */
interface TargetLanguageResolution {
  language: string;
  confident: boolean;
}

/**
 * Resolve the tailor run's target language — an explicit, ordered precedence
 * chain (owner decision, see the plan's "What to build" §1/§3): a GUESS must
 * never be preferred over a confident answer, and must never be REMEMBERED
 * as one either.
 *
 * 1. The persisted `targetLanguage` — the field the STAGED PIPELINE actually
 *    writes (`target_language` → `AiGenerationRecord.targetLanguage`,
 *    `commands/resume_pipeline/mod.rs:712`). #1003's "keep the regenerate
 *    language" branch read `resumeLanguage`/`jobAdLanguage` instead, which
 *    the staged pipeline leaves EMPTY (`empty_record()`), so it was dead on
 *    this flow — this is the fix.
 * 2. The persisted `jobAdLanguage` — the fast (AIGeneratePage) path's own
 *    field, a legitimate target per `metadata.ts:211` (`targetLanguage:
 *    jobAdLanguage`, i.e. the SAME "target = the ad's language" answer as
 *    tier 1, just from the other write path).
 * 3. A fresh, confident detection of the CURRENT job ad (`detectLanguage`
 *    already returns `'unknown'` rather than a low-confidence guess — <20
 *    chars, franc `'und'`, or an unmapped code).
 * 4. `'en'` — the LAST resort. Not a confident fact: every downstream
 *    consumer needs a concrete 2-letter code to run generation with, so this
 *    function cannot return "unknown" here. This is the ONLY tier where
 *    `confident` is `false`.
 *
 * `resumeLanguage` — the SOURCE résumé's language — is deliberately never
 * read. It is the second door the English-lock bug (Defect B) walks through:
 * an English résumé applying to a German job is not, on its own, evidence
 * the candidate wants an English document; the job ad's language is the only
 * legitimate signal for a TARGET.
 *
 * Each candidate is normalized ({@link toLanguageCode}) and validated
 * INDEPENDENTLY before the next tier is tried — the SAME persisted field
 * carries two shapes across writers (`extractMetadata`'s heuristic fallback
 * writes a display NAME like "German"; every other writer stores an ISO
 * code), and a short-circuit on presence-without-validity would send a
 * perfectly good lower tier to detection unread.
 *
 * `confident` is what CALLERS use to decide what may reach the wire: sending
 * the tier-4 guess to `session.start` and having Rust persist it verbatim
 * would let a FUTURE run's tier 1 prefer that guess forever — exactly the
 * bug this chain exists to close. See `start`'s own comment for how the
 * caller keeps a guess off the wire without a schema change (Rust's own
 * `ai_generations::pick` already treats an empty incoming field as "keep
 * whatever is stored").
 *
 * Known limit (not fixable here, not worth a test): the renderer detects
 * with **franc**, Rust's `validate::content` checks with **whatlang** — two
 * different third-party models can legitimately disagree on the same text.
 */
export function resolveTargetLanguage(
  // Deliberately its own small shape, not `Pick<AiGenerationRecord, …>`: both
  // fields are OPTIONAL here (a caller may not have a record at all yet),
  // where `AiGenerationRecord`'s own fields are always-present strings. A
  // real `AiGenerationRecord` (including one with `resumeLanguage` set) still
  // satisfies this structurally — see the pure-function tests, which pass a
  // full record to prove `resumeLanguage` is present on the input yet never
  // read.
  latestGeneration: { targetLanguage?: string; jobAdLanguage?: string } | undefined,
  jobDesc: string
): TargetLanguageResolution {
  const persistedTarget = toLanguageCode(latestGeneration?.targetLanguage ?? '');
  if (/^[a-z]{2}$/.test(persistedTarget)) {
    return { language: persistedTarget, confident: true };
  }
  const persistedJobAd = toLanguageCode(latestGeneration?.jobAdLanguage ?? '');
  if (/^[a-z]{2}$/.test(persistedJobAd)) {
    return { language: persistedJobAd, confident: true };
  }
  const detected = detectLanguage(jobDesc);
  if (detected !== 'unknown') {
    return { language: detected, confident: true };
  }
  return { language: 'en', confident: false };
}

/**
 * Runs the staged quality pipeline for the tailor flow — the replacement for
 * `useTailorGeneration`'s one-shot path (PR-3 of the staged-cutover plan).
 * Thin adapter over {@link useResumePipelineSession}: this hook owns only
 * page-local UI state (active doc, copy/export, inline-edit overrides) plus
 * the translation/formatting glue the session doesn't know about.
 */
export function useTailorPipeline({
  jobDesc,
  sourceResume,
  jobUrl,
  jobTitle,
  companyName,
  jobLocation,
  board,
  canUse,
  hasDesc,
  templateId,
  atsMode,
  accent,
  letterLayoutId,
  latestGeneration,
  initialRunId,
  initialJobId,
  onRunStarted,
}: Params) {
  const { t } = useTranslation();
  const notify = useNotification();
  const qc = useQueryClient();
  const updateAiGeneration = useUpdateAiGeneration();
  const regenerate = useRegenerateSection();
  const resolveFabrication = useResolveFabrication();
  const runs = usePipelineRunsForJob(jobUrl).data ?? [];

  const session = useResumePipelineSession(initialRunId, initialJobId);

  // `onRunStarted` is host-supplied and, on the real DocumentsTab/TailorFlow
  // wiring, a FRESH arrow every render (and calling it writes a Zustand
  // slice, which always returns a new object → re-renders the host → a new
  // arrow again). Reading it from a ref keeps the effect below from ever
  // listing it as a dependency, so a host re-render alone can't re-fire it —
  // only an actual new `{runId, jobId}` can. `persistedRunRef` is a second,
  // independent guard: even if this effect DOES re-run for the same run
  // (e.g. a remount that re-seeds identical ids), it's a no-op rather than a
  // redundant host write. Together these close the infinite update loop
  // (`onRunStarted` → host state → new arrow → effect refires → …).
  const onRunStartedRef = useRef(onRunStarted);
  onRunStartedRef.current = onRunStarted;
  const persistedRunRef = useRef<string | null>(null);
  useEffect(() => {
    if (!session.runId || !session.jobId) return;
    const key = `${session.runId}|${session.jobId}`;
    if (persistedRunRef.current === key) return;
    persistedRunRef.current = key;
    onRunStartedRef.current?.({ runId: session.runId, jobId: session.jobId });
  }, [session.runId, session.jobId]);

  // The 4-step checklist position — keeps the LAST known step for a stage
  // name this build doesn't map (see `pipelineStepForStage`), never regresses.
  const [currentStep, setCurrentStep] = useState(0);
  const stageName = session.stage?.stage;
  useEffect(() => {
    if (!stageName) return;
    setCurrentStep((prev) => pipelineStepForStage(stageName, prev));
  }, [stageName]);

  // A stage name this build doesn't have a `pipeline.stage.*` copy entry for
  // (added server-side after this renderer shipped) falls back to the
  // machine's own translated coarse state — never the raw snake_case wire
  // name, which would leak straight onto the panel.
  const stageLabel = session.stage
    ? t(`pipeline.stage.${session.stage.stage}`, {
        defaultValue: t(`pipeline.state.${session.state}`, { defaultValue: '' }),
      })
    : t(`pipeline.state.${session.state}`, { defaultValue: '' });

  // Modal-local, ephemeral UI — fine to reset on remount.
  const [activeOut, setActiveOut] = useState<'resume' | 'cover'>('resume');
  const [copied, setCopied] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);

  // Local overrides for a hand-edit — the run record / aggregate are the
  // source of truth until the user types, exactly like the fast path's
  // session-store outputs were.
  const [resumeOverride, setResumeOverride] = useState<string | null>(null);
  const [letterOverride, setLetterOverride] = useState<string | null>(null);

  // CR-2: unmount previously CLEARED these timers without flushing — leaving
  // a tab (or a host remount of `DocumentsTab`) inside the debounce window
  // silently dropped the hand-edit; `resumeOverride`/`letterOverride` die
  // with the unmount too, so nothing recovers it. Matches DocumentsTab's own
  // `flushJd` posture (`ApplicationDetailPage/index.tsx`) exactly: the
  // pending PAYLOAD lives in a ref (captured at schedule time, not re-read at
  // flush time), one `flushPersist` function is the sole place a write is
  // fired — the debounce timeout and the unmount cleanup both just call it —
  // and a ref indirection keeps the unmount effect's empty deps honest
  // without a stale closure over `flushPersist`/`mutate`.
  const persistTimers = useRef<{
    resume?: ReturnType<typeof setTimeout>;
    cover?: ReturnType<typeof setTimeout>;
  }>({});
  const pendingEdits = useRef<{
    resume?: { id: string; text: string };
    cover?: { id: string; text: string };
  }>({});
  const mutateAiGenerationRef = useRef(updateAiGeneration.mutate);
  mutateAiGenerationRef.current = updateAiGeneration.mutate;

  const flushPersist = useCallback((field: 'resume' | 'cover') => {
    const timer = persistTimers.current[field];
    if (timer) {
      clearTimeout(timer);
      persistTimers.current[field] = undefined;
    }
    const pending = pendingEdits.current[field];
    if (!pending) return;
    pendingEdits.current[field] = undefined;
    mutateAiGenerationRef.current(
      field === 'resume'
        ? { id: pending.id, resumeText: pending.text }
        : { id: pending.id, coverLetterText: pending.text }
    );
  }, []);

  const flushPersistRef = useRef(flushPersist);
  flushPersistRef.current = flushPersist;
  useEffect(
    () => () => {
      flushPersistRef.current('resume');
      flushPersistRef.current('cover');
    },
    []
  );

  // `session.detail` is the LIVE run's own document — present once this
  // session started or reconnected to a run. `latestGeneration` (the job's
  // aggregate, threaded through from a live query one level up) is what
  // fills a COLD entry instead: no `runId` was ever persisted for this
  // session (a fresh app start, or a different surface produced the run),
  // but the posting already has a saved result. Same fallback shape both
  // texts already need for the letter (which has no run-detail source at
  // all) — the résumé just has one more rung.
  const resumeOut =
    resumeOverride ?? session.detail?.resumeText ?? latestGeneration?.resumeText ?? '';
  const coverOut = letterOverride ?? latestGeneration?.coverLetterText ?? '';
  const output = activeOut === 'resume' ? resumeOut : coverOut;
  const hasOutput = !!(resumeOut || coverOut);

  // Structurally identical to the renderer's own `QualityReport` (see
  // `PipelineQualityReport`'s doc comment) — every slot the fast path's parser
  // produces is present here too, plus the pipeline-only `fabrications`, which
  // `QualityReportSlot` already documents as opaque additional data. No cast
  // needed: `PipelineQualityReportSlot` is a strict superset. Prefers the live
  // run's own report, but falls back to the aggregate's persisted wrapper
  // (parsed the same way the fast path always has) whenever the live one is
  // absent — both for a cold entry (no `session.detail` at all) AND for a run
  // that ended before the `validate` stage ever ran (cancel, or a deadline
  // stop at a stage boundary), which leaves `session.detail.report === null`.
  // Without the fallback either case blanks the whole quality panel even
  // though the aggregate still holds a perfectly good report from an earlier
  // run.
  const report: QualityReport | null =
    session.detail?.report ?? parseQualityReport(latestGeneration?.qualityReport);

  // Regenerate must keep writing in whatever language the last run actually
  // produced, not re-detect from `jobDesc` and collapse to English the moment
  // detection can't tell (empty/short text, an unmapped ISO code, franc
  // returning `und`) — `latestGeneration` already carries the answer, WHEN it
  // is a confident one (see {@link resolveTargetLanguage}'s doc comment for
  // the owner decision this chain implements: persist/prefer only a confident
  // value, never a guess).
  const { language: targetLanguage, confident: targetLanguageConfident } = useMemo(
    () =>
      resolveTargetLanguage(
        {
          targetLanguage: latestGeneration?.targetLanguage,
          jobAdLanguage: latestGeneration?.jobAdLanguage,
        },
        jobDesc
      ),
    [jobDesc, latestGeneration?.targetLanguage, latestGeneration?.jobAdLanguage]
  );

  // Export/preview market — derived from the found job's free-text `location`
  // (unlike `AIGeneratePage`'s extracted meta, this hook never had a structured
  // `jobCountry`), falling back to the letter-language default when the
  // location doesn't resolve to a known country (see `resolveMarket`). Still
  // far better than the `undefined` this hook used to send, which the Rust
  // exporter silently treats as "intl" and skips market-specific conventions
  // (e.g. DIN 5008 for `de`, US Letter for `us`) entirely.
  const market = useMemo(
    () =>
      resolveMarket({
        jobCountry: countryFromLocation(jobLocation, targetLanguage),
        targetLanguage,
      }),
    [jobLocation, targetLanguage]
  );

  // A best-effort stand-in for the fast path's model-extracted `meta`: the
  // staged run resolves job title/company server-side (or from the request,
  // on the text path) but never echoes a structured meta object back over
  // IPC. Seeded from `latestGeneration` (the job's persisted aggregate, when
  // one exists) rather than fabricated — a blank `topRequirements` silently
  // strips the "top requirement hits" metric out of a Re-check
  // (`use-quality-recheck.ts`), and a non-null-but-empty `meta` short-circuits
  // the answers assistant's own metadata extraction
  // (`useApplicationAnswers.ts` treats any non-null `meta` as already
  // detected). `jobTitle`/`companyName` stay off the live props, not the
  // record — this hook already gets those fresh off the current posting every
  // render, which a persisted record can lag. `candidateName` only feeds the
  // export FILENAME's cosmetic fallback ("Candidate-…") — ADR-0021 keeps the
  // editor as the header's authority at export time, unaffected by this.
  const meta: GenerationMeta | null = hasOutput
    ? {
        candidateName: latestGeneration?.candidateName || '',
        jobTitle,
        companyName,
        resumeLanguage: latestGeneration?.resumeLanguage || targetLanguage,
        jobAdLanguage: latestGeneration?.jobAdLanguage || targetLanguage,
        mismatch: latestGeneration?.mismatch ?? false,
        targetLanguage,
        topRequirements: latestGeneration?.topRequirements ?? [],
      }
    : null;

  // Refresh the aggregate (letter text, quality-report fallback context) and
  // the autopilot score once the SESSION reaches a terminal state — nothing
  // else invalidates either, and the app's global refetch-on-focus/mount is
  // off. Keyed on `session.state`, not `session.detail?.status`: a run that
  // dies via the umbrella `job.failed` path (a full queue, no configured
  // provider, a deleted résumé, …) sends `ERROR` straight to the machine with
  // NO run record ever written (see `use-resume-pipeline-session.ts`'s
  // `handleJobEvent` doc) — `detail` stays `null` forever, so gating on its
  // `status` field would leave the panel showing stale pre-run data
  // indefinitely. "Terminal" here is "not idle and not busy" rather than an
  // enumerated list, so a future terminal state added to the machine is
  // covered without an edit here. `pipelineState` as the sole dependency
  // mirrors this hook's other re-render-loop guards (`onRunStartedRef`,
  // `persistedRunRef`): a re-render while already terminal is a no-op, only
  // an actual state change re-fires this.
  const pipelineState = session.state;
  useEffect(() => {
    if (pipelineState === 'idle' || RESUME_PIPELINE_BUSY_STATES.includes(pipelineState)) return;
    void qc.invalidateQueries({ queryKey: keys.aiGenerations.all });
    void qc.invalidateQueries({ queryKey: keys.autopilot.all });
  }, [qc, pipelineState]);

  const onReportChange = useCallback(
    (next: QualityReport) => {
      if (!session.runId) return;
      qc.setQueryData<PipelineRunDetail | null>(keys.pipeline.run(session.runId), (old) =>
        old ? { ...old, report: next } : old
      );
    },
    [qc, session.runId]
  );

  const { recheck, rechecking } = useQualityRecheck({
    report,
    meta,
    sourceResume,
    jobAd: jobDesc,
    docKind: activeOut === 'resume' ? 'resume' : 'coverLetter',
    onReportChange,
    resumeText: resumeOut,
    coverLetterText: coverOut,
    generating: session.busy,
    jobUrl,
    board,
  });

  // Section-fix / fabrication-review extras for the ACTIVE document — this
  // session's OWN run is always the posting's newest (nothing else can start
  // one from here), so unlike `TailoredResumePanel` there is no older-run
  // gate to apply.
  // Read off the RAW `PipelineQualityReport` (not the renderer-shaped `report`
  // above) — its slot type declares `fabrications`, where `QualityReportSlot`
  // deliberately doesn't (it's opaque additional data there).
  const rawSlot =
    activeOut === 'resume' ? session.detail?.report?.resume : session.detail?.report?.coverLetter;
  const sections = useMemo(() => buildSectionVerdicts(rawSlot?.report, output), [rawSlot, output]);
  const fabrications = useMemo(() => parseFabrications(rawSlot?.fabrications), [rawSlot]);
  const runId = session.detail?.runId;

  // Unresolved fabrication count across BOTH documents — the Rust
  // `needsReview` verdict (`still_needs_review`) scans resume AND coverLetter,
  // but `fabrications` above (and the ACTIVE-tab-only `pipelineReview` it
  // feeds) only ever reflects whichever document is on screen. A run flagged
  // for review while the user is looking at the OTHER, clean document must
  // not read as "0 claims" just because this session hasn't switched tabs.
  const resumeReportSlot = session.detail?.report?.resume;
  const coverReportSlot = session.detail?.report?.coverLetter;
  const openClaimsTotal = useMemo(() => {
    const resumeUnresolved = resumeReportSlot
      ? unresolvedCount(parseFabrications(resumeReportSlot.fabrications), resumeOut)
      : 0;
    const coverUnresolved = coverReportSlot
      ? unresolvedCount(parseFabrications(coverReportSlot.fabrications), coverOut)
      : 0;
    return resumeUnresolved + coverUnresolved;
  }, [resumeReportSlot, coverReportSlot, resumeOut, coverOut]);
  const pipelineReview: QualityPipelineReview | undefined = runId
    ? {
        documentText: output,
        sections,
        fabrications,
        onFixSection: (sectionKey, note) =>
          regenerate.mutate({ runId, sectionKey, ...(note ? { note } : {}) }),
        fixingSection: regenerate.isPending ? (regenerate.variables?.sectionKey ?? null) : null,
        fixError: regenerate.error
          ? t('autopilot.apply.wizard.results.fixFailed', { detail: errorDetail(regenerate.error) })
          : null,
        onResolveFabrication: (issueKey, decision) =>
          resolveFabrication.mutate({ runId, issueKey, decision }),
        resolvingIssueKey: resolveFabrication.isPending
          ? (resolveFabrication.variables?.issueKey ?? null)
          : null,
        resolveError: resolveFabrication.error
          ? t('autopilot.apply.wizard.results.resolveFailed', {
              detail: errorDetail(resolveFabrication.error),
            })
          : null,
        ...(session.detail?.metrics.repairRounds != null
          ? { repairRounds: session.detail.metrics.repairRounds }
          : {}),
        ...(session.detail?.metrics.reverted != null
          ? { repairReverted: session.detail.metrics.reverted }
          : {}),
      }
    : undefined;

  const editActiveOutput = (text: string) => {
    if (activeOut === 'resume') setResumeOverride(text);
    else setLetterOverride(text);
    const id = latestGeneration?.id;
    if (!id) return;
    const field = activeOut === 'resume' ? 'resume' : 'cover';
    // Captured NOW, read at flush time (debounce fire OR unmount) instead of
    // closing over `text`/`id` — mirrors DocumentsTab's `pendingJd` exactly.
    pendingEdits.current[field] = { id, text };
    const existing = persistTimers.current[field];
    if (existing) clearTimeout(existing);
    persistTimers.current[field] = setTimeout(() => flushPersist(field), PERSIST_DEBOUNCE_MS);
  };

  const start = async (values: TailorWizardState) => {
    if (!canUse || !hasDesc) return null;
    setResumeOverride(null);
    setLetterOverride(null);
    setCurrentStep(0);
    const resumeId = values.resumeDocId ?? '';
    // Computed HERE, not in a memo — a memo evaluated at mount would go stale
    // in a session left open across midnight, and a wrong date on a cover
    // letter is worse than none. `targetLanguage` is a valid BCP-47 tag from
    // `detectLanguage`, but `toLocaleDateString` still throws `RangeError` on
    // a malformed one — fall back to the runtime default locale rather than
    // failing the whole run over a date string.
    let today: string;
    try {
      today = new Date().toLocaleDateString(targetLanguage, {
        day: 'numeric',
        month: 'long',
        year: 'numeric',
      });
    } catch {
      today = new Date().toLocaleDateString(undefined, {
        day: 'numeric',
        month: 'long',
        year: 'numeric',
      });
    }
    // What goes on the WIRE deliberately differs from `targetLanguage` above
    // when the resolution wasn't confident (the tier-4 'en' guess): Rust
    // writes whatever `targetLanguage` it receives straight onto the
    // `ai_generations` aggregate, UNCONDITIONALLY, at
    // `commands/resume_pipeline/mod.rs:712` — there is no confidence concept
    // on that write, and it cannot be added from here (Rust is out of scope
    // for this change; see the handoff note in the PR). Sending '' instead of
    // the guess needs no schema change: `ResumePipelineRunSchema.targetLanguage`
    // (`packages/shared/src/schemas/index.ts`) only `.default('en')`s an
    // ABSENT key, so an explicit '' passes through untouched;
    // `normalize_language('')` already treats it as 'en' for every
    // prompt/validation use; and `ai_generations::pick` (`ai_generations/mod.rs:958`)
    // already keeps whatever the record had for an empty INCOMING field — the
    // exact mechanism that already protects `resumeLanguage`/`jobAdLanguage`
    // from a stale overwrite. A guess therefore still runs THIS generation
    // (via the local `targetLanguage` above) but is never remembered, so it
    // can never win `resolveTargetLanguage`'s tier-1 "persisted confident
    // value" branch on a later run. `AiGenerateRequest.locale` (the draft/
    // cover-letter stages' own use of this same value) is set from the raw
    // target_language and is unread by every provider adapter today — an
    // empty value there is a no-op, not a behavior change (verified by
    // reading, not editing, `commands/ai_provider/**`).
    const wireTargetLanguage = targetLanguageConfident ? targetLanguage : '';
    const runId = await session.start({
      resumeId,
      resumeText: resumeId ? '' : values.resume,
      jobId: '',
      jobAdText: jobDesc,
      jobTitle,
      companyName,
      board,
      jobUrl,
      targetLanguage: wireTargetLanguage,
      // Same value the export/preview path already resolved via this hook's
      // `market` memo — sent through unchanged so the letter prompt and the
      // export layout agree on one market.
      market,
      today,
      topRequirements: [],
      coverLetterText: '',
      includeCoverLetter: values.outputType !== 'resume',
      researchCompany: values.researchCompany,
    });
    // `session.start` already logged the cause and set `error`/`state` — this
    // is the one thing it can't do itself: a transient, dismissable toast.
    // The persistent banner (rendered off the same `error`) is the durable
    // half of that pair.
    if (!runId) notify.error({ message: t('autopilot.apply.failed') });
    return runId;
  };

  const cancel = () => session.cancel();

  const copy = async () => {
    if (!output) return;
    await navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), COPY_FEEDBACK_MS);
  };

  const exportAs = async (fmt: 'pdf' | 'docx' | 'txt') => {
    setExportOpen(false);
    if (!output) return;
    const docType = activeOut === 'resume' ? 'resume' : 'cover-letter';
    const fileMeta: GenerationMeta = meta ?? {
      candidateName: '',
      jobTitle: '',
      companyName: '',
      resumeLanguage: 'en',
      jobAdLanguage: 'en',
      mismatch: false,
      targetLanguage: 'en',
      topRequirements: [],
    };
    const name = buildFilename(fileMeta, docType, fmt);
    try {
      if (fmt === 'pdf')
        await exportPDF(
          output,
          name,
          docType,
          meta ?? undefined,
          templateId,
          atsMode,
          market,
          accent,
          letterLayoutId
        );
      else if (fmt === 'docx')
        await exportDOCX(
          output,
          name,
          docType,
          meta ?? undefined,
          templateId,
          atsMode,
          market,
          accent,
          letterLayoutId
        );
      else exportTXT(output, name);
    } catch (err) {
      console.error('[export] failed', {
        format: fmt,
        docType,
        error: errorClass(err),
      });
      notify.error({
        message: err instanceof Error && err.message ? err.message : t('common.exportFailed'),
      });
    }
  };

  return {
    state: session.state,
    busy: session.busy,
    starting: session.starting,
    currentStep,
    stageLabel,
    // The run's own backend-recorded start time (`pipeline_runs.started_at`),
    // for `GeneratingPanel`'s elapsed caption — anchoring on this instead of
    // the panel's own mount time is what survives a navigate-away-and-back
    // (see that prop's doc comment for why a remount needs it). `> 0`, not a
    // bare presence check: a `0`/negative epoch ms is never a real timestamp
    // (defensive — today's `now_ms()`-populated column never produces one),
    // and falling through to `null` here is what lets `GeneratingPanel`'s own
    // `runStartedAt ?? mountFallback` recover instead of anchoring the
    // caption on the Unix epoch and counting up from ~1970.
    runStartedAt:
      session.detail?.startedAt && session.detail.startedAt > 0 ? session.detail.startedAt : null,
    thinking: session.thinking,
    // The résumé pane's live stream — the letter's own (`session.letterDraft`)
    // is display-only in the same sense, exposed separately so a cover-only
    // run's checklist doesn't show résumé tokens under the "Generate" step.
    draft: session.draft,
    letterDraft: session.letterDraft,
    resumeOut,
    coverOut,
    activeOut,
    setActiveOut,
    output,
    hasOutput,
    error: session.error,
    stoppedReason: session.detail?.stoppedReason,
    copied,
    exportOpen,
    setExportOpen,
    start,
    cancel,
    copy,
    exportAs,
    editActiveOutput,
    meta,
    // Export/preview market — see the computation's doc comment above. The live
    // preview (GenerationOutput → PdfPreview) needs the SAME value the export
    // sends, or the on-screen letter's salutation/sign-off silently disagrees
    // with the downloaded one (the Rust exporter defaults to "intl" on `undefined`).
    market,
    report,
    pipelineReview,
    openClaimsTotal,
    recheck,
    rechecking,
    runs,
  };
}

export type TailorPipelineSession = ReturnType<typeof useTailorPipeline>;
export type { ResumePipelineSession };
