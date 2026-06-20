import { Loader2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import {
  useBoardConnect,
  useBoardDisconnect,
  useBoardStatus,
  useLinkedInConnect,
  useLinkedInDisconnect,
  useLinkedInStatus,
} from '@/services';

interface BoardConnectChipProps {
  board: string;
}

/**
 * Self-contained chip for a single board that needs login. Internally owns
 * the status + connect hooks for that board, sidestepping the hooks-in-a-loop
 * rule — callers render one chip instance per board.
 *
 * LinkedIn is special-cased to use its dedicated hooks.
 */
export function BoardConnectChip({ board }: BoardConnectChipProps) {
  const { t } = useTranslation();
  const isLinkedIn = board === 'linkedin';

  // LinkedIn-specific hooks — always called (hook rule), but only used when relevant.
  const linkedInStatus = useLinkedInStatus();
  const linkedInConnect = useLinkedInConnect();
  const linkedInDisconnect = useLinkedInDisconnect();

  // Generic board hooks — `useBoardStatus` is disabled when boardId is empty.
  const boardStatus = useBoardStatus(isLinkedIn ? '' : board);
  const boardConnect = useBoardConnect();
  const boardDisconnect = useBoardDisconnect();

  const connected = isLinkedIn
    ? ((linkedInStatus.data as { connected?: boolean } | undefined)?.connected ?? false)
    : ((boardStatus.data as { connected?: boolean } | undefined)?.connected ?? false);

  const connectPending = isLinkedIn ? linkedInConnect.isPending : boardConnect.isPending;
  const disconnectPending = isLinkedIn ? linkedInDisconnect.isPending : boardDisconnect.isPending;

  const handleConnect = async () => {
    if (isLinkedIn) {
      await linkedInConnect.mutateAsync();
    } else {
      await boardConnect.mutateAsync(board);
    }
  };

  const handleDisconnect = async () => {
    if (isLinkedIn) {
      await linkedInDisconnect.mutateAsync();
    } else {
      await boardDisconnect.mutateAsync(board);
    }
  };

  const boardLabel = t(`jobs.boards.${board}`);

  if (connected) {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
        <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
        {boardLabel}
        <Button
          variant="unstyled"
          type="button"
          disabled={disconnectPending}
          aria-label={`${t('jobs.disconnect')} ${boardLabel}`}
          onClick={() => void handleDisconnect()}
          className="ml-0.5 text-[10px] text-red-400/70 underline-offset-2 hover:text-red-400 hover:underline disabled:opacity-50"
        >
          {disconnectPending ? (
            <Loader2 size={10} className="animate-spin" />
          ) : (
            t('jobs.disconnect')
          )}
        </Button>
      </span>
    );
  }

  return (
    <Button
      variant="unstyled"
      type="button"
      disabled={connectPending}
      onClick={() => void handleConnect()}
      className="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2 py-0.5 text-[10px] font-medium text-amber-400 underline-offset-2 hover:underline disabled:opacity-50"
      aria-label={`${t('jobs.needsLogin.connectBoard')} ${boardLabel}`}
    >
      {connectPending ? (
        <>
          <Loader2 size={10} className="animate-spin" />
          <span className="sr-only">{boardLabel}</span>
        </>
      ) : (
        <>
          <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
          {boardLabel}
        </>
      )}
    </Button>
  );
}
