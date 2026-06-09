import { ArrowRight, RotateCcw } from 'lucide-react';
import { AnimatePresence } from 'motion/react';
import { useState } from 'react';

import { Button, ErrorState } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { AiSetupHint } from '@/components/ui/AiSetupHint';
import { ModelSelector } from '@/components/ui/ModelSelector';
import { OutputPanelDone } from '@/features/ai-generate/components/OutputPanelDone';
import { OutputPanelGenerating } from '@/features/ai-generate/components/OutputPanelGenerating';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  isTwoColumnTemplate,
  type TemplateId,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import { useResumeBuilder } from '../../hooks/useResumeBuilder';
import { BuilderWizard } from '../BuilderWizard';

export function ResumeBuilderPage() {
  const { t } = useTranslation();
  const {
    language,
    locale,
    templateId,
    atsMode,
    stage,
    output,
    setResumeBuilder,
    meta,
    canGenerate,
    canUseAI,
    aiReason,
    isComplete,
    isGenerating,
    streamBuffer,
    thinkingBuffer,
    modelLoading,
    tokenCount,
    tokenStartMs,
    error,
    synthesize,
    tailorToJob,
    reset,
  } = useResumeBuilder();

  const [copied, setCopied] = useState(false);

  const onLanguageChange = (lng: string) => setResumeBuilder({ language: lng, locale: lng });
  const onTemplateChange = (id: TemplateId) =>
    setResumeBuilder(
      isTwoColumnTemplate(id) ? { templateId: id } : { templateId: id, atsMode: false }
    );
  const onAtsModeChange = (enabled: boolean) => setResumeBuilder({ atsMode: enabled });

  const copyOutput = async () => {
    if (isGenerating || !output) return;
    await navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), 1800);
  };

  const doExport = async (fmt: 'pdf' | 'docx' | 'txt') => {
    if (isGenerating || !output) return;
    const name = buildFilename(meta, 'resume', fmt);
    if (fmt === 'pdf') await exportPDF(output, name, 'resume', meta, templateId, atsMode, locale);
    if (fmt === 'docx') await exportDOCX(output, name, 'resume', meta, templateId, atsMode, locale);
    if (fmt === 'txt') exportTXT(output, name);
  };

  return (
    <PageTransition className="h-full overflow-hidden">
      <div className="flex h-full flex-col">
        {/* Header: title + model picker */}
        <div className="shrink-0 flex items-center justify-between gap-4 border-b border-white/8 px-8 py-4">
          <div>
            <h1 className="text-sm font-semibold text-foreground/90">{t('build.title')}</h1>
            <p className="text-xs text-foreground/40">{t('build.subtitle')}</p>
          </div>
          <div className="flex items-center gap-3">
            <AiSetupHint show={!canUseAI} reason={aiReason} />
            <ModelSelector />
          </div>
        </div>

        <div className="flex flex-1 flex-col overflow-hidden">
          <AnimatePresence mode="wait">
            {stage === 'interview' && (
              <BuilderWizard
                key="wizard"
                language={language}
                templateId={templateId}
                atsMode={atsMode}
                isComplete={isComplete}
                canGenerate={canGenerate}
                isGenerating={isGenerating}
                onLanguageChange={onLanguageChange}
                onTemplateChange={onTemplateChange}
                onAtsModeChange={onAtsModeChange}
                onGenerate={() => void synthesize()}
              />
            )}

            {stage === 'generating' && (
              <OutputPanelGenerating
                key="generating"
                stageLabel={t('build.wizard.generating')}
                streamBuffer={streamBuffer}
                activeOut="resume"
                thinkingBuffer={thinkingBuffer}
                modelLoading={modelLoading}
                tokenCount={tokenCount}
                tokenStartMs={tokenStartMs}
              />
            )}

            {stage === 'done' && (
              <div key="done" className="flex flex-1 flex-col overflow-hidden">
                <OutputPanelDone
                  resumeOut={output}
                  coverOut=""
                  activeOut="resume"
                  meta={meta}
                  mode="ats"
                  templateId={templateId}
                  atsMode={atsMode}
                  locale={locale}
                  onActiveOutChange={() => {}}
                  onCopy={() => void copyOutput()}
                  onExport={doExport}
                  onOutputChange={(value) => setResumeBuilder({ output: value })}
                  onRegenerate={() => void synthesize()}
                  copied={copied}
                  isGenerating={isGenerating}
                  generatingDoc={null}
                />
                <div className="shrink-0 flex items-center justify-end gap-2 border-t border-white/8 px-8 py-3">
                  <Button onClick={reset} variant="ghost" size="sm" className="gap-1.5">
                    <RotateCcw size={14} />
                    {t('build.output.startOver')}
                  </Button>
                  <Button onClick={tailorToJob} variant="glass" size="sm" className="gap-1.5">
                    {t('build.output.tailor')}
                    <ArrowRight size={14} />
                  </Button>
                </div>
              </div>
            )}
          </AnimatePresence>

          {error && (
            <div className="shrink-0 mx-6 mb-4">
              <ErrorState
                title={t('build.toast.failed')}
                description={error}
                onRetry={() => void synthesize()}
                className="rounded-xl border border-red-400/20 bg-red-400/5 py-6"
              />
            </div>
          )}
        </div>
      </div>
    </PageTransition>
  );
}
