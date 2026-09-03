import { Copy, Terminal } from 'lucide-react';
import { useState } from 'react';
import { useRouter } from '@tanstack/react-router';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import {
  Button,
  Input,
  RowSkeleton,
  SegmentedControl,
  SettingsSection,
  useNotification,
} from '@ajh/ui';

import { ROUTES } from '@/constants/routes/routes';
import {
  AGENT_CLI_TIERS,
  type AgentCliTier,
  buildClaudeCodeSnippet,
  buildCodexSnippet,
} from '@/features/settings/lib/agent-cli-snippets';
import { useAgentCliInfo } from '@/services';

/**
 * Developer → the app's own command-line / MCP surface: where the binary is,
 * and the exact commands that register it with Claude Code or Codex.
 *
 * The tier choice is LOCAL component state and is deliberately not persisted:
 * it selects which command text to show, and nothing here grants anything. The
 * grant happens when the user runs the command in their own agent.
 *
 * Nothing on this card claims anything about `PATH` membership — it cannot be
 * measured honestly from inside the running process, and the full-path
 * snippets work regardless of it.
 */
export function AgentCliSection() {
  const { t } = useTranslation();
  const router = useRouter();
  const notify = useNotification();
  const { data, isPending } = useAgentCliInfo();
  const [tier, setTier] = useState<AgentCliTier>('read');

  const exePath = data?.exePath ?? null;
  const claudeSnippet = buildClaudeCodeSnippet(exePath, tier);
  const codexSnippet = buildCodexSnippet(exePath, tier);

  const handleCopy = async (value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      notify.success({ message: t('settings.developer.agentCli.copied') });
    } catch {
      notify.error({ message: t('settings.developer.agentCli.copyFailed') });
    }
  };

  const tierOptions = AGENT_CLI_TIERS.map((value) => ({
    value,
    label: t(`settings.developer.agentCli.tiers.${value}.label`),
  }));

  return (
    <SettingsSection icon={Terminal} label={t('settings.developer.agentCli.title')}>
      <div className="space-y-4" data-testid={TEST_IDS.settings.agentCliSection}>
        <p className="text-xs leading-snug text-foreground/70">
          {t('settings.developer.agentCli.description')}
        </p>
        <Button
          variant="ghost"
          className="h-auto px-0 text-xs"
          onClick={() => void router.navigate({ to: ROUTES.SUPPORT })}
        >
          {t('settings.developer.agentCli.helpLink')}
        </Button>

        {/* CLI path */}
        <div className="space-y-1.5">
          <label className="text-xs text-foreground/70" htmlFor="agent-cli-path">
            {t('settings.developer.agentCli.pathLabel')}
          </label>
          {isPending ? (
            <RowSkeleton />
          ) : exePath ? (
            <div className="flex items-center gap-2">
              <Input
                id="agent-cli-path"
                data-testid={TEST_IDS.settings.agentCliPath}
                readOnly
                value={exePath}
                className="flex-1 font-mono text-xs"
                onFocus={(e) => e.currentTarget.select()}
              />
              <Button
                variant="glass"
                data-testid={TEST_IDS.settings.agentCliCopyPath}
                onClick={() => void handleCopy(exePath)}
                className="shrink-0"
              >
                <Copy size={11} />
                {t('settings.developer.agentCli.copyPath')}
              </Button>
            </div>
          ) : (
            <p className="text-xs leading-snug text-foreground/70">
              {t('settings.developer.agentCli.pathUnavailable')}
            </p>
          )}
        </div>

        {/* Access tier — picks the flag the snippets below carry. */}
        <div className="space-y-1.5 border-t border-foreground/10 pt-3">
          <span className="text-xs text-foreground/70">
            {t('settings.developer.agentCli.tierLabel')}
          </span>
          {/* SegmentedControl takes only its own props, so the test id lives on
              a wrapper rather than being dropped silently. */}
          <div data-testid={TEST_IDS.settings.agentCliTier}>
            <SegmentedControl
              options={tierOptions}
              value={tier}
              onChange={setTier}
              ariaLabel={t('settings.developer.agentCli.tierLabel')}
            />
          </div>
          <p className="text-[11px] leading-snug text-foreground/70">
            {t(`settings.developer.agentCli.tiers.${tier}.description`)}
          </p>
        </div>

        {/* Claude Code */}
        <SnippetBlock
          label={t('settings.developer.agentCli.claudeLabel')}
          copyLabel={t('settings.developer.agentCli.copyClaude')}
          snippet={claudeSnippet}
          testId={TEST_IDS.settings.agentCliClaudeSnippet}
          onCopy={handleCopy}
        />

        {/* Codex */}
        <SnippetBlock
          label={t('settings.developer.agentCli.codexLabel')}
          copyLabel={t('settings.developer.agentCli.copyCodex')}
          snippet={codexSnippet}
          testId={TEST_IDS.settings.agentCliCodexSnippet}
          onCopy={handleCopy}
        />
      </div>
    </SettingsSection>
  );
}

interface SnippetBlockProps {
  label: string;
  copyLabel: string;
  /** `null` when the binary path is unknown — the block renders nothing. */
  snippet: string | null;
  testId: string;
  onCopy: (value: string) => Promise<void>;
}

/**
 * One labelled, read-only command block plus its Copy button. A `<pre>` rather
 * than a TextArea: the Codex block is multi-line and must wrap exactly as
 * written, and there is nothing here to edit.
 */
function SnippetBlock({ label, copyLabel, snippet, testId, onCopy }: SnippetBlockProps) {
  if (!snippet) return null;
  return (
    <div className="space-y-1.5 border-t border-foreground/10 pt-3">
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-foreground/70">{label}</span>
        <Button variant="glass" onClick={() => void onCopy(snippet)} className="shrink-0">
          <Copy size={11} />
          {copyLabel}
        </Button>
      </div>
      <pre
        data-testid={testId}
        className="overflow-x-auto rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2 font-mono text-[11px] leading-relaxed text-foreground/80"
      >
        {snippet}
      </pre>
    </div>
  );
}
