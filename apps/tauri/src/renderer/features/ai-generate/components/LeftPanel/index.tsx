import { AlertCircle, ArrowRight, RefreshCw, Wand2 } from 'lucide-react';

import { Button, SelectDropdown } from '@ajh/ui';

import { JobAdField } from '@/components/job/JobAdField';
import { ResumeInputCard } from '@/components/resume/ResumeInputCard';
import { AiSetupHint } from '@/components/ui/AiSetupHint';
import { ModelSelector } from '@/components/ui/ModelSelector';
import { GenerationConfig } from '@/features/ai-generate/components/GenerationConfig';
import { GenerationMetadata } from '@/features/ai-generate/components/GenerationMetadata';
import { TemplateRecommendation } from '@/features/ai-generate/components/TemplateRecommendation';
import { isOllamaFamily } from '@/lib/ai-providers/provider-meta';
import {
  type GenerationMeta,
  type GenerationMode,
  isTwoColumnTemplate,
  LETTER_MARKET_IDS,
  letterConventions,
  type TemplateId,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useHasProviderKey } from '@/services';
import { useAiProviderConfig } from '@/store/preferences-store';

import type { PreviewFocus } from '../../samples';

interface Props {
  resume: string;
  jobAd: string;
  stage: string;
  meta: GenerationMeta | null;
  mode: GenerationMode;
  target: 'resume' | 'cover' | 'both';
  templateId: TemplateId;
  atsMode: boolean;
  /** Target-market id for the cover letter ('' / language code = auto-detect). */
  locale: string;
  uploading: 'resume' | 'jobAd' | null;
  uploadError: string | null;
  canGenerate: boolean;
  canUseAI: boolean;
  aiReason: string;
  canProceed: boolean;
  setResume: (v: string) => void;
  setJobAd: (v: string) => void;
  setMode: (v: GenerationMode) => void;
  setTarget: (v: 'resume' | 'cover' | 'both') => void;
  setTemplateId: (v: TemplateId) => void;
  setAtsMode: (v: boolean) => void;
  setLocale: (v: string) => void;
  researchCompany: boolean;
  onResearchCompanyChange: (v: boolean) => void;
  onUpload: (target: 'resume' | 'jobAd', file: File) => Promise<void>;
  onReset: () => void;
  onAnalyze: () => void;
  onGenerate: () => void;
  isGenerating: boolean;
  onPreviewFocus: (focus: PreviewFocus) => void;
}

