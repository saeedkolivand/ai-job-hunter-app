import { AnimatePresence } from 'motion/react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { ErrorState, useNotification } from '@ajh/ui';

import { ContactPromptModal } from '@/components/contact/ContactPromptModal';
import { PageTransition } from '@/components/layout/PageTransition';
import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { GenerateWizard } from '@/features/ai-generate/components/GenerateWizard';
import { LeftPanel } from '@/features/ai-generate/components/LeftPanel';
import { OutputPanelDone } from '@/features/ai-generate/components/OutputPanelDone';
import { OutputPanelExtracting } from '@/features/ai-generate/components/OutputPanelExtracting';
import { OutputPanelGenerating } from '@/features/ai-generate/components/OutputPanelGenerating';
import { OutputPanelIdle } from '@/features/ai-generate/components/OutputPanelIdle';
import { useFileUpload } from '@/features/ai-generate/hooks/useFileUpload';
import { useGeneration } from '@/features/ai-generate/hooks/useGeneration';
import { useStageRotation } from '@/features/ai-generate/hooks/useStageRotation';
import { useQualityRecheck } from '@/hooks/use-quality-recheck';
import { useResearchCompanyDefault } from '@/hooks/use-research-company-default';
import {
  type EmphasisId,
  type GenerationMode,
  isDecoratedLetterLayout,
  type LetterLayoutId,
  type TemplateId,
} from '@/lib/generate';
import { COPY_FEEDBACK_LONG_MS } from '@/lib/timings';
import { useExtractText } from '@/services';
import { useSaveAiGeneration } from '@/services/use-ai-generations';
import {
  resetAIGenerateAll,
  runAbortRef,
  runTokenStartRef,
  useAIGenerateRunStore,
  useSessionStore,
} from '@/store/session-store';

import { exportActiveDocument } from './export-document';
import { runSetters } from './run-setters';
import { useContactPromptGate } from './useContactPromptGate';

