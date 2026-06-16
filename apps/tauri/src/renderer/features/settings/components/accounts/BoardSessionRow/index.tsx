import { Check, Link as LinkIcon, LogOut } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, ConfirmModal, useNotification } from '@ajh/ui';

import {
  useBoardConnect,
  useBoardDisconnect,
  useBoardStatus,
  useLinkedInConnect,
  useLinkedInDisconnect,
  useLinkedInStatus,
} from '@/services';

interface Board {
  id: string;
  name: string;
  hint: string;
  useSessionAuth: boolean;
}

const BOARD_STYLE: Record<string, { iconBg: string; glowColor: string; abbr: string }> = {
  linkedin: { iconBg: '#0077B5', glowColor: 'rgba(0,119,181,0.2)', abbr: 'in' },
  indeed: { iconBg: '#003A9B', glowColor: 'rgba(0,58,155,0.2)', abbr: 'id' },
  xing: { iconBg: '#026466', glowColor: 'rgba(2,100,102,0.2)', abbr: 'xi' },
  glassdoor: { iconBg: '#0CAA41', glowColor: 'rgba(12,170,65,0.2)', abbr: 'gd' },
};

const FALLBACK = {
  iconBg: 'var(--color-brand)',
  glowColor: 'color-mix(in srgb, var(--color-brand) 20%, transparent)',
  abbr: '?',
};

export function BoardSessionRow({ board }: { board: Board }) {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const notify = useNotification();
  const { t } = useTranslation();

  const isLinkedIn = board.id === 'linkedin';
  const style = BOARD_STYLE[board.id] ?? FALLBACK;

  const linkedInStatus = useLinkedInStatus();
  const boardStatus = useBoardStatus(isLinkedIn ? '' : board.id);
  const linkedInConnect = useLinkedInConnect();
  const linkedInDisconnect = useLinkedInDisconnect();
  const boardConnect = useBoardConnect();
  const boardDisconnect = useBoardDisconnect();

  const statusData = isLinkedIn ? linkedInStatus.data : boardStatus.data;
  const connected = (statusData as { connected?: boolean } | undefined)?.connected ?? false;
  const accountEmail = (statusData as { accountEmail?: string } | undefined)?.accountEmail;

  const loading = isLinkedIn
    ? linkedInConnect.isPending || linkedInDisconnect.isPending
    : boardConnect.isPending || boardDisconnect.isPending;

  const handleConnect = async () => {
    try {
      const result = isLinkedIn
        ? await linkedInConnect.mutateAsync()
        : await boardConnect.mutateAsync(board.id);
      const res = result as
        | { connected?: boolean; viaImport?: boolean; error?: string }
        | undefined;
      if (res?.error) {
        notify.error({ message: res.error });
      } else if (res?.viaImport) {
        notify.success({
          message: t('settings.accounts.connectedViaBrowser', { board: board.name }),
        });
      } else if (!res?.connected) {
        notify.warning({ message: `${board.name} sign-in was cancelled or timed out.` });
      }
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : `Failed to connect to ${board.name}.`,
      });
    }
  };

  const handleDisconnect = async () => {
    setConfirmOpen(false);
    try {
      if (isLinkedIn) {
        await linkedInDisconnect.mutateAsync();
      } else {
        await boardDisconnect.mutateAsync(board.id);
      }
      notify.success({ message: `Disconnected from ${board.name}.` });
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : `Failed to disconnect from ${board.name}.`,
      });
    }
  };

  return (
    <>
      <div className="relative flex items-center gap-4 overflow-hidden rounded-xl border border-foreground/10 px-4 py-3.5">
        {/* Ambient glow */}
        <div
          className="pointer-events-none absolute -bottom-4 -left-4 h-24 w-24 rounded-full blur-2xl"
          style={{ background: style.glowColor }}
        />

        {/* Icon */}
        <div
          className="relative flex h-10 w-10 shrink-0 items-center justify-center rounded-xl text-[11px] font-bold uppercase tracking-wider text-white shadow-md"
          style={{ backgroundColor: style.iconBg }}
        >
          {style.abbr}
        </div>

        {/* Info */}
        <div className="relative min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-sm font-semibold text-foreground/90">{board.name}</span>
            {connected && (
              <span className="flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
                <Check size={9} strokeWidth={2.5} /> Connected
              </span>
            )}
          </div>
          <div className="mt-0.5 text-xs text-foreground/40 leading-snug">
            {connected && accountEmail ? accountEmail : board.hint}
          </div>
        </div>

        {/* Action button */}
        {connected ? (
          <Button
            variant="danger"
            onClick={() => setConfirmOpen(true)}
            disabled={loading}
            className="relative shrink-0"
          >
            {loading ? (
              <>
                <span className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
                Disconnecting…
              </>
            ) : (
              <>
                <LogOut size={11} />
                Disconnect
              </>
            )}
          </Button>
        ) : (
          <Button
            variant="info"
            onClick={() => void handleConnect()}
            disabled={loading}
            className="relative shrink-0"
          >
            {loading ? (
              <>
                <span className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
                Connecting…
              </>
            ) : (
              <>
                <LinkIcon size={11} />
                Connect
              </>
            )}
          </Button>
        )}
      </div>

      <ConfirmModal
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        onConfirm={() => void handleDisconnect()}
        title={`Disconnect ${board.name}`}
        description={`This will sign you out of ${board.name}. You can reconnect at any time.`}
        confirmText="Disconnect"
        variant="info"
        isConfirming={loading}
      />
    </>
  );
}