export function LeftPanel({
  resume,
  jobAd,
  stage,
  meta,
  mode,
  target,
  templateId,
  atsMode,
  locale,
  uploading,
  uploadError,
  canGenerate,
  canUseAI,
  aiReason,
  canProceed,
  setResume,
  setJobAd,
  setMode,
  setTarget,
  setTemplateId,
  setAtsMode,
  setLocale,
  researchCompany,
  onResearchCompanyChange,
  onUpload,
  onReset,
  onAnalyze,
  onGenerate,
  isGenerating,
  onPreviewFocus,
}: Props) {
  const { t } = useTranslation();
  const providerConfig = useAiProviderConfig();
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  // Research works for every provider; only Ollama-family needs the (free) Ollama
  // account key, since they search via the Ollama Web Search API. The checkbox is
  // never hidden — we just nudge when that key is missing.
  const { data: ollamaKey } = useHasProviderKey('ollama-cloud');
  const showOllamaResearchHint = isOllamaFamily(activeProvider) && !(ollamaKey?.has ?? false);

  // Target-market options for the cover letter: an "Auto (detected)" entry plus
  // each supported market (labelled by country). Auto resolves from the job's
  // detected country at generation + export time, so they always agree.
  const marketOptions = [
    { value: '', label: t('aiGenerate.market.auto') },
    ...LETTER_MARKET_IDS.filter((id) => id !== 'intl').map((id) => ({
      value: id,
      label: letterConventions(id).country,
    })),
  ];
  const marketValue = marketOptions.some((o) => o.value === locale) ? locale : '';

  return (
    <div className="flex w-[420px] shrink-0 flex-col border-r border-white/[0.05] overflow-y-auto">
      {/* Header */}
      <div className="px-6 pt-8 pb-4">
        <div className="flex items-center justify-between mb-1">
          <div className="flex items-center gap-2">
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-brand/15">
              <Wand2 size={14} className="text-brand-soft" />
            </div>
            <span className="text-base font-semibold text-foreground/90">
              {t('aiGenerate.title')}
            </span>
          </div>
          {(stage === 'configuring' || stage === 'done') && (
            <Button
              onClick={onReset}
              className="flex items-center gap-1 text-[11px] text-foreground/40 hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
            >
              <RefreshCw size={11} /> {t('aiGenerate.regenerate')}
            </Button>
          )}
        </div>
        <p className="text-xs text-foreground/40">{t('aiGenerate.subtitle')}</p>
      </div>

      {/* Model selector */}
      <div className="px-6 pb-4">
        <ModelSelector />
      </div>

      {/* One-click AI setup when no provider is ready */}
      <div className="px-6">
        <AiSetupHint show={!canUseAI} reason={aiReason} />
      </div>

      <div className="px-6 space-y-3 pb-4">
        {/* Resume input */}
        <ResumeInputCard
          value={resume}
          onChange={setResume}
          onUpload={(f) => onUpload('resume', f)}
          uploading={uploading === 'resume'}
          disabled={stage !== 'idle'}
          placeholder={t('aiGenerate.resumePlaceholder')}
        />

        {/* Job ad input */}
        <JobAdField
          label={t('aiGenerate.jobAdLabel')}
          value={jobAd}
          onChange={setJobAd}
          uploading={uploading === 'jobAd'}
          onUpload={(f: File) => onUpload('jobAd', f)}
          placeholder={t('aiGenerate.placeholder').replace('…', '')}
          uploadText={t('aiGenerate.upload')}
          disabled={stage !== 'idle'}
        />

        {uploadError && (
          <div className="flex items-center gap-2 rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-xs text-amber-200/80">
            <AlertCircle size={11} /> {uploadError}
          </div>
        )}
      </div>

      {/* Detected metadata — shown after extraction */}
      <GenerationMetadata meta={meta} />

      {/* Template + locale suggestion from the detected metadata */}
      <TemplateRecommendation
        meta={meta}
        templateId={templateId}
        onApply={(id, atsSuggested, recommendedLocale) => {
          setTemplateId(id);
          setLocale(recommendedLocale);
          if (atsSuggested && isTwoColumnTemplate(id)) setAtsMode(true);
          else if (!isTwoColumnTemplate(id)) setAtsMode(false);
        }}
      />

      {/* Config — target + mode */}
      <GenerationConfig
        stage={stage}
        mode={mode}
        target={target}
        templateId={templateId}
        atsMode={atsMode}
        onModeChange={setMode}
        onTargetChange={setTarget}
        onTemplateChange={(id) => {
          setTemplateId(id);
          if (!isTwoColumnTemplate(id)) setAtsMode(false);
        }}
        onAtsModeChange={setAtsMode}
        onGenerate={onGenerate}
        isGenerating={isGenerating}
        onPreviewFocus={onPreviewFocus}
      />

      {/* Target market — drives the cover letter's etiquette + exported layout.
          Auto-detected from the job's country; override here. */}
      {(target === 'cover' || target === 'both') && (
        <div className="px-6 pb-2">
          <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
            {t('aiGenerate.market.label')}
          </div>
          <SelectDropdown
            options={marketOptions}
            value={marketValue}
            onChange={setLocale}
            placeholder={t('aiGenerate.market.auto')}
          />
          <p className="mt-1 text-[10px] leading-relaxed text-foreground/35">
            {t('aiGenerate.market.hint')}
          </p>
        </div>
      )}

      {/* Opt-in company research — only when a cover letter is produced. Default
          off, so generation makes no extra web/LLM call unless the user asks. */}
      {(target === 'cover' || target === 'both') && (
        <div className="px-6 pb-2">
          <label className="flex cursor-pointer items-start gap-2 rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-2">
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

      {/* Idle CTA */}
      {stage === 'idle' && (
        <div className="px-6 pb-6 mt-auto">
          <Button
            size="md"
            variant={canGenerate ? 'glass' : 'ghost'}
            onClick={onAnalyze}
            disabled={!canGenerate}
            className="w-full justify-center transition-all duration-150 ease-out"
          >
            <ArrowRight size={14} />
            {!canUseAI
              ? aiReason === 'addApiKey'
                ? t('aiGenerate.addApiKey')
                : aiReason === 'installCli'
                  ? t('aiGenerate.installCli')
                  : t('aiGenerate.selectModel')
              : !canProceed
                ? t('aiGenerate.pasteResumeJob')
                : t('aiGenerate.continue')}
          </Button>
        </div>
      )}
    </div>
  );
}
