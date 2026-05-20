import { motion } from 'motion/react';
import { Link as LinkIcon, LogOut, Check, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { useLinkedInStatus, useLinkedInConnect, useLinkedInDisconnect } from '@/services';

interface LinkedInSessionRowProps {
  board: { id: string; name: string; hint: string; useSessionAuth: boolean };
}

export function LinkedInSessionRow({ board }: LinkedInSessionRowProps) {
  const { data: status } = useLinkedInStatus();
  const linkedInConnect = useLinkedInConnect();
  const linkedInDisconnect = useLinkedInDisconnect();
  const loading = linkedInConnect.isPending || linkedInDisconnect.isPending;

  const handleConnect = async () => {
    try {
      await linkedInConnect.mutateAsync();
    } catch (err) {
      console.error('LinkedIn connection failed:', err);
    }
  };

  const handleDisconnect = async () => {
    try {
      await linkedInDisconnect.mutateAsync();
    } catch (err) {
      console.error('LinkedIn disconnection failed:', err);
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 5 }}
      animate={{ opacity: 1, y: 0 }}
      className="rounded-xl border border-white/10 bg-white/5 p-4"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-medium text-foreground">{board.name}</h3>
            {(status as { connected?: boolean } | undefined)?.connected && (
              <div className="flex items-center gap-1 rounded-full bg-green-500/20 px-2 py-0.5 text-xs text-green-400">
                <Check size={10} />
                Connected
              </div>
            )}
          </div>
          <p className="mt-1 text-xs text-foreground/55">{board.hint}</p>
          {(status as { connected?: boolean; accountEmail?: string } | undefined)?.connected &&
            (status as { accountEmail?: string } | undefined)?.accountEmail && (
              <p className="mt-1 text-xs text-foreground/40">
                Account: {(status as { accountEmail?: string }).accountEmail}
              </p>
            )}
        </div>
        <div className="flex items-center gap-2">
          {loading ? (
            <Button size="sm" variant="glass" disabled>
              <Loader2 size={14} className="animate-spin" />
            </Button>
          ) : (status as { connected?: boolean } | undefined)?.connected ? (
            <Button size="sm" variant="glass" onClick={handleDisconnect} className="hover:glow-red">
              <LogOut size={14} />
              Disconnect
            </Button>
          ) : (
            <Button size="sm" variant="glass" onClick={handleConnect} className="hover:glow-purple">
              <LinkIcon size={14} />
              Connect
            </Button>
          )}
        </div>
      </div>
    </motion.div>
  );
}
