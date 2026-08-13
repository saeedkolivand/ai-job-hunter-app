import { Terminal } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';

import { StageSuggestionBanner } from '../StageOverridesSettings/StageSuggestionBanner';
import { suggestExtractionModel } from '../StageOverridesSettings/suggest-stage-models';
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
  /** Applied or declined — either way the offer is spent for this visit. */
  const [declined, setDeclined] = useState(false);

  // Same pure predicate the banner uses, on the same inputs — so the section
  // header and its body can never disagree about whether advice exists.
  const hasSuggestion =
    suggestExtractionModel({
      activeProvider: ctx.activeProvider,
      activeModel: ctx.activeModel,
      installedModels: ctx.installedModels,
      inspections: ctx.inspections,
      overrides: ctx.overrides,
    }) !== null;

  return (
    <div className="space-y-4">
      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/60">
          {t('settings.ai.advisor.recommend.stagesHeading')}
        </h3>
        {hasSuggestion && !declined ? (
          <StageSuggestionBanner
            activeProvider={ctx.activeProvider}
            activeModel={ctx.activeModel}
            installedModels={ctx.installedModels}
            overrides={ctx.overrides}
            // Both paths unmount the button that was just pressed. Inside a
            // focus-trapped dialog that is worse than untidy: focus lands on
            // <body>, and the trap's keydown listener is bound to the panel, so
            // the next Tab goes to the page BEHIND the overlay.
            onApplied={() => {
              setDeclined(true);
              ctx.focusPrimaryAction?.();
            }}
            onDismiss={() => {
              setDeclined(true);
              ctx.focusPrimaryAction?.();
            }}
          />
        ) : (
          // An empty section reads as "the advisor has nothing to say and won't
          // admit it" — say the actual finding instead. Declining counts: the
          // banner unmounts, and this section must not go blank with it.
          <p className="text-xs leading-relaxed text-foreground/60">
            {t(
              declined
                ? 'settings.ai.advisor.recommend.stagesDeclined'
                : 'settings.ai.advisor.recommend.stagesNothing'
            )}
          </p>
        )}
        <p className="text-xs leading-relaxed text-foreground/50">
          {t('settings.ai.advisor.recommend.stagesBody')}
        </p>
      </section>

      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/60">
          {t('settings.ai.advisor.recommend.appHeading')}
        </h3>
        <p className="text-xs leading-relaxed text-foreground/50">
          {t('settings.ai.advisor.recommend.appBody')}
        </p>
      </section>

      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/60">
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
              <Terminal size={12} className="shrink-0 text-foreground/60" aria-hidden="true" />
              <code className="font-mono text-[11px] text-foreground/70">{line}</code>
            </li>
          ))}
        </ul>
        <p className="text-[11px] leading-relaxed text-foreground/60">
          {t('settings.ai.advisor.recommend.serverDisclaimer')}
        </p>
      </section>
    </div>
  );
}
