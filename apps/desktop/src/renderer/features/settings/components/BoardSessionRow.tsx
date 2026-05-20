import { useState } from 'react';
import { Check, Link as LinkIcon, LogOut } from 'lucide-react';
import { ConfirmModal } from '@/components/ui/ConfirmModal';
import { cn } from '@/lib/cn';
import {
  useLinkedInStatus,
  useLinkedInConnect,
  useLinkedInDisconnect,
  useBoardStatus,
  useBoardConnect,
  useBoardDisconnect,
} from '@/services';

interface Board {
  id: string;
  name: string;
  hint: string;
  useSessionAuth: boolean;
}

const BOARD_STYLE: Record<string, { iconBg: string; glowColor: string; abbr: string }> = {
  linkedin: { iconBg: 'bg-[#0077B5]', glowColor: 'rgba(0,119,181,0.2)', abbr: 'in' },
  indeed: { iconBg: 'bg-[#003A9B]', glowColor: 'rgba(0,58,155,0.2)', abbr: 'id' },
  xing: { iconBg: 'bg-[#026466]', glowColor: 'rgba(2,100,102,0.2)', abbr: 'xi' },
  glassdoor: { iconBg: 'bg-[#0CAA41]', glowColor: 'rgba(12,170,65,0.2)', abbr: 'gd' },
};

const FALLBACK = { iconBg: 'bg-brand', glowColor: 'rgba(168,85,247,0.2)', abbr: '?' };

export function BoardSessionRow({ board }: { board: Board }) {
  const [confirmOpen, setConfirmOpen] = useState(false);

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
      if (isLinkedIn) {
        await linkedInConnect.mutateAsync();
      } else {
        await boardConnect.mutateAsync(board.id);
      }
    } catch (err) {
      console.error(`${board.name} connection failed:`, err);
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
    } catch (err) {
      console.error(`${board.name} disconnection failed:`, err);
    }
  };

  return (
    <>
      <div
        className="relative flex items-center gap-4 overflow-hidden rounded-xl border border-white/[0.07] px-4 py-3.5"
        style={{
          background:
            'linear-gradient(135deg, rgba(255,255,255,0.03) 0%, rgba(255,255,255,0.01) 100%)',
        }}
      >
        {/* Ambient glow */}
        <div
          className="pointer-events-none absolute -bottom-4 -left-4 h-24 w-24 rounded-full blur-2xl"
          style={{ background: style.glowColor }}
        />

        {/* Icon */}
        <div
          className={cn(
            'relative flex h-10 w-10 shrink-0 items-center justify-center rounded-xl text-[11px] font-bold uppercase tracking-wider text-white shadow-md',
            style.iconBg
          )}
        >
          {style.abbr}
        </div>

        {/* Info */}
        <div className="relative min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-sm font-semibold text-white/90">{board.name}</span>
            {connected && (
              <span className="flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
                <Check size={9} strokeWidth={2.5} /> Connected
              </span>
            )}
          </div>
          <div className="mt-0.5 text-xs text-white/40 leading-snug">
            {connected && accountEmail ? accountEmail : board.hint}
          </div>
        </div>

        {/* Action button */}
        {connected ? (
          <button
            onClick={() => setConfirmOpen(true)}
            disabled={loading}
            className={cn(
              'relative shrink-0 rounded-lg border px-3 py-1.5 text-xs font-medium transition-all duration-150',
              'border-red-500/40 text-red-400 hover:border-red-500/60 hover:bg-red-500/5',
              'disabled:pointer-events-none disabled:opacity-40'
            )}
          >
            {loading ? (
              <span className="flex items-center gap-1.5">
                <span className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
                Disconnecting…
              </span>
            ) : (
              <span className="flex items-center gap-1.5">
                <LogOut size={11} />
                Disconnect
              </span>
            )}
          </button>
        ) : (
          <button
            onClick={() => void handleConnect()}
            disabled={loading}
            className={cn(
              'relative shrink-0 rounded-lg border px-3 py-1.5 text-xs font-medium transition-all duration-150',
              'border-brand/40 text-brand-soft hover:border-brand/60 hover:bg-brand/5',
              'disabled:pointer-events-none disabled:opacity-40'
            )}
          >
            {loading ? (
              <span className="flex items-center gap-1.5">
                <span className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
                Connecting…
              </span>
            ) : (
              <span className="flex items-center gap-1.5">
                <LinkIcon size={11} />
                Connect
              </span>
            )}
          </button>
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
