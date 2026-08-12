import { Terminal } from 'lucide-react';

import { useTranslation } from '@ajh/translations';

import { StageSuggestionBanner } from '../StageOverridesSettings/StageSuggestionBanner';
import type { AdvisorStepProps } from './types';

/** Server-level settings the app must NOT write. Shown as commands to run, with
 *  the reason, because they live in the user's own Ollama service config. */
const SERVER_ADVICE = ['OLLAMA_FLASH_ATTENTION=1', 'OLLAMA_KV_CACHE_TYPE=q8_0'] as const;

/**
 * Step 3 — what to change, split by WHO changes it.
 *
 * 1. Per-stage overrides — the app's own setting, applied by the same
 *    `setStageOverride` call the Settings section uses (one accept path, not a
 *    second one that could drift).
 * 2. What the app already sends per request (`num_ctx`, `keep_alive`) — stated
 *    so nobody goes looking for a setting that is already handled.
 * 3. Server-level advice — ADVICE ONLY. The app never edits the user's Ollama
 *    service configuration, so these are printed to run yourself.
 */
export function RecommendationsStep({ ctx }: AdvisorStepProps) {
  const { t } = useTranslation();

  return (
    <div className="space-y-4">
      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('settings.ai.advisor.recommend.stagesHeading')}
        </h3>
        <StageSuggestionBanner
          activeProvider={ctx.activeProvider}
          activeModel={ctx.activeModel}
          installedModels={ctx.installedModels}
          overrides={ctx.overrides}
        />
        <p className="text-xs leading-relaxed text-foreground/50">
          {t('settings.ai.advisor.recommend.stagesBody')}
        </p>
      </section>

      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('settings.ai.advisor.recommend.appHeading')}
        </h3>
        <p className="text-xs leading-relaxed text-foreground/50">
          {t('settings.ai.advisor.recommend.appBody')}
        </p>
      </section>

      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('settings.ai.advisor.recommend.serverHeading')}
        </h3>
        <p className="text-xs leading-relaxed text-foreground/50">
          {t('settings.ai.advisor.recommend.serverBody')}
        </p>
        <ul className="space-y-1.5">
          {SERVER_ADVICE.map((line) => (
            <li
              key={line}
              className="flex items-center gap-2 rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2"
            >
              <Terminal size={12} className="shrink-0 text-foreground/40" aria-hidden="true" />
              <code className="font-mono text-[11px] text-foreground/70">{line}</code>
            </li>
          ))}
        </ul>
        <p className="text-[11px] leading-relaxed text-foreground/40">
          {t('settings.ai.advisor.recommend.serverDisclaimer')}
        </p>
      </section>
    </div>
  );
}