export function AIGeneratePage() {
  const { t } = useTranslation();

  const { aiGenerate, setAIGenerate } = useSessionStore();
  const run = useAIGenerateRunStore();
  const {
    resume,
    jobAd,
    jobUrl,
    board,
    stage,
    meta,
    mode,
    emphasis,
    target,
    templateId,
    atsMode,
    accent,
    letterLayoutId,
    locale,
    activeOut,
    report,
  } = aiGenerate;
  const { isGenerating, stageLabel, streamBuffer, thinkingBuffer } = run;
  const { modelLoading, tokenCount, genStep, error } = run;
  // Per-token text streams in the run store; the committed text lives in the session slice.
  const resumeOut = run.liveResume || aiGenerate.resumeOut;
  const coverOut = run.liveCover || aiGenerate.coverOut;

  const setResume = (v: string) => setAIGenerate({ resume: v });
  // A manual edit / paste-over / upload replaces the ad, so any URL-import
  // provenance is now stale (ADR-031): clear it so a since-replaced job's url
  // never persists onto this generation.
  const setJobAd = (v: string) => setAIGenerate({ jobAd: v, jobUrl: undefined, board: undefined });
  // URL-import fills the ad AND records its provenance atomically.
  const setJobAdFromImport = (text: string, provenance: { url: string; board?: string }) =>
    setAIGenerate({ jobAd: text, jobUrl: provenance.url, board: provenance.board });
  const setStage = (v: typeof stage) =>
    setAIGenerate(v === 'configuring' ? { stage: v, wizardStep: 0 } : { stage: v });
  const setMeta = (v: typeof meta) => setAIGenerate({ meta: v });
  const setReport = (v: typeof report) => setAIGenerate({ report: v });
  const setMode = (v: GenerationMode) => setAIGenerate({ mode: v });
  const setEmphasis = (v: EmphasisId[]) => setAIGenerate({ emphasis: v });
  const setTarget = (v: 'resume' | 'cover' | 'both') => setAIGenerate({ target: v });
  const setTemplateId = (v: TemplateId) => setAIGenerate({ templateId: v });
  const setAtsMode = (v: boolean) => setAIGenerate({ atsMode: v });
  const setAccent = (v: string | undefined) => setAIGenerate({ accent: v });
  const setLetterLayoutId = (v: LetterLayoutId) => setAIGenerate({ letterLayoutId: v });
  const setLocale = (v: string) => setAIGenerate({ locale: v });
  const { setResumeOut, setCoverOut, setIsGenerating, setStageLabel, setError } = runSetters;
  const { setStreamBuffer, setThinkingBuffer, setModelLoading } = runSetters;
  const { setTokenCount, setGenStep } = runSetters;
  const setActiveOut = (v: 'resume' | 'cover') => setAIGenerate({ activeOut: v });

  const [uploadError, setUploadError] = useState<string | null>(null);
  const [uploading, setUploading] = useState<'resume' | 'jobAd' | null>(null);
  const [copied, setCopied] = useState(false);
  const tokenStartRef = runTokenStartRef;
  // Company research for the cover letter — the initial default is capability-
  // driven (ON when the active model can web-search, OFF otherwise); a user
  // toggle always wins from then on.
  const [researchCompany, setResearchCompany] = useResearchCompanyDefault();

  const notify = useNotification();
  const selectedModel = useSelectedModel();
  const { canUse: canUseAI, reason: aiReason } = useCanUseAI();
  const extractTextMutation = useExtractText();

  const abortControllerRef = runAbortRef;

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
    setReport,
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
    setIsGenerating,
    notify,
    researchCompany,
    locale,
    emphasis,
    jobUrl,
    board
  );

  // Quality panel "Re-check" for the active document — owned HERE, not in
  // OutputPanelDone: this page holds the run state (`isGenerating`) and the
  // outputs, and it stays mounted across a run, while the done panel is swapped
  // out the moment Regenerate sets stage `generating` (an exiting
  // AnimatePresence child is never re-rendered with the new props, so a guard
  // living down there would be frozen at its pre-Regenerate values).
  const { recheck, rechecking } = useQualityRecheck({
    report,
    meta,
    sourceResume: resume,
    jobAd,
    docKind: activeOut === 'resume' ? 'resume' : 'coverLetter',
    onReportChange: setReport,
    resumeText: resumeOut,
    coverLetterText: coverOut,
    generating: isGenerating,
    jobUrl,
    board,
  });

  const canProceed = resume.trim().length > 50 && jobAd.trim().length > 50;
  const canGenerate = canProceed && canUseAI;

  const { contactModalOpen, closeContactModal, requestGenerate, continueFromContactPrompt } =
    useContactPromptGate(handleGenerate);

  const reset = () => {
    // Abort whenever a run is in flight — including the post-generation
    // validation/save window (stage is already 'done' there, not 'generating'),
    // so Reset can't leave a stale save persisting after the user left.
    resetAIGenerateAll();
  };

  const copyOutput = async () => {
    if (isGenerating) return;
    const text = activeOut === 'resume' ? resumeOut : coverOut;
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), COPY_FEEDBACK_LONG_MS);
  };

  const doExport = async (fmt: 'pdf' | 'docx' | 'txt') => {
    if (isGenerating) return;
    await exportActiveDocument(
      { activeOut, resumeOut, coverOut, meta, locale, templateId, atsMode, accent, letterLayoutId },
      fmt
    );
  };

  // Which document is still streaming (only meaningful in the progressive-reveal
  // window: stage `done`, résumé shown, cover still generating). Drives the cover
  // tab's "generating…" indicator in OutputPanelDone (#23).
  const generatingDoc: 'resume' | 'cover' | null = isGenerating
    ? genStep?.label === 'Cover Letter'
      ? 'cover'
      : 'resume'
    : null;

  return (
    <PageTransition className="h-full overflow-hidden">
      <div className="mx-auto flex h-full w-full max-w-6xl flex-col md:flex-row 2xl:max-w-7xl">
        <LeftPanel
          resume={resume}
          jobAd={jobAd}
          stage={stage}
          meta={meta}
          templateId={templateId}
          uploading={uploading}
          uploadError={uploadError}
          canGenerate={canGenerate}
          canUseAI={canUseAI}
          aiReason={aiReason ?? ''}
          canProceed={canProceed}
          setResume={setResume}
          setJobAd={setJobAd}
          onJobAdImport={setJobAdFromImport}
          setTemplateId={setTemplateId}
          setAtsMode={setAtsMode}
          // Applying a template recommendation must not clear atsMode out from
          // under a decorated cover letter — that flag is the letter's only way
          // to drop its rail / tile / band.
          letterAtsApplies={target !== 'resume' && isDecoratedLetterLayout(letterLayoutId)}
          setLocale={setLocale}
          onUpload={handleUpload}
          onReset={reset}
          onAnalyze={handleAnalyze}
        />

        <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
          <AnimatePresence mode="wait">
            {stage === 'idle' && <OutputPanelIdle />}

            {stage === 'configuring' && (
              <GenerateWizard
                key="wizard"
                mode={mode}
                emphasis={emphasis}
                target={target}
                templateId={templateId}
                atsMode={atsMode}
                accent={accent}
                letterLayoutId={letterLayoutId}
                locale={locale}
                researchCompany={researchCompany}
                isGenerating={isGenerating}
                onModeChange={setMode}
                onEmphasisChange={setEmphasis}
                onTargetChange={setTarget}
                onTemplateChange={setTemplateId}
                onAtsModeChange={setAtsMode}
                onAccentChange={setAccent}
                onLetterLayoutChange={setLetterLayoutId}
                onLocaleChange={setLocale}
                onResearchCompanyChange={setResearchCompany}
                onGenerate={requestGenerate}
              />
            )}

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
                report={report}
                onRecheck={recheck}
                rechecking={rechecking}
                sourceResume={resume}
                jobAd={jobAd}
                mode={mode}
                templateId={templateId}
                atsMode={atsMode}
                accent={accent}
                letterLayoutId={letterLayoutId}
                locale={locale}
                onActiveOutChange={setActiveOut}
                onLetterLayoutChange={setLetterLayoutId}
                onAtsModeChange={setAtsMode}
                onCopy={() => void copyOutput()}
                onExport={doExport}
                onOutputChange={activeOut === 'resume' ? setResumeOut : setCoverOut}
                onRegenerate={() => void handleGenerate()}
                copied={copied}
                isGenerating={isGenerating}
                generatingDoc={generatingDoc}
              />
            )}
          </AnimatePresence>

          {error && (
            <div className="shrink-0 mx-6 mb-4">
              <ErrorState
                title={t('aiGenerate.error')}
                description={error}
                onRetry={() => void handleGenerate()}
                className="rounded-xl border border-red-400/20 bg-red-400/5 py-6"
              />
            </div>
          )}
        </div>
      </div>

      <ContactPromptModal
        open={contactModalOpen}
        onClose={closeContactModal}
        onContinue={continueFromContactPrompt}
      />
    </PageTransition>
  );
}
