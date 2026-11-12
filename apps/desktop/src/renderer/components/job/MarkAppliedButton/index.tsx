import { CheckCircle2 } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { useSetApplicationStatus } from '@/services';

interface MarkAppliedButtonProps {
  applicationId: string;
  /** Current status; the action renders only while it is still `saved`. */
  status: string | undefined;
  size?: React.ComponentProps<typeof Button>['size'];
}

/** Explicit "I submitted this application" action — generating documents never records one. */
export function MarkAppliedButton({ applicationId, status, size }: MarkAppliedButtonProps) {
  const { t } = useTranslation();
  const setStatus = useSetApplicationStatus();
  const [failed, setFailed] = useState(false);
  const [done, setDone] = useState(false);
  const [seenStatus, setSeenStatus] = useState(status);

  // Adjust state during render: when the status really returns to `saved` after
  // having been past it (reverted elsewhere), our earlier success no longer holds.
  if (status !== seenStatus) {
    setSeenStatus(status);
    if (status === 'saved') setDone(false);
  }

  // After OUR successful call the focused button unmounts once the status flips;
  // keep a short live-region confirmation (only while the status stays past `saved`).
  if (status !== 'saved') {
    return done ? (
      <span role="status" className="shrink-0 text-xs text-foreground/70">
        {t('applications.detail.markedApplied')}
      </span>
    ) : null;
  }

  return (
    <>
      <Button
        variant="glass"
        size={size}
        // `done` while still `saved`: the refetch hasn't landed — a second click would
        // append another applied event.
        disabled={setStatus.isPending || done}
        onClick={() => {
          setFailed(false);
          setStatus.mutate(
            { id: applicationId, status: 'applied' },
            {
              // The command resolves `{ error }` on failure (it never rejects), so
              // `onError` only covers transport failures.
              onSuccess: (data) => (data.error ? setFailed(true) : setDone(true)),
              onError: () => setFailed(true),
            }
          );
        }}
        className="shrink-0 gap-1.5 text-brand-soft"
      >
        <CheckCircle2 size={13} /> {t('applications.detail.markApplied')}
      </Button>
      {failed && (
        <span role="alert" className="shrink-0 text-xs text-red-400">
          {t('applications.detail.markAppliedError')}
        </span>
      )}
    </>
  );
}
