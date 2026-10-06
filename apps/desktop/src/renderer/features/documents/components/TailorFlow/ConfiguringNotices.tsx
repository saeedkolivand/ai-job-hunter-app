import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { ErrorState } from '@ajh/ui';

interface ConfiguringNoticesProps {
  error: string | null | undefined;
  cancelled: boolean;
}

/**
 * What the wizard stage says when a run left no output behind.
 *
 * A start failure falls back to the wizard — surface WHY, not silence. A
 * terminal needsReview/cancelled/error is NOT shown here — ResultsPanel's own
 * status banner (`stage === 'done'`) owns that.
 *
 * Cancelling BEFORE any text streamed leaves no output, so the stage
 * derivation falls back to the wizard with nothing else to say it happened —
 * `session.cancel()` sets no `error`, and the error banner is gated on one. A
 * one-line acknowledgement instead of dead silence; stays until the next
 * `start()` moves the session state off `cancelled`.
 */
export function ConfiguringNotices({ error, cancelled }: ConfiguringNoticesProps) {
  const { t } = useTranslation();
  if (error) {
    return (
      <div data-testid={TEST_IDS.documents.generationError} className="mx-8 mb-4 shrink-0">
        <ErrorState
          title={t('autopilot.apply.error')}
          description={error}
          className="rounded-xl border border-red-400/20 bg-red-400/5 py-6"
        />
      </div>
    );
  }
  if (!cancelled) return null;
  return (
    <div
      data-testid={TEST_IDS.documents.generationCancelled}
      role="status"
      className="mx-8 mb-4 shrink-0 rounded-lg border border-[var(--border-clear)] bg-foreground/[0.02] px-4 py-3 text-[11px] text-foreground/60"
    >
      {t('autopilot.apply.cancelledNoOutput')}
    </div>
  );
}
