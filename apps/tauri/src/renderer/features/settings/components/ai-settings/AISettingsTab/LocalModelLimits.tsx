import { Button } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useInspectModel, useSystemResources } from '@/services';
import { usePreferencesStore } from '@/store/preferences-store';

import { suggestLocalLimits } from './suggest-local-limits';

interface Props {
  selectedModel?: string;
}

const CTX_MIN = 2048;
const CTX_MAX = 131072;
const OUT_MIN = 512;
const OUT_MAX = 8192;

// Range-input styling shared by the two limit sliders below.
const SLIDER_CLASS =
  'w-full h-2 appearance-none rounded-lg bg-white/5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-brand-soft [&::-webkit-slider-thumb]:cursor-pointer';

/**
 * Per-local-model generation limits: an "Analyze model" action that reads the
 * model's real context window via `/api/show`, sliders for the context window
 * (num_ctx) + max output (num_predict) persisted per model, a "Use suggested"
 * button driven by hardware, and a hardware-lag warning mirroring onboarding.
 */
export function LocalModelLimits({ selectedModel }: Props) {
  const { t } = useTranslation();
  const inspect = useInspectModel();
  const { resources } = useSystemResources(selectedModel);
  const setLocalModelLimits = usePreferencesStore((s) => s.setLocalModelLimits);
  const limits = usePreferencesStore((s) =>
    selectedModel ? s.aiProviderConfig?.providers?.ollama?.modelLimits?.[selectedModel] : undefined
  );

  if (!selectedModel) return null;

  const inspected = inspect.data;
  const detectedMax = inspected?.contextLength;
  const ctxMax = Math.min(CTX_MAX, detectedMax ?? CTX_MAX);

  const contextWindow = Math.min(limits?.contextWindow ?? Math.min(8192, ctxMax), ctxMax);
  const maxTokens = limits?.maxTokens ?? 2048;

  const suggestion = suggestLocalLimits({
    modelMaxContext: detectedMax,
    freeRamGb: resources.freeRamGb,
    hasGpu: resources.hasGpu,
    freeVramGb: resources.freeVramGb,
  });

  // Mirror onboarding: warn when the chosen context exceeds what memory comfortably fits.
  const mightLag = contextWindow > suggestion.contextWindow;

  return (
    <div className="mt-2 space-y-3 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-3">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-foreground/60">
          {t('settings.ai.localLimits.title')}
        </span>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => inspect.mutate({ model: selectedModel })}
          disabled={inspect.isPending}
        >
          {inspect.isPending
            ? t('settings.ai.localLimits.analyzing')
            : t('settings.ai.localLimits.analyze')}
        </Button>
      </div>

      {inspected && (
        <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-foreground/45">
          {detectedMax != null && (
            <span>
              {t('settings.ai.localLimits.maxContext')}: {detectedMax.toLocaleString()}
            </span>
          )}
          {inspected.parameterSize && <span>{inspected.parameterSize}</span>}
          {inspected.quantization && <span>{inspected.quantization}</span>}
        </div>
      )}
      {inspect.isSuccess && !inspected && (
        <p className="text-xs text-foreground/40">{t('settings.ai.localLimits.noInfo')}</p>
      )}

      {/* Context window (num_ctx) */}
      <div>
        <div className="mb-2 flex justify-between text-xs">
          <span className="text-foreground/55">{t('settings.ai.localLimits.contextWindow')}</span>
          <span className="text-foreground/80">{contextWindow.toLocaleString()}</span>
        </div>
        <input
          type="range"
          min={CTX_MIN}
          max={ctxMax}
          step={512}
          value={contextWindow}
          onChange={(e) =>
            setLocalModelLimits(selectedModel, { contextWindow: Number(e.target.value) })
          }
          className={SLIDER_CLASS}
        />
      </div>

      {/* Max output (num_predict) */}
      <div>
        <div className="mb-2 flex justify-between text-xs">
          <span className="text-foreground/55">{t('settings.ai.localLimits.maxOutput')}</span>
          <span className="text-foreground/80">{maxTokens.toLocaleString()}</span>
        </div>
        <input
          type="range"
          min={OUT_MIN}
          max={OUT_MAX}
          step={256}
          value={maxTokens}
          onChange={(e) =>
            setLocalModelLimits(selectedModel, { maxTokens: Number(e.target.value) })
          }
          className={SLIDER_CLASS}
        />
      </div>

      <div className="flex items-center justify-between">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setLocalModelLimits(selectedModel, suggestion)}
        >
          {t('settings.ai.localLimits.useSuggested')}
        </Button>
        <span className="text-xs text-foreground/35">
          {t('settings.ai.localLimits.suggested')}: {suggestion.contextWindow.toLocaleString()}
        </span>
      </div>

      {mightLag && (
        <p className="text-xs text-amber-400/80">
          ⚠️{' '}
          {resources.hasGpu
            ? t('settings.ai.localLimits.mayLagVram')
            : t('settings.ai.localLimits.mayLagRam')}
        </p>
      )}
    </div>
  );
}
