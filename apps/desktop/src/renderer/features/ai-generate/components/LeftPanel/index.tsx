import { AlertCircle, ArrowRight, RefreshCw, Wand2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { JobAdField } from '@/components/job/JobAdField';
import { ResumeInputCard } from '@/components/resume/ResumeInputCard';
import { AiSetupHint } from '@/components/ui/AiSetupHint';
import { ModelSelector } from '@/components/ui/ModelSelector';
import { GenerationMetadata } from '@/features/ai-generate/components/GenerationMetadata';
import { TemplateRecommendation } from '@/features/ai-generate/components/TemplateRecommendation';
import {
  type GenerationMeta,
  isDesignTier,
  isTwoColumnTemplate,
  type TemplateId,
} from '@/lib/generate';

interface Props {
  resume: string;
  jobAd: string;
  stage: string;
  meta: GenerationMeta | null;
  templateId: TemplateId;
  uploading: 'resume' | 'jobAd' | null;
  uploadError: string | null;
  canGenerate: boolean;
  canUseAI: boolean;
  aiReason: string;
  canProceed: boolean;
  setResume: (v: string) => void;
  setJobAd: (v: string) => void;
  setTemplateId: (v: TemplateId) => void;
  setAtsMode: (v: boolean) => void;
  setLocale: (v: string) => void;
  onUpload: (target: 'resume' | 'jobAd', file: File) => Promise<void>;
  onReset: () => void;
  onAnalyze: () => void;
}

export function LeftPanel({
  resume,
  jobAd,
  stage,
  meta,
  templateId,
  uploading,
  uploadError,
  canGenerate,
  canUseAI,
  aiReason,
  canProceed,
  setResume,
  setJobAd,
  setTemplateId,
  setAtsMode,
  setLocale,
  onUpload,
  onReset,
  onAnalyze,
}: Props) {
  const { t } = useTranslation();

  return (
    <div className="@container flex w-full md:w-[320px] lg:w-[380px] xl:w-[420px] shrink-0 flex-col overflow-y-auto">
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
          // Reset gates on design-tier semantics like the other four surfaces;
          // the auto-suggest guard above deliberately stays two-column-only.
          else if (!isDesignTier(id)) setAtsMode(false);
        }}
      />

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
