import { AnimatePresence } from 'motion/react';
import { useRef, useState } from 'react';

import { ContactPromptModal } from '@/components/contact/ContactPromptModal';
import { PageTransition } from '@/components/layout/PageTransition';
import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { LeftPanel } from '@/features/ai-generate/components/LeftPanel';
import { OutputPanelDone } from '@/features/ai-generate/components/OutputPanelDone';
import { OutputPanelExtracting } from '@/features/ai-generate/components/OutputPanelExtracting';
import { OutputPanelGenerating } from '@/features/ai-generate/components/OutputPanelGenerating';
import { OutputPanelIdle } from '@/features/ai-generate/components/OutputPanelIdle';
import { useFileUpload } from '@/features/ai-generate/hooks/useFileUpload';
import { useGeneration } from '@/features/ai-generate/hooks/useGeneration';
import { useStageRotation } from '@/features/ai-generate/hooks/useStageRotation';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  type GenerationMode,
  type TemplateId,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useExtractText } from '@/services';
import { useSaveAiGeneration } from '@/services/use-ai-generations';
import { useContactPromptSeen, usePreferencesStore } from '@/store/preferences-store';
import { useSessionStore } from '@/store/session-store';

type GenTarget = 'resume' | 'cover' | 'both';

export function AIGeneratePage() {
  const { t } = useTranslation();

  const { aiGenerate, setAIGenerate, resetAIGenerate } = useSessionStore();
  const {
    resume,
    jobAd,
    stage,
    meta,
    mode,
    target,
    templateId,
    atsMode,
    locale,
    resumeOut,
    coverOut,
    activeOut,
  } = aiGenerate;

  const setResume = (v: string) => setAIGenerate({ resume: v });
  const setJobAd = (v: string) => setAIGenerate({ jobAd: v });
  const setStage = (v: typeof stage) => setAIGenerate({ stage: v });
  const setMeta = (v: typeof meta) => setAIGenerate({ meta: v });
  const setMode = (v: GenerationMode) => setAIGenerate({ mode: v });
  const setTarget = (v: GenTarget) => setAIGenerate({ target: v });
  const setTemplateId = (v: TemplateId) => setAIGenerate({ templateId: v });
  const setAtsMode = (v: boolean) => setAIGenerate({ atsMode: v });
  const setLocale = (v: string) => setAIGenerate({ locale: v });
  const setResumeOut = (v: string | ((p: string) => string)) =>
    setAIGenerate({ resumeOut: typeof v === 'function' ? v(resumeOut) : v });
  const setCoverOut = (v: string | ((p: string) => string)) =>
    setAIGenerate({ coverOut: typeof v === 'function' ? v(coverOut) : v });
  const setActiveOut = (v: 'resume' | 'cover') => setAIGenerate({ activeOut: v });

  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploading, setUploading] = useState<'resume' | 'jobAd' | null>(null);
  const [stageLabel, setStageLabel] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [streamBuffer, setStreamBuffer] = useState('');
  const [thinkingBuffer, setThinkingBuffer] = useState('');
  const [copied, setCopied] = useState(false);
  const [modelLoading, setModelLoading] = useState(false);
  const [tokenCount, setTokenCount] = useState(0);
  const tokenStartRef = useRef<number | null>(null);
  const [genStep, setGenStep] = useState<{ current: number; total: number; label: string } | null>(
    null
  );
  // Opt-in company research for the cover letter — default off (no extra web/LLM call).
  const [researchCompany, setResearchCompany] = useState(false);

  const selectedModel = useSelectedModel();
  const { canUse: canUseAI, reason: aiReason } = useCanUseAI();
  const extractTextMutation = useExtractText();

  const abortControllerRef = useRef<AbortController | null>(null);

  const { handleUpload } = useFileUpload(
    setUploadError,
    setUploading,
    setResume,
    setJobAd,
    extractTextMutation,
    t
  );

  const { startStageRotation, stopStageRotation } = useStageRotation(setStageLabel, t);

  const saveAiGeneration = useSaveAiGeneration();

  const { handleAnalyze, handleGenerate } = useGeneration(
    resume,
    jobAd,
    meta,
    mode,
    target,
    selectedModel,
    setStage,
    setMeta,
    setResumeOut,
    setCoverOut,
    setActiveOut,
    setStreamBuffer,
    setThinkingBuffer,
    setModelLoading,
    setTokenCount,
    setGenStep,
    setError,
    tokenStartRef,
    startStageRotation,
    stopStageRotation,
    abortControllerRef,
    saveAiGeneration,
    t,
    setStageLabel,
    researchCompany
  );

  const canProceed = resume.trim().length > 50 && jobAd.trim().length > 50;
  const canGenerate = canProceed && canUseAI;

  // First-run nudge: before the very first generation, surface the contact profile
  // so the document header is complete. Shown once (persisted flag), then never
  // again — afterwards Generate runs straight through.
  const contactPromptSeen = useContactPromptSeen();
  const setContactPromptSeen = usePreferencesStore((s) => s.setContactPromptSeen);
  const [contactModalOpen, setContactModalOpen] = useState(false);

  const requestGenerate = () => {
    if (!contactPromptSeen) {
      setContactPromptSeen();
      setContactModalOpen(true);
      return;
    }
    void handleGenerate();
  };

  const continueFromContactPrompt = () => {
    setContactModalOpen(false);
    void handleGenerate();
  };

  const reset = () => {
    if (abortControllerRef.current && stage === 'generating') {
      abortControllerRef.current.abort();
    }
    stopStageRotation();
    setError(null);
    setStreamBuffer('');
    setThinkingBuffer('');
    resetAIGenerate();
  };

  const copyOutput = async () => {
    if (isGenerating) return;
    const text = activeOut === 'resume' ? resumeOut : coverOut;
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1800);
  };

  const currentOutput = activeOut === 'resume' ? resumeOut : coverOut;

  const doExport = async (fmt: 'pdf' | 'docx' | 'txt') => {
    if (isGenerating) return;
    const text = currentOutput;
    if (!text) return;
    const type = activeOut === 'resume' ? 'resume' : 'cover-letter';
    const name = buildFilename(
      meta ?? {
        candidateName: '',
        jobTitle: '',
        companyName: '',
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        targetLanguage: 'en',
        topRequirements: [],
      },
      type,
      fmt
    );
    if (fmt === 'pdf') {
      await exportPDF(text, name, type, meta ?? undefined, templateId, atsMode, locale);
    }
    if (fmt === 'docx') {
      await exportDOCX(text, name, type, meta ?? undefined, templateId, atsMode, locale);
    }
    if (fmt === 'txt') {
      exportTXT(text, name);
    }
  };

  const isGenerating = stage === 'generating';

  return (
    <PageTransition className="h-full overflow-hidden">
      <div className="flex h-full">
        <LeftPanel
          resume={resume}
          jobAd={jobAd}
          stage={stage}
          meta={meta}
          mode={mode}
          target={target}
          templateId={templateId}
          atsMode={atsMode}
          uploading={uploading}
          uploadError={uploadError}
          canGenerate={canGenerate}
          canUseAI={canUseAI}
          aiReason={aiReason ?? ''}
          canProceed={canProceed}
          setResume={setResume}
          setJobAd={setJobAd}
          setMode={setMode}
          setTarget={setTarget}
          setTemplateId={setTemplateId}
          setAtsMode={setAtsMode}
          setLocale={setLocale}
          researchCompany={researchCompany}
          onResearchCompanyChange={setResearchCompany}
          onUpload={handleUpload}
          onReset={reset}
          onAnalyze={handleAnalyze}
          onGenerate={requestGenerate}
          isGenerating={isGenerating}
        />

        <div className="flex flex-1 flex-col overflow-hidden">
          <AnimatePresence mode="wait">
            {(stage === 'idle' || stage === 'configuring') && <OutputPanelIdle />}

            {stage === 'extracting' && <OutputPanelExtracting stageLabel={stageLabel} />}

            {stage === 'generating' && (
              <OutputPanelGenerating
                stageLabel={stageLabel}
                streamBuffer={streamBuffer}
                activeOut={activeOut}
                thinkingBuffer={thinkingBuffer}
                modelLoading={modelLoading}
                genStep={genStep}
                tokenCount={tokenCount}
                tokenStartMs={tokenStartRef.current}
              />
            )}

            {stage === 'done' && (
              <OutputPanelDone
                resumeOut={resumeOut}
                coverOut={coverOut}
                activeOut={activeOut}
                meta={meta}
                mode={mode}
                templateId={templateId}
                onActiveOutChange={setActiveOut}
                onCopy={() => void copyOutput()}
                onExport={doExport}
                onOutputChange={(value) => {
                  if (activeOut === 'resume') setResumeOut(value);
                  else setCoverOut(value);
                }}
                onRegenerate={() => void handleGenerate()}
                copied={copied}
                isGenerating={isGenerating}
              />
            )}
          </AnimatePresence>

          {error && (
            <div className="shrink-0 mx-6 mb-4 rounded-xl border border-red-400/20 bg-red-400/5 px-4 py-3 text-xs text-red-300/80">
              <div className="font-medium mb-0.5">{t('aiGenerate.error')}</div>
              {error}
            </div>
          )}
        </div>
      </div>

      <ContactPromptModal
        open={contactModalOpen}
        onClose={() => setContactModalOpen(false)}
        onContinue={continueFromContactPrompt}
      />
    </PageTransition>
  );
}
