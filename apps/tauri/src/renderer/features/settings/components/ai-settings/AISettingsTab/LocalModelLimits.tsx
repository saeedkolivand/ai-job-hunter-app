import { useTranslation } from '@ajh/translations';
import { Button, Switch } from '@ajh/ui';

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

// Range-input styling shared by the limit + temperature sliders below.
const SLIDER_CLASS =
  'w-full h-2 appearance-none rounded-lg bg-foreground/[0.06] [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-brand-soft [&::-webkit-slider-thumb]:cursor-pointer';

// Per-step temperature sliders revealed when "Custom temperature" is ON. Each
// step is set independently; `def` is the app's per-step default (mirrors
// generation.ts) used both to seed the toggle and as the slider's fallback value.
const TEMP_STEPS = [
  { key: 'analysis', labelKey: 'settings.ai.localLimits.tempAnalysis', def: 0.15 },
  { key: 'resume', labelKey: 'settings.ai.localLimits.tempResume', def: 0.3 },
  { key: 'cover', labelKey: 'settings.ai.localLimits.tempCover', def: 0.5 },
  { key: 'answers', labelKey: 'settings.ai.localLimits.tempAnswers', def: 0.3 },
  { key: 'referral', labelKey: 'settings.ai.localLimits.tempReferral', def: 0.4 },
] as const;

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

  // Temperature is OPTIONAL: undefined (no object) = use the app's per-task
  // defaults. The toggle's checked state is derived purely from whether the
  // per-step override object exists.
  const temperatureOn = limits?.temperature !== undefined;

  const suggestion = suggestLocalLimits({
    modelMaxContext: detectedMax,
    freeRamGb: resources.freeRamGb,
    hasGpu: resources.hasGpu,
    freeVramGb: resources.freeVramGb,
  });

  // Mirror onboarding: warn when the chosen context exceeds what memory comfortably fits.
  const mightLag = contextWindow > suggestion.contextWindow;

  return (
    <div className="mt-2 space-y-3 rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-3">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-foreground/60">
          {t('settings.ai.localLimits.title')}
        </span>
        <Button
          variant="ghost"
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
        <Button variant="ghost" onClick={() => setLocalModelLimits(selectedModel, suggestion)}>
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

      {/* Per-step temperature override (optional). OFF = app per-task defaults; ON
          reveals one slider per generation step, each capped at 1 for usable UX
          (the schema allows up to 2). Kept as the LAST element, separated by a
          divider, so the context-window "Suggested: …" hint above never reads as
          a temperature suggestion. */}
      <div className="border-t border-foreground/10 pt-3">
        <Switch
          label={t('settings.ai.localLimits.temperatureOverride')}
          checked={temperatureOn}
          onCheckedChange={(v) =>
            setLocalModelLimits(selectedModel, {
              temperature: v
                ? { analysis: 0.15, resume: 0.3, cover: 0.5, answers: 0.3, referral: 0.4 }
                : undefined,
            })
          }
        />

        {temperatureOn ? (
          <div className="mt-3 space-y-3">
            {TEMP_STEPS.map(({ key, labelKey, def }) => {
              const value = limits?.temperature?.[key] ?? def;
              return (
                <div key={key}>
                  <div className="mb-2 flex justify-between text-xs">
                    <span className="text-foreground/55">{t(labelKey)}</span>
                    <span className="text-foreground/80">{value.toFixed(2)}</span>
                  </div>
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.05}
                    value={value}
                    onChange={(e) =>
                      setLocalModelLimits(selectedModel, {
                        temperature: { ...limits?.temperature, [key]: Number(e.target.value) },
                      })
                    }
                    className={SLIDER_CLASS}
                  />
                </div>
              );
            })}
          </div>
        ) : (
          <p className="text-foreground/40 mt-1.5 text-xs">
            {t('settings.ai.localLimits.temperatureAuto')}
          </p>
        )}
      </div>
    </div>
  );
}
