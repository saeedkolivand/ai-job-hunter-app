import { CheckCircle2 } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { useSetApplicationStatus } from '@/services';

interface MarkAppliedButtonProps {
  applicationId: string;
  /** Current status; the action renders only while it is still `saved`. */
  status: string | undefined;
}

/** Explicit "I submitted this application" action — generating documents never records one. */
export function MarkAppliedButton({ applicationId, status }: MarkAppliedButtonProps) {
  const { t } = useTranslation();
  const setStatus = useSetApplicationStatus();
  const [failed, setFailed] = useState(false);
  const [done, setDone] = useState(false);

  // Keep a short live-region confirmation after OUR successful call: the focused
  // button unmounts once the status flips, which would otherwise be silent.
  if (done) {
    return (
      <span role="status" className="shrink-0 text-xs text-foreground/70">
        {t('applications.detail.markedApplied')}
      </span>
    );
  }
  if (status !== 'saved') return null;

  return (
    <>
      <Button
        variant="glass"
        disabled={setStatus.isPending}
        onClick={() => {
          setFailed(false);
          setStatus.mutate(
            { id: applicationId, status: 'applied' },
            { onSuccess: () => setDone(true), onError: () => setFailed(true) }
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
