import { AlertTriangle, Check, Gauge, type LucideIcon, SlidersHorizontal, Zap } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, Dropdown } from '@ajh/ui';

import { isOllamaFamily } from '@/lib/ai-providers/provider-meta';
import {
  EMPHASIS_OPTIONS,
  type EmphasisId,
  type GenerationMode,
  LETTER_MARKET_IDS,
  letterConventions,
  MODES,
} from '@/lib/generate';
import { useHasProviderKey } from '@/services';
import type { PromptQuality } from '@/store/preferences-schema';
import {
  useAiProviderConfig,
  usePreferencesStore,
  usePromptQuality,
} from '@/store/preferences-store';

interface StepFineTuneProps {
  mode: GenerationMode;
  emphasis: EmphasisId[];
  target: 'resume' | 'cover' | 'both';
  locale: string;
  researchCompany: boolean;
  onModeChange: (mode: GenerationMode) => void;
  onEmphasisChange: (ids: EmphasisId[]) => void;
  onLocaleChange: (locale: string) => void;
  onResearchCompanyChange: (v: boolean) => void;
}

const QUALITY_OPTIONS: {
  id: PromptQuality;
  labelKey: string;
  descKey: string;
  icon: LucideIcon;
}[] = [
  {
    id: 'full',
    labelKey: 'aiGenerate.wizard.quality.full',
    descKey: 'aiGenerate.wizard.quality.fullDesc',
    icon: SlidersHorizontal,
  },
  {
    id: 'auto',
    labelKey: 'aiGenerate.wizard.quality.auto',
    descKey: 'aiGenerate.wizard.quality.autoDesc',
    icon: Gauge,
  },
  {
    id: 'compact',
    labelKey: 'aiGenerate.wizard.quality.fast',
    descKey: 'aiGenerate.wizard.quality.fastDesc',
    icon: Zap,
  },
];

