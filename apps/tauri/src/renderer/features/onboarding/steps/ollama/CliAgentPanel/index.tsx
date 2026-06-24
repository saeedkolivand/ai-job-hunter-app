import { Bot, CheckCircle2, ExternalLink, Terminal, WifiOff } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { Alert, Button, transition } from '@ajh/ui';

import { useOpenExternal, useSystemHealth } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';

interface CliAgent {
  id: AiProvider;
  label: string;
  docsUrl: string;
  color: string;
}

// Mirrors the cli-agent entries in the settings provider-meta. Kept local because
// onboarding must not import across feature directories (same as CloudProviderPanel).
const CLI_AGENTS: CliAgent[] = [
  {
    id: 'claude-code',
    label: 'Claude Code',
    docsUrl: 'https://docs.anthropic.com/en/docs/claude-code',
    color: 'text-orange-400',
  },
  {
    id: 'codex',
    label: 'OpenAI Codex',
    docsUrl: 'https://developers.openai.com/codex/cli',
    color: 'text-green-400',
  },
  {
    id: 'gemini-cli',
    label: 'Gemini CLI',
    docsUrl: 'https://github.com/google-gemini/gemini-cli',
    color: 'text-blue-400',
  },
];

interface CliAgentPanelProps {
  selectedProvider: AiProvider;
  onProviderChange: (provider: AiProvider) => void;
}

/**
 * Onboarding panel for CLI agents: pick the agent and confirm it's detected. Model
 * selection is intentionally deferred to Settings → AI after onboarding.
 */
export function CliAgentPanel({ selectedProvider, onProviderChange }: CliAgentPanelProps) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();
  const { data: health } = useSystemHealth();
  const cliAgents = health?.cliAgents ?? {};

  const meta = CLI_AGENTS.find((a) => a.id === selectedProvider) ?? CLI_AGENTS[0];
  const label = meta?.label ?? '';
  const docsUrl = meta?.docsUrl ?? '';
  const detected = cliAgents[selectedProvider]?.detected ?? false;

  return (
    <motion.div
      key="cli-panel"
      initial={{ opacity: 0, y: 20, scale: 0.95 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -20, scale: 0.95 }}
      transition={transition.normal}
      className="mb-6 space-y-4"
    >
      <p className="flex items-center gap-1.5 text-xs text-foreground/40">
        <Terminal size={12} /> Run a coding agent you already have installed — uses its own login,
        no API key. You'll pick a model later in Settings.
      </p>

      {/* Agent selector */}
      <div className="space-y-2">
        {CLI_AGENTS.map((a) => {
          const isSelected = selectedProvider === a.id;
          const isDetected = cliAgents[a.id]?.detected ?? false;
          return (
            <Button
              key={a.id}
              variant="unstyled"
              onClick={() => onProviderChange(a.id)}
              className={`flex w-full items-center gap-3 rounded-xl border px-4 py-2.5 text-left transition-all duration-150 ${
                isSelected
                  ? 'border-brand/40 bg-brand/10'
                  : 'border-[var(--border-clear)] bg-card hover:bg-muted'
              }`}
            >
              <Bot size={14} className={isSelected ? a.color : 'text-foreground/30'} />
              <span
                className={`text-sm font-medium ${
                  isSelected ? 'text-foreground/90' : 'text-foreground/60'
                }`}
              >
                {a.label}
              </span>
              <span className="ml-auto flex items-center gap-1 text-[10px]">
                {isDetected ? (
                  <span className="flex items-center gap-1 text-emerald-400/80">
                    <CheckCircle2 size={11} /> Detected
                  </span>
                ) : (
                  <span className="flex items-center gap-1 text-amber-400/60">
                    <WifiOff size={11} /> Not detected
                  </span>
                )}
              </span>
            </Button>
          );
        })}
      </div>

      {/* Install hint when the selected agent isn't found */}
      {!detected && (
        <Alert
          type="warning"
          showIcon
          message={t('onboarding.ai.cliNotDetected', { agent: label })}
          action={
            <Button
              variant="glass"
              onClick={() => void openExternal.mutateAsync(docsUrl)}
              className="h-auto px-2 py-1 text-xs"
            >
              <ExternalLink size={11} className="mr-1" /> Install {label}
            </Button>
          }
        />
      )}
    </motion.div>
  );
}
