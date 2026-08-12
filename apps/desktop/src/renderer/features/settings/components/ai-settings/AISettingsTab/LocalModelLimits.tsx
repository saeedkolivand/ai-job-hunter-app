import { useRef } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, Switch, useNotification } from '@ajh/ui';

import { useInspectModel, useSaveProviderSettings, useSystemResources } from '@/services';
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
// step is set independently; `def` seeds the toggle and is the slider's
// fallback value. These do NOT mirror `generation.ts` any more — the
// renderer no longer ships per-step numbers at all (`generation.ts` now
// sends a `deterministic`/`prose`/`prose_grounded` INTENT; each provider
// adapter's own `sampling_profile` — `commands/ai_provider/mod.rs` — picks
// the numbers). These seeds instead mirror that Rust module's
// `DETERMINISTIC_TEMPERATURE` (0.3) / `PROSE_TEMPERATURE` (0.5) /
// `PROSE_GROUNDED_TEMPERATURE` (0.6) constants. Each key maps to exactly ONE
// intent/surface group (see `TemperatureStep`'s doc comment): analysis
// (metadata extraction, job-ad summaries) and résumé (résumé generation,
// imported GitHub projects) are `deterministic`; cover letter, application
// answers, and referral are `prose_grounded`; questions (interview
// questions, likely questions, STAR feedback) is `prose`.
const TEMP_STEPS = [
  { key: 'analysis', labelKey: 'settings.ai.localLimits.tempAnalysis', def: 0.3 },
  { key: 'resume', labelKey: 'settings.ai.localLimits.tempResume', def: 0.3 },
  { key: 'cover', labelKey: 'settings.ai.localLimits.tempCover', def: 0.6 },
  { key: 'answers', labelKey: 'settings.ai.localLimits.tempAnswers', def: 0.6 },
  { key: 'questions', labelKey: 'settings.ai.localLimits.tempQuestions', def: 0.5 },
  { key: 'referral', labelKey: 'settings.ai.localLimits.tempReferral', def: 0.6 },
] as const;

/**
 * Per-local-model generation limits: an "Analyze model" action that reads the
 * model's real context window via `/api/show`, sliders for the context window
 * (num_ctx) + max output (num_predict) persisted per model, a "Use suggested"
 * button driven by hardware, and a hardware-lag warning mirroring onboarding.
 */
export function LocalModelLimits({ selectedModel }: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const inspect = useInspectModel();
  const { resources } = useSystemResources(selectedModel);
  const setLocalModelLimits = usePreferencesStore((s) => s.setLocalModelLimits);
  const { save: saveProviderSettings } = useSaveProviderSettings();
  /** Last value actually sent, so a release that moved nothing writes nothing. */
  const committedWindow = useRef<number | undefined>(undefined);
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

  /**
   * Push the window to the BACKEND row for this model.
   *
   * The slider only ever wrote renderer preferences, which the fast path reads
   * and a staged run cannot — so at quality/max depth it silently did nothing
   * (`context_window: None` at all three request sites). Committed on release
   * rather than on every `onChange` tick, so a drag is one write, not eighty;
   * `onKeyUp` covers the arrow-key path so keyboard users commit too.
   *
   * A rejected write is surfaced: the slider would otherwise keep showing a
   * value the backend never accepted, which is the same silent divergence this
   * whole commit path exists to remove.
   */
  const commitWindow = (value: number) => {
    // Key-up and pointer-up both fire on a press that moved nothing.
    if (value === committedWindow.current) return;
    committedWindow.current = value;
    saveProviderSettings(
      { provider: 'ollama', model: selectedModel, contextWindow: value },
      {
        onError: (err) =>
          notify.error({
            message: t('settings.ai.localLimits.windowSaveFailed', { reason: err.message }),
          }),
      }
    );
  };

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
          onPointerUp={(e) => commitWindow(Number(e.currentTarget.value))}
          onKeyUp={(e) => commitWindow(Number(e.currentTarget.value))}
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
          onClick={() => {
            setLocalModelLimits(selectedModel, suggestion);
            commitWindow(suggestion.contextWindow);
          }}
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
                ? Object.fromEntries(TEMP_STEPS.map(({ key, def }) => [key, def]))
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
