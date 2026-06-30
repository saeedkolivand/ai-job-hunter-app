import { Download, ExternalLink, Loader2, RotateCcw } from 'lucide-react';
import { useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, ConfirmModal, useNotification } from '@ajh/ui';

import { useCliAgents, useInstallCliAgent } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';

interface Props {
  /** The CLI-agent provider id (`claude-code` | `codex` | `gemini-cli`). */
  provider: AiProvider;
  label: string;
  /** Open the agent's official install/setup docs (the guide path). */
  onGuide: () => void;
  /** Re-probe provider health so a freshly-installed agent flips to connected. */
  onRecheck: () => void;
}

/**
 * The "not detected" block for a CLI agent (#22): a consent-gated one-click
 * `npm install -g <pkg>` (only when npm is available), with the guide + re-check
 * always available. The install runs the capability-allowlisted command through
 * the shell plugin and streams its output here.
 */
export function CliAgentInstall({ provider, label, onGuide, onRecheck }: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const { data: status } = useCliAgents();
  const install = useInstallCliAgent();

  const agent = status?.agents.find((a) => a.id === provider);
  const npmAvailable = status?.npmAvailable ?? false;
  const canOneClick = Boolean(agent) && npmAvailable && !install.isPending;

  const [confirmOpen, setConfirmOpen] = useState(false);
  const [output, setOutput] = useState('');
  const abortRef = useRef<AbortController | null>(null);

  // The literal command, shown in the consent dialog (no interpolation at runtime
  // — args are the fixed, capability-allowlisted values).
  const command = agent ? `npm ${agent.installArgs.join(' ')}` : '';

  const runInstall = async () => {
    if (!agent) return;
    setConfirmOpen(false);
    setOutput('');
    const controller = new AbortController();
    abortRef.current = controller;
    try {
      const result = await install.mutateAsync({
        commandName: agent.installCommandName,
        args: agent.installArgs,
        signal: controller.signal,
        onOutput: (line) =>
          // Keep the tail only — installers can be chatty.
          setOutput((prev) => `${prev}${line}\n`.split('\n').slice(-40).join('\n')),
      });
      if (result.success) {
        notify.success({ message: t('settings.cliInstall.done', { label }) });
        onRecheck();
      } else {
        notify.error({ message: t('settings.cliInstall.failed', { label }) });
      }
    } catch {
      notify.error({ message: t('settings.cliInstall.failed', { label }) });
    } finally {
      abortRef.current = null;
    }
  };

  return (
    <div className="space-y-2">
      <p className="text-sm text-foreground/50">
        {t('settings.cliInstall.notDetected', { label })}
      </p>

      {!npmAvailable && (
        <p className="text-xs text-amber-200/70">{t('settings.cliInstall.npmMissing')}</p>
      )}

      <div className="flex flex-wrap gap-2">
        {canOneClick && (
          <Button variant="glass" onClick={() => setConfirmOpen(true)} className="gap-1.5">
            <Download size={11} /> {t('settings.cliInstall.install', { label })}
          </Button>
        )}
        {install.isPending && (
          <span className="flex items-center gap-1.5 text-xs text-foreground/50">
            <Loader2 size={12} className="animate-spin" /> {t('settings.cliInstall.installing')}
          </span>
        )}
        <Button variant="ghost" className="text-foreground/50 gap-1.5" onClick={onGuide}>
          <ExternalLink size={11} /> {t('settings.cliInstall.guide')}
        </Button>
        <Button variant="ghost" className="text-foreground/40 gap-1.5" onClick={onRecheck}>
          <RotateCcw size={11} /> {t('settings.cliInstall.recheck')}
        </Button>
      </div>

      {output && (
        <pre className="max-h-40 overflow-auto rounded-lg border border-foreground/10 bg-foreground/[0.06] p-2.5 text-[10px] leading-relaxed text-foreground/55 whitespace-pre-wrap">
          {output}
        </pre>
      )}

      <ConfirmModal
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        onConfirm={() => void runInstall()}
        title={t('settings.cliInstall.confirmTitle', { label })}
        description={t('settings.cliInstall.confirmBody', { command })}
        confirmText={t('settings.cliInstall.confirm')}
        variant="warning"
        isConfirming={install.isPending}
      />
    </div>
  );
}
