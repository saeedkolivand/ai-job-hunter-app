import { AlertTriangle, Gauge, type LucideIcon, SlidersHorizontal, Zap } from 'lucide-react';

import { Button, cn, SelectDropdown } from '@ajh/ui';

import { isOllamaFamily } from '@/lib/ai-providers/provider-meta';
import { type GenerationMode, LETTER_MARKET_IDS, letterConventions, MODES } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useHasProviderKey } from '@/services';
import type { PromptQuality } from '@/store/preferences-schema';
import {
  useAiProviderConfig,
  usePreferencesStore,
  usePromptQuality,
} from '@/store/preferences-store';

interface StepFineTuneProps {
  mode: GenerationMode;
  target: 'resume' | 'cover' | 'both';
  locale: string;
  researchCompany: boolean;
  onModeChange: (mode: GenerationMode) => void;
  onLocaleChange: (locale: string) => void;
  onResearchCompanyChange: (v: boolean) => void;
}

const QUALITY_OPTIONS: { id: PromptQuality; labelKey: string; icon: LucideIcon }[] = [
  { id: 'full', labelKey: 'aiGenerate.wizard.quality.full', icon: SlidersHorizontal },
  { id: 'auto', labelKey: 'aiGenerate.wizard.quality.auto', icon: Gauge },
  { id: 'compact', labelKey: 'aiGenerate.wizard.quality.fast', icon: Zap },
];

export function StepFineTune({
  mode,
  target,
  locale,
  researchCompany,
  onModeChange,
  onLocaleChange,
  onResearchCompanyChange,
}: StepFineTuneProps) {
  const { t } = useTranslation();
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
      <div>
        <p className="text-sm font-semibold text-foreground/70">{t('aiGenerate.wizard.steps.2')}</p>
      </div>

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
                  <div className="text-[10px] text-foreground/35 truncate">{m.description}</div>
                </div>
                {mode === id && <div className="h-1.5 w-1.5 rounded-full bg-brand shrink-0" />}
              </Button>
            )
          )}
        </div>
      </div>

      {/* Prompt quality */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('aiGenerate.wizard.quality.label')}
        </div>
        <div className="grid grid-cols-3 gap-1.5">
          {QUALITY_OPTIONS.map(({ id, labelKey, icon: Icon }) => (
            <Button
              key={id}
              onClick={() => setPromptQuality(id)}
              className={cn(
                'flex flex-col items-center gap-1 rounded-xl border py-2.5 text-[11px] font-medium transition-all h-auto',
                promptQuality === id
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-white/[0.06] bg-white/[0.02] text-foreground/45 hover:border-white/10 hover:text-foreground/70'
              )}
            >
              <Icon size={12} />
              {t(labelKey)}
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
          <SelectDropdown
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
              className="mt-0.5 accent-brand"
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
