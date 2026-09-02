import { Briefcase, CheckCircle2, Cpu, FileText, Info, type LucideIcon } from 'lucide-react';
import { useRouter } from '@tanstack/react-router';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { ActionTile, Button } from '@ajh/ui';

import { useCanUseAI } from '@/components/ui/ModelSelector';
import { ROUTES } from '@/constants/routes/routes';
import { TRACKED_INTERACTION_TYPES } from '@/features/dashboard/constants';
import {
  deriveNextStep,
  type NextStepId,
  type StepSignal,
} from '@/features/dashboard/lib/next-step';
import { useEmbeddingStatus, useInteractions } from '@/services';
import { useSessionStore } from '@/store/session-store';

const STEP_ICON: Record<NextStepId, LucideIcon> = {
  resume: FileText,
  ai: Cpu,
  job: Briefcase,
};

/** Mirrors `AiSetupHint`'s `MESSAGE_KEY` — the copy for each `useCanUseAI`
 *  block reason, reused here so the AI step says which part of setup is
 *  missing instead of a generic nudge. Duplicated rather than imported: every
 *  gating call site in the app inlines its own copy of this tiny reason→key
 *  map (see the same note on `AISystemStatus`). */
const AI_REASON_KEY: Record<string, string> = {
  addApiKey: 'aiSetup.addApiKey',
  selectModel: 'aiSetup.selectModel',
  installCli: 'aiSetup.installCli',
  startOllama: 'aiSetup.startOllama',
  healthUnavailable: 'aiSetup.healthUnavailable',
};

/**
 * The one thing to do next, above the quick actions.
 *
 * Permanent by design: it collapses to a single line — "setup complete", or a
 * neutral "can't tell right now" when a signal query failed — rather than
 * disappearing, so the row never becomes a mystery gap and there is always a
 * route to help. Nothing is persisted; the three signals are re-derived from
 * their queries on every mount.
 */
export function NextStepTile() {
  const { t } = useTranslation();
  const router = useRouter();
  const setSettings = useSessionStore((s) => s.setSettings);

  const { data: embedding, isError: embeddingFailed } = useEmbeddingStatus();
  const { canUse: aiCanUse, reason: aiReason } = useCanUseAI();
  const { data: interactions, isError: interactionsFailed } = useInteractions();

  // Three states per signal, never two. `data === undefined` is `'pending'`
  // (cold boot — the tile stays hidden rather than advising a user to redo
  // something they already did), a REJECTED query is `'unavailable'` — its
  // `data` is `undefined` for good, so reading it as "still loading" hid this
  // row for the rest of the session.

  // Deliberate proxy: a document row carries no `kind`, so "any document
  // exists" has to stand for "a résumé exists". `documents.total` is the same
  // count `documents.list()` returns, from a query the root layout already
  // mounts — without shipping every document's full text in to ask.
  const hasResume: StepSignal = embeddingFailed
    ? 'unavailable'
    : embedding === undefined
      ? 'pending'
      : embedding.documents.total > 0;

  // `useCanUseAI` blocks with NO reason while it is still checking (its
  // documented cold-boot convention); a reason means it has really answered.
  // It has no `isError` to read and needs none: it turns its own failed reads
  // into concrete reasons (`healthUnavailable`, `addApiKey`), so unlike a raw
  // query it can never sit reason-less forever.
  const aiUsable: StepSignal = aiCanUse ? true : aiReason == null ? 'pending' : false;

  // Interactions ONLY, filtered by the same allowlist the pipeline card counts
  // with — a `dismissed` posting is the opposite of a tracked one, and the two
  // surfaces sit inches apart. Applications are deliberately NOT consulted:
  // `useApplications()` ships every application's full job description onto the
  // home route to answer one boolean, and the app-global `applications:changed`
  // listener would re-fetch all of it while the user just sits on the
  // Dashboard. The cost of leaving it out is that a job tracked by hand or by
  // the extension, with no interaction row, does not satisfy the step — which
  // is why the step's copy asks the user to FIND a job (search a board, open a
  // posting: that is what writes an interaction) and never claims "track".
  const hasJob: StepSignal = interactionsFailed
    ? 'unavailable'
    : interactions === undefined
      ? 'pending'
      : (interactions as { interactionType?: string }[]).some((i) =>
          TRACKED_INTERACTION_TYPES.has(i.interactionType ?? '')
        );

  const next = deriveNextStep({ resume: hasResume, ai: aiUsable, job: hasJob });

  if (next.kind === 'pending') return null;

  if (next.kind === 'done' || next.kind === 'unavailable') {
    const settled = next.kind === 'done';
    return (
      <div
        data-testid={
          settled ? TEST_IDS.dashboard.nextStepDone : TEST_IDS.dashboard.nextStepUnavailable
        }
        className="mb-8 flex items-center gap-2 rounded-xl border border-[var(--border-clear)] bg-card px-4 py-2"
      >
        {settled ? (
          <CheckCircle2 size={15} className="shrink-0 text-emerald-400" />
        ) : (
          // Neutral, not an alarm: a failed status read is not the user's
          // problem to fix, and claiming "setup complete" on a question we
          // could not answer would be worse than either.
          <Info size={15} className="shrink-0 text-muted-foreground" />
        )}
        <span className="min-w-0 flex-1 text-sm text-foreground/70">
          {t(settled ? 'dashboard.nextStep.doneTitle' : 'dashboard.nextStep.unavailableTitle')}
        </span>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => void router.navigate({ to: ROUTES.SUPPORT })}
        >
          {t('dashboard.nextStep.help')}
        </Button>
      </div>
    );
  }

  const { step, done, total } = next;

  const openStep = () => {
    if (step === 'job') {
      void router.navigate({ to: ROUTES.JOBS });
      return;
    }
    // Exactly how `AiSetupHint` reaches a Settings section: select it in the
    // session store first, then navigate — the page reads `activeSection` on
    // mount, so navigating alone would land on `general`.
    setSettings({ activeSection: step === 'ai' ? 'ai' : 'resume' });
    void router.navigate({ to: ROUTES.SETTINGS });
  };

  const description =
    step === 'ai'
      ? t(AI_REASON_KEY[aiReason ?? ''] ?? 'dashboard.nextStep.ai.description')
      : t(`dashboard.nextStep.${step}.description`);

  return (
    <div className="mb-8" data-testid={TEST_IDS.dashboard.nextStepTile}>
      <ActionTile
        icon={STEP_ICON[step]}
        label={t(`dashboard.nextStep.${step}.title`)}
        description={description}
        badge={
          <span className="rounded-full bg-muted px-2 py-0.5 text-[10px] text-muted-foreground">
            {t('dashboard.nextStep.progress', { done, total })}
          </span>
        }
        active
        className="w-full"
        onClick={openStep}
      />
    </div>
  );
}
