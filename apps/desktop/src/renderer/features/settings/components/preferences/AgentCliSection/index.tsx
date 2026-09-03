import { Copy, Terminal } from 'lucide-react';
import { useState } from 'react';
import { useRouter } from '@tanstack/react-router';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import {
  Button,
  ErrorState,
  Input,
  SegmentedControl,
  SettingsSection,
  Skeleton,
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
 * The card's four group labels (path, tier, and the two snippets) share one
 * step so the groups read as peers. A distinct weight rather than a distinct
 * size: the card is dense, and four different type sizes inside one card is
 * what made the groups hard to tell apart from the body copy under them.
 */
const GROUP_LABEL = 'block text-xs font-semibold text-foreground/80';

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
  const { data, isPending, isError, refetch } = useAgentCliInfo();
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

  const pathLabel = t('settings.developer.agentCli.pathLabel');

  return (
    <SettingsSection icon={Terminal} label={t('settings.developer.agentCli.title')}>
      <div className="space-y-4" data-testid={TEST_IDS.settings.agentCliSection}>
        <p className="text-xs leading-snug text-foreground/70">
          {t('settings.developer.agentCli.description')}
        </p>
        {/* An in-app navigation that READS as the inline link it is. A
            padding-stripped ghost button kept button chrome (hover fill, active
            scale) at link size, which is neither. `unstyled` still routes
            through the primitive, so the focus-visible ring survives. */}
        <Button
          variant="unstyled"
          className="text-xs text-brand underline-offset-2 hover:underline"
          onClick={() => void router.navigate({ to: ROUTES.SUPPORT })}
        >
          {t('settings.developer.agentCli.helpLink')}
        </Button>

        {/* CLI path */}
        <div className="space-y-1.5">
          {/* `htmlFor` only in the branch that actually renders that input — a
              label pointing at an id that does not exist (pending, error, or
              unresolved path) is a broken reference, not a hidden one. */}
          {exePath ? (
            <label className={GROUP_LABEL} htmlFor="agent-cli-path">
              {pathLabel}
            </label>
          ) : (
            <span className={GROUP_LABEL}>{pathLabel}</span>
          )}
          {isPending ? (
            <Skeleton className="h-8 w-full rounded-lg" />
          ) : isError ? (
            <ErrorState
              className="py-6"
              title={t('settings.developer.agentCli.pathErrorTitle')}
              description={t('settings.developer.agentCli.pathErrorDesc')}
              onRetry={() => void refetch()}
            />
          ) : exePath ? (
            // `flex-wrap` + `min-w-0`: at the 900px minimum window on the
            // `large` text scale this column is ~316px, and an Input without
            // `min-w-0` refuses to shrink past its intrinsic width — with a
            // `shrink-0` button beside it the row overflowed the card.
            <div className="flex flex-wrap items-center gap-2">
              <Input
                id="agent-cli-path"
                data-testid={TEST_IDS.settings.agentCliPath}
                readOnly
                value={exePath}
                className="min-w-0 flex-1 font-mono text-xs"
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

        {/* Access tier — picks the flag the snippets below carry. Rendered only
            when there IS a path: with no path there are no snippets under it,
            so the control would rewrite nothing. */}
        {exePath ? (
          <div className="space-y-1.5 border-t border-foreground/10 pt-3">
            <span className={GROUP_LABEL}>{t('settings.developer.agentCli.tierLabel')}</span>
            <p className="text-[11px] leading-snug text-foreground/70">
              {t('settings.developer.agentCli.tierHint')}
            </p>
            {/* SegmentedControl takes only its own props, so the test id lives on
                a wrapper rather than being dropped silently. `flex-wrap` +
                `max-w-full` on the control: its segments are `whitespace-nowrap`,
                so at the narrow column above the three German labels have to
                wrap onto a second row instead of running off the card. */}
            <div data-testid={TEST_IDS.settings.agentCliTier}>
              <SegmentedControl
                options={tierOptions}
                value={tier}
                onChange={setTier}
                className="max-w-full flex-wrap"
                ariaLabel={t('settings.developer.agentCli.tierLabel')}
              />
            </div>
            <p className="text-[11px] leading-snug text-foreground/70">
              {t(`settings.developer.agentCli.tiers.${tier}.description`)}
            </p>
          </div>
        ) : null}

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
 *
 * The block WRAPS rather than scrolling. A horizontally-scrolling `<pre>` is
 * unreachable by keyboard on WebKit (no focusable scroll container) and hid the
 * privilege flag — the one word on the line that changes what the user is about
 * to grant — off the right edge at every width this card is rendered at.
 * `break-all` because the overflowing token is a filesystem path with no spaces
 * to break at.
 */
function SnippetBlock({ label, copyLabel, snippet, testId, onCopy }: SnippetBlockProps) {
  if (!snippet) return null;
  const labelId = `${testId}-label`;
  return (
    <div className="space-y-1.5 border-t border-foreground/10 pt-3">
      <div className="flex items-center justify-between gap-2">
        <span id={labelId} className={GROUP_LABEL}>
          {label}
        </span>
        <Button variant="glass" onClick={() => void onCopy(snippet)} className="shrink-0">
          <Copy size={11} />
          {copyLabel}
        </Button>
      </div>
      {/* `role="group"` is what makes `aria-labelledby` count: a bare `<pre>`
          has no role, and a name on a roleless element is not exposed — the
          block would be announced as an unlabelled run of text. */}
      <pre
        data-testid={testId}
        role="group"
        aria-labelledby={labelId}
        className="whitespace-pre-wrap break-all rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2 font-mono text-[11px] leading-relaxed text-foreground/80"
      >
        {snippet}
      </pre>
    </div>
  );
}