export function StepFineTune({
  mode,
  emphasis,
  target,
  locale,
  researchCompany,
  onModeChange,
  onEmphasisChange,
  onLocaleChange,
  onResearchCompanyChange,
}: StepFineTuneProps) {
  const { t } = useTranslation();

  // #15 — emphasis directives are multi-select (toggle in/out of the set).
  const toggleEmphasis = (id: EmphasisId) =>
    onEmphasisChange(emphasis.includes(id) ? emphasis.filter((e) => e !== id) : [...emphasis, id]);
  const promptQuality = usePromptQuality();
  const setPromptQuality = usePreferencesStore((s) => s.setPromptQuality);

  const providerConfig = useAiProviderConfig();
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const { data: ollamaKey } = useHasProviderKey('ollama-cloud');
  const showOllamaResearchHint = isOllamaFamily(activeProvider) && !(ollamaKey?.has ?? false);

  const showCoverOptions = target === 'cover' || target === 'both';

  const marketOptions = [
    { value: '', label: t('aiGenerate.market.auto') },
    ...LETTER_MARKET_IDS.filter((id) => id !== 'intl').map((id) => ({
      value: id,
      label: letterConventions(id).country,
    })),
  ];
  const marketValue = marketOptions.some((o) => o.value === locale) ? locale : '';

  return (
    <div className="space-y-5">
      {/* Style / tone */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('aiGenerate.style')}
        </div>
        <div className="space-y-1.5">
          {(Object.entries(MODES) as [GenerationMode, (typeof MODES)[GenerationMode]][]).map(
            ([id, m]) => (
              <Button
                key={id}
                onClick={() => onModeChange(id)}
                className={cn(
                  'w-full flex items-center gap-3 rounded-xl border px-3 py-2 text-left transition-all h-auto',
                  mode === id
                    ? 'border-brand/35 bg-brand/8 text-foreground/90'
                    : 'border-white/[0.05] bg-transparent text-foreground/50 hover:border-white/[0.08] hover:text-foreground/75'
                )}
              >
                <div className="flex-1 min-w-0">
                  <div className="text-[11px] font-medium">{m.label}</div>
                  <div className="text-[10px] text-foreground/35">{m.description}</div>
                </div>
                {mode === id && <div className="h-1.5 w-1.5 rounded-full bg-brand shrink-0" />}
              </Button>
            )
          )}
        </div>
      </div>

      {/* Emphasis directives (#15) — multi-select, fact-safe rewrite biases */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('aiGenerate.wizard.emphasis.label')}
        </div>
        <div className="grid grid-cols-1 gap-1.5 @xs:grid-cols-2">
          {EMPHASIS_OPTIONS.map(({ id }) => {
            const active = emphasis.includes(id);
            return (
              <Button
                key={id}
                onClick={() => toggleEmphasis(id)}
                aria-pressed={active}
                className={cn(
                  'flex items-center gap-2 rounded-xl border px-3 py-2 text-left transition-all h-auto',
                  active
                    ? 'border-brand/35 bg-brand/8 text-foreground/90'
                    : 'border-white/[0.05] bg-transparent text-foreground/50 hover:border-white/[0.08] hover:text-foreground/75'
                )}
              >
                <span
                  className={cn(
                    'flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded border',
                    active ? 'border-brand bg-brand text-white' : 'border-white/15'
                  )}
                >
                  {active && <Check size={10} />}
                </span>
                <span className="min-w-0">
                  <span className="block text-[11px] font-medium">
                    {t(`aiGenerate.wizard.emphasis.${id}.label`)}
                  </span>
                  <span className="block text-[10px] text-foreground/35">
                    {t(`aiGenerate.wizard.emphasis.${id}.desc`)}
                  </span>
                </span>
              </Button>
            );
          })}
        </div>
      </div>

      {/* Prompt quality */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('aiGenerate.wizard.quality.label')}
        </div>
        <div className="space-y-1.5">
          {QUALITY_OPTIONS.map(({ id, labelKey, descKey, icon: Icon }) => (
            <Button
              key={id}
              onClick={() => setPromptQuality(id)}
              className={cn(
                'w-full flex items-center gap-3 rounded-xl border px-3 py-2 text-left transition-all h-auto',
                promptQuality === id
                  ? 'border-brand/35 bg-brand/8 text-foreground/90'
                  : 'border-white/[0.05] bg-transparent text-foreground/50 hover:border-white/[0.08] hover:text-foreground/75'
              )}
            >
              <Icon size={14} className="shrink-0 text-brand-soft" />
              <div className="flex-1 min-w-0">
                <div className="text-[11px] font-medium">{t(labelKey)}</div>
                <div className="text-[10px] text-foreground/35">{t(descKey)}</div>
              </div>
              {promptQuality === id && (
                <div className="h-1.5 w-1.5 rounded-full bg-brand shrink-0" />
              )}
            </Button>
          ))}
        </div>
        {promptQuality === 'compact' && (
          <div className="mt-2 flex items-start gap-2 rounded-xl border border-amber-500/20 bg-amber-500/5 px-3 py-2">
            <Zap size={12} className="text-amber-400 mt-0.5 shrink-0" />
            <p className="text-[10px] text-amber-400/80 leading-relaxed">
              {t('aiGenerate.wizard.quality.compactWarning')}
            </p>
          </div>
        )}
        {promptQuality === 'full' && (
          <div className="mt-2 flex items-start gap-2 rounded-xl border border-orange-500/20 bg-orange-500/5 px-3 py-2">
            <AlertTriangle size={12} className="text-orange-400 mt-0.5 shrink-0" />
            <p className="text-[10px] text-orange-400/80 leading-relaxed">
              {t('aiGenerate.wizard.quality.fullWarning')}
            </p>
          </div>
        )}
      </div>

      {/* Target market — cover letter only */}
      {showCoverOptions && (
        <div>
          <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
            {t('aiGenerate.market.label')}
          </div>
          <Dropdown
            options={marketOptions}
            value={marketValue}
            onChange={onLocaleChange}
            placeholder={t('aiGenerate.market.auto')}
          />
          <p className="mt-1 text-[10px] leading-relaxed text-foreground/35">
            {t('aiGenerate.market.hint')}
          </p>
        </div>
      )}

      {/* Company research — cover letter only */}
      {showCoverOptions && (
        <div>
          <label className="flex cursor-pointer items-start gap-2 rounded-xl border border-white/[0.06] bg-white/[0.03] px-3 py-2">
            <input
              type="checkbox"
              checked={researchCompany}
              onChange={(e) => onResearchCompanyChange(e.target.checked)}
              className="mt-0.5 accent-[color:var(--color-brand)]"
            />
            <span className="min-w-0">
              <span className="block text-[11px] font-medium text-foreground/80">
                {t('aiGenerate.research.label')}
              </span>
              <span className="block text-[10px] text-foreground/40">
                {t('aiGenerate.research.hint')}
              </span>
              {showOllamaResearchHint && (
                <span className="mt-1 block text-[10px] text-amber-400/70">
                  {t('aiGenerate.research.ollamaKeyHint')}
                </span>
              )}
            </span>
          </label>
        </div>
      )}
    </div>
  );
}
