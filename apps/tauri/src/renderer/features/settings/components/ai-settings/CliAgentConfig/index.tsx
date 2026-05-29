import { ExternalLink, Loader2 } from 'lucide-react';
import { useEffect, useState } from 'react';

import { Button, Input } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface Props {
  /** Human label of the agent, e.g. "Claude Code" — used in the install hint. */
  label: string;
  /** Whether the agent's CLI binary was detected. */
  connected: boolean;
  /** Suggested model aliases (e.g. sonnet / opus / haiku) — quick-picks only. */
  models: string[];
  providerModel: string;
  onSelect: (model: string) => void;
  onSetActive: () => void;
  isActive: boolean;
  /** Open the agent's install/setup docs. */
  onInstall: () => void;
  /** Re-probe detection (re-runs the system health check). */
  onRecheck: () => void;
}

/**
 * Config UI for a `cli-agent` provider: a locally-installed headless tool with no
 * API key. The model is a **free-text** field (the CLI exposes no model list to
 * query) with the known aliases as quick-picks — so any model the CLI supports can
 * be used without a codebase change, while routine version bumps ride the aliases.
 */
export function CliAgentConfig({
  label,
  connected,
  models,
  providerModel,
  onSelect,
  onSetActive,
  isActive,
  onInstall,
  onRecheck,
}: Props) {
  const { t } = useTranslation();
  // Local draft so we persist on commit (blur / Enter / quick-pick), not per keystroke.
  const [draft, setDraft] = useState(providerModel);
  useEffect(() => setDraft(providerModel), [providerModel]);

  const commit = () => {
    const next = draft.trim();
    if (next !== providerModel) onSelect(next);
  };

  const pick = (model: string) => {
    setDraft(model);
    onSelect(model);
  };

  return (
    <>
      {!connected && (
        <div className="space-y-2">
          <p className="text-sm text-foreground/50">
            {label} CLI not detected. Install it and sign in once, then recheck.
          </p>
          <div className="flex gap-2">
            <Button variant="ghost" size="sm" className="text-foreground/50" onClick={onInstall}>
              <ExternalLink size={11} /> Install {label}
            </Button>
            <Button variant="ghost" size="sm" className="text-foreground/40" onClick={onRecheck}>
              <Loader2 size={11} /> Recheck
            </Button>
          </div>
        </div>
      )}

      {connected && (
        <>
          <div className="space-y-1.5">
            <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
              {t('settings.aiModel.title')}
            </div>
            <Input
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={commit}
              onKeyDown={(e) => e.key === 'Enter' && commit()}
              placeholder="sonnet, opus, haiku, or any model your CLI supports…"
              className="w-full text-sm"
            />
            {models.length > 0 && (
              <div className="flex flex-wrap gap-1.5 pt-0.5">
                {models.map((m) => (
                  <button
                    key={m}
                    onClick={() => pick(m)}
                    className={`rounded-md border px-2 py-0.5 text-[11px] transition-colors ${
                      providerModel === m
                        ? 'border-brand/40 bg-brand/10 text-brand-soft'
                        : 'border-white/[0.08] bg-white/[0.02] text-foreground/50 hover:text-foreground/80'
                    }`}
                  >
                    {m}
                  </button>
                ))}
              </div>
            )}
          </div>
          <Button
            variant="glass"
            size="sm"
            onClick={onSetActive}
            disabled={isActive}
            className={isActive ? 'opacity-40' : 'ring-1 ring-brand/20'}
          >
            {isActive ? 'Currently active' : 'Set as active'}
          </Button>
        </>
      )}
    </>
  );
}
