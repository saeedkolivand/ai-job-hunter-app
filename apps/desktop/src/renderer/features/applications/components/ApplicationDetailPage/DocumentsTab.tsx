import { HelpCircle, MessagesSquare, UserPlus } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import type { AiGenerationRecord, Application, AutopilotFoundJob } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, CardSkeleton } from '@ajh/ui';

import {
  TailorFlow,
  type TailorFlowController,
  type TailorFlowPersistence,
} from '@/features/documents/components/TailorFlow';
import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
import type { RawDoc } from '@/lib/doc-record';
import { useDocuments, useDocumentText, useUpdateApplication } from '@/services';
import { useSessionStore } from '@/store/session-store';

interface DocumentsTabProps {
  application: Application;
  matchingGenerations: AiGenerationRecord[];
}

const COUNT_BADGE = 'rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] text-brand-soft';

/**
 * Documents tab — a full-height host for the shared {@link TailorFlow} generator
 * seeded with the user's default résumé, mirroring the autopilot apply flow.
 * Wizard / template / ATS persistence lives on the `applicationApply` session
 * slice (this surface owns it); TailorFlow surfaces a controller so the toolbar
 * can drive its Questions / Referral modals.
 */
export function DocumentsTab({ application, matchingGenerations }: DocumentsTabProps) {
  const { t } = useTranslation();
  const applicationApply = useSessionStore((s) => s.applicationApply);
  const setApplicationApply = useSessionStore((s) => s.setApplicationApply);
  const [controller, setController] = useState<TailorFlowController | null>(null);
  const updateApplication = useUpdateApplication();

  // Debounce-persist job-ad edits from TailorFlow back to application.jobDescription
  // so the Interview prep tab (and BriefTab) can read the updated text without
  // navigating away and back. 600ms debounce avoids a mutation per keystroke.
  // Refs keep the unmount flush free of stale-closure issues (no dep on application/mutate).
  // The id is captured together with the text at schedule time so a reuse of this
  // component instance for a different application (before the timer fires) cannot
  // flush A's text onto B's id.
  const jdPersistTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingJd = useRef<{ id: string; text: string } | null>(null);
  const mutateRef = useRef(updateApplication.mutate);
  mutateRef.current = updateApplication.mutate;

  const flushJd = () => {
    if (jdPersistTimer.current !== null) {
      clearTimeout(jdPersistTimer.current);
      jdPersistTimer.current = null;
    }
    if (pendingJd.current !== null) {
      mutateRef.current({ id: pendingJd.current.id, jobDescription: pendingJd.current.text });
      pendingJd.current = null;
    }
  };

  const handleJobDescChange = (text: string) => {
    pendingJd.current = { id: application.id, text };
    if (jdPersistTimer.current !== null) clearTimeout(jdPersistTimer.current);
    jdPersistTimer.current = setTimeout(flushJd, 600);
  };

  // Flush any pending edit on unmount instead of discarding it — this prevents
  // the edit from being lost when the user switches tabs before the 600ms fires.
  // All state accessed here is via refs so the empty-dep array is correct: the
  // cleanup reads the live ref values at the time it runs, not stale captures.
  const flushJdRef = useRef(flushJd);
  flushJdRef.current = flushJd;
  useEffect(
    () => () => {
      flushJdRef.current();
    },
    []
  );

  // Seed the résumé text ONCE at mount — wait for BOTH the documents list (which
  // resolves `defaultResumeId`) and the default résumé text so the one-shot
  // wizard seed is present before TailorFlow mounts. `useDefaultResumeId` reads
  // `useDocuments` internally; while that list loads it returns `null`, so we
  // must gate on the list load too or TailorFlow seeds empty and locks it in.
  const docsQuery = useDocuments();
  const defaultResumeId = useDefaultResumeId();
  const resumeQuery = useDocumentText(defaultResumeId);

  if (docsQuery.isLoading || (!!defaultResumeId && resumeQuery.isLoading)) {
    return (
      <div className="h-full overflow-y-auto px-6 py-5">
        <CardSkeleton />
      </div>
    );
  }

  // Prefer the autopilot one-shot seed (deep-link from Apply), then the user's
  // default résumé, then the most recent matching generation.
  const seedResumeText =
    (applicationApply.applySeedResume ?? '') ||
    (resumeQuery.data ?? '') ||
    (matchingGenerations[0]?.resumeText ?? '');

  // Seed the id ONLY when the seeded text IS the default résumé's text — the
  // autopilot one-shot and a previous generation's output have no saved-document
  // backing, and an id that doesn't match the visible text is the exact drift
  // `useResumeInput`'s `selectDoc` contract exists to prevent.
  const seedResumeDocId =
    seedResumeText && seedResumeText === resumeQuery.data
      ? (defaultResumeId ?? undefined)
      : undefined;

  // Generation-store session key. Empty job URLs (`z.string().default('')`) would
  // collide for every URL-less application, bleeding one application's live
  // tailoring session into another — so key those by the stable application id.
  // Real URLs keep the `autopilot:` key so the live session is shared across the
  // autopilot apply surface and this detail tab.
  const contextId =
    application.jobUrl.trim() === '' ? `app:${application.id}` : `autopilot:${application.jobUrl}`;

  const job: AutopilotFoundJob = {
    title: application.title,
    company: application.company,
    url: application.jobUrl,
    location: undefined,
    description: application.jobDescription || undefined,
    foundAt: application.createdAt,
    salaryMin: application.salaryMin,
    salaryMax: application.salaryMax,
    salaryCurrency: application.salaryCurrency,
  };

  // Self-describing read: only trust `applyRun` when it was written FOR this
  // application. Evaluated at render time (not in an effect), so it's correct
  // on the very first render even when this tab mounts (default tab) before
  // the parent's applyForId-reset effect has had a chance to run — see
  // `ApplicationApplySlice.applyRun`'s doc comment for the full hazard.
  const applyRun =
    applicationApply.applyRun?.forId === application.id ? applicationApply.applyRun : null;

  // A persisted `resumeDocId` can outlive the document it points at: advance a
  // wizard step (which snapshots the form into the memory-only
  // `applyWizardForm`), delete that résumé on the Documents page, come back.
  // `resume_source` is ID-WINS with NO fallback — `resolve_resume` answers
  // `resume not found: <id>` and never consults `resumeText`, which
  // `useTailorPipeline` has already blanked precisely BECAUSE an id was set. So
  // a stale id fails the whole run with an opaque id echo while the résumé text
  // sits visible on screen. Drop the id and let the run use that text; this is
  // the one place that holds both the persisted form and the live document
  // list, so it is the only place that can tell.
  const persistedWizardForm = applicationApply.applyWizardForm;
  // `useDocuments` is TYPED as `DocumentRecord[]` (`id`) but the backend really
  // returns `_id` — `useDefaultResume` casts through `RawDoc` for exactly this
  // reason, and `defaultResumeId` (what we compare against) is a `_id`. Reading
  // `.id` here would typecheck and be `undefined` at runtime, quietly stripping
  // EVERY persisted id instead of only stale ones.
  const rawDocs = (docsQuery.data ?? []) as unknown as RawDoc[];
  const knownDocIds = new Set(rawDocs.map((d) => d._id));
  const wizardForm =
    persistedWizardForm?.resumeDocId && !knownDocIds.has(persistedWizardForm.resumeDocId)
      ? { ...persistedWizardForm, resumeDocId: undefined }
      : persistedWizardForm;

  const persistence: TailorFlowPersistence = {
    wizardStep: applicationApply.applyWizardStep,
    wizardForm,
    templateId: applicationApply.applyTemplateId,
    atsMode: applicationApply.applyAtsMode,
    accent: applicationApply.applyAccent,
    letterLayoutId: applicationApply.applyLetterLayoutId,
    runId: applyRun?.runId ?? null,
    runJobId: applyRun?.jobId ?? null,
    setWizardStep: (v) => setApplicationApply({ applyWizardStep: v }),
    setWizardForm: (v) => setApplicationApply({ applyWizardForm: v }),
    setTemplateId: (v) => setApplicationApply({ applyTemplateId: v }),
    setAtsMode: (v) => setApplicationApply({ applyAtsMode: v }),
    setAccent: (v) => setApplicationApply({ applyAccent: v }),
    setLetterLayoutId: (v) => setApplicationApply({ applyLetterLayoutId: v }),
    setRun: (ids) =>
      setApplicationApply({
        applyRun: ids ? { forId: application.id, runId: ids.runId, jobId: ids.jobId } : null,
      }),
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Toolbar — Questions (only on `done`) + Referral */}
      <div className="flex shrink-0 items-center justify-end gap-2 border-b border-[var(--border-soft)] px-8 py-3">
        {controller?.stage === 'done' && (
          <Button
            variant="glass"
            onClick={() => controller.openQuestions()}
            className="shrink-0 gap-1.5 text-brand-soft"
          >
            <HelpCircle size={13} /> {t('autopilot.apply.questions.title')}
            {controller.questionsCount > 0 && (
              <span className={COUNT_BADGE}>{controller.questionsCount}</span>
            )}
          </Button>
        )}
        <Button
          variant="glass"
          disabled={!controller}
          onClick={() => controller?.openInterviewQuestions()}
          className="shrink-0 gap-1.5 text-brand-soft"
        >
          <MessagesSquare size={13} /> {t('applications.detail.interview.title')}
          {controller && controller.interviewQuestionsCount > 0 && (
            <span className={COUNT_BADGE}>{controller.interviewQuestionsCount}</span>
          )}
        </Button>
        <Button
          variant="glass"
          disabled={!controller}
          onClick={() => controller?.openReferral()}
          className="shrink-0 gap-1.5 text-brand-soft"
        >
          <UserPlus size={13} /> {t('autopilot.referral.open')}
        </Button>
      </div>

      {/* Shared tailoring body — full-height, matching the autopilot apply flow */}
      <div className="min-h-0 flex-1">
        <TailorFlow
          job={job}
          resumeText={seedResumeText}
          resumeDocId={seedResumeDocId}
          board={application.board ?? ''}
          contextId={contextId}
          jobUrl={application.jobUrl}
          seedGeneration={matchingGenerations[0]}
          persistence={persistence}
          onController={setController}
          applicationId={application.id}
          initialSummary={application.jobSummary ?? undefined}
          onJobDescChange={handleJobDescChange}
        />
      </div>
    </div>
  );
}
