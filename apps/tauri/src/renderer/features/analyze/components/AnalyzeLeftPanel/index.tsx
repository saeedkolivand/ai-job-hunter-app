import {
  AlertCircle,
  AlertTriangle,
  Briefcase,
  GraduationCap,
  RefreshCw,
  ScanSearch,
  Sparkles,
  Zap,
} from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, SegmentedControl } from '@ajh/ui';

import { JobAdField } from '@/components/job/JobAdField';
import { ResumeInputCard } from '@/components/resume/ResumeInputCard';
import { AiSetupHint } from '@/components/ui/AiSetupHint';
import { ModelSelector } from '@/components/ui/ModelSelector';
import type { Stage } from '@/features/analyze/constants';
import type { AnalysisMode } from '@/lib/resume-ai';
import type { PromptQuality } from '@/store/preferences-schema';

interface Props {
  resume: string;
  jobAd: string;
  stage: Stage;
  uploading: 'resume' | 'jobAd' | null;
  uploadError: string | null;
  canRun: boolean;
  canUseAI: boolean;
  aiReason: string;
  promptQuality: PromptQuality;
  analysisMode: AnalysisMode;
  onUpload: (target: 'resume' | 'jobAd', file: File) => Promise<void>;
  onReset: () => void;
  onRun: () => void;
  setResume: (v: string) => void;
  setJobAd: (v: string) => void;
  setPromptQuality: (v: PromptQuality) => void;
  setAnalysisMode: (v: AnalysisMode) => void;
}

export function AnalyzeLeftPanel({
  resume,
  jobAd,
  stage,
  uploading,
  uploadError,
  canRun,
  canUseAI,
  aiReason,
  promptQuality,
  analysisMode,
  onUpload,
  onReset,
  onRun,
  setResume,
  setJobAd,
  setPromptQuality,
  setAnalysisMode,
}: Props) {
  const { t } = useTranslation();

  // Which half is missing — so the disabled CTA names the exact next step
  // ("Add your résumé" / "Add the job ad") instead of the generic prompt. Mirrors
  // the `canRun` length threshold in AnalyzePage so the label can't disagree.
  const hasResume = resume.trim().length > 50;
  const hasJobAd = jobAd.trim().length > 50;

  return (
    <div className="flex w-[400px] shrink-0 flex-col overflow-y-auto">
      {/* Header */}
      <div className="px-6 pt-8 pb-4">
        <div className="flex items-center justify-between mb-1">
          <div className="flex items-center gap-2">
            <div className="flex h-7 w-7 items-center justify-center rounded-lg bg-brand/15">
              <ScanSearch size={14} className="text-brand-soft" />
            </div>
            <span className="text-base font-semibold text-foreground/90">{t('analyze.title')}</span>
          </div>
          {stage !== 'idle' && (
            <Button
              onClick={onReset}
              className="flex items-center gap-1 text-[11px] text-foreground/40 hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
            >
              <RefreshCw size={11} /> {t('analyze.reset')}
            </Button>
          )}
        </div>
        <p className="text-xs text-foreground/40">{t('analyze.subtitle')}</p>
      </div>

      {/* Model selector */}
      <div className="px-6 pb-4">
        <ModelSelector />
      </div>

      {/* One-click AI setup when no provider is ready */}
      <div className="px-6">
        <AiSetupHint show={!canUseAI} reason={aiReason} />
      </div>

      {/* Document type — academic CV vs work résumé (#54). Drives the ATS criteria. */}
      <div className="px-6 pb-4">
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('analyze.mode.label')}
        </div>
        <SegmentedControl<AnalysisMode>
          variant="grid"
          ariaLabel={t('analyze.mode.label')}
          value={analysisMode}
          onChange={setAnalysisMode}
          options={[
            { value: 'work', label: t('analyze.mode.work'), icon: Briefcase },
            { value: 'academic', label: t('analyze.mode.academic'), icon: GraduationCap },
          ]}
        />
        <p className="mt-1.5 text-[10px] leading-relaxed text-foreground/35">
          {t(analysisMode === 'academic' ? 'analyze.mode.academicHint' : 'analyze.mode.workHint')}
        </p>
      </div>

      {/* Prompt quality selector */}
      <div className="px-6 pb-4">
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          Prompt Quality
        </div>
        <SegmentedControl<PromptQuality>
          variant="grid"
          ariaLabel={t('ai.promptQuality')}
          value={promptQuality}
          onChange={setPromptQuality}
          options={[
            { value: 'full', label: 'Full' },
            { value: 'auto', label: 'Auto' },
            { value: 'compact', label: 'Fast', icon: Zap },
          ]}
        />
        {promptQuality === 'compact' && (
          <div className="mt-2 flex items-start gap-2 rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2">
            <Zap size={11} className="text-amber-400 mt-0.5 shrink-0" />
            <p className="text-[10px] text-amber-400/80 leading-relaxed">
              Fast mode — rewrites and detailed suggestions are reduced for speed.
            </p>
          </div>
        )}
        {promptQuality === 'full' && (
          <div className="mt-2 flex items-start gap-2 rounded-lg border border-orange-500/20 bg-orange-500/5 px-3 py-2">
            <AlertTriangle size={11} className="text-orange-400 mt-0.5 shrink-0" />
            <p className="text-[10px] text-orange-400/80 leading-relaxed">
              Full mode on a small model may produce incomplete or noisy output.
            </p>
          </div>
        )}
      </div>

      {/* Inputs */}
      <div className="px-6 space-y-3 pb-4">
        <ResumeInputCard
          value={resume}
          onChange={setResume}
          onUpload={(f) => onUpload('resume', f)}
          uploading={uploading === 'resume'}
          disabled={stage === 'running'}
          placeholder={t('analyze.resumePlaceholder')}
        />
        <JobAdField
          label={t('analyze.jobAd')}
          value={jobAd}
          onChange={setJobAd}
          uploading={uploading === 'jobAd'}
          onUpload={(f: File) => onUpload('jobAd', f)}
          placeholder={t('analyze.jobAdPlaceholder')}
          uploadText={t('analyze.uploadButton')}
          disabled={stage === 'running'}
        />
        {uploadError && (
          <div className="flex items-center gap-2 rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-xs text-amber-200/80">
            <AlertCircle size={11} /> {uploadError}
          </div>
        )}
      </div>

      {/* CTA */}
      <div className="px-6 pb-6 mt-auto">
        <Button
          size="md"
          variant={canRun && stage !== 'running' ? 'glass' : 'ghost'}
          onClick={() => void onRun()}
          loading={stage === 'running'}
          disabled={!canRun || stage === 'running'}
          className={cn('w-full justify-center', 'transition-all duration-150 ease-out')}
        >
          {stage !== 'running' && <Sparkles size={14} />}
          {stage === 'running'
            ? t('analyze.running')
            : stage === 'done'
              ? t('analyze.reAnalyse')
              : !canUseAI
                ? aiReason === 'addApiKey'
                  ? t('analyze.addApiKey')
                  : aiReason === 'installCli'
                    ? t('analyze.installCli')
                    : t('analyze.selectModel')
                : !canRun
                  ? !hasResume && !hasJobAd
                    ? t('analyze.pasteContent')
                    : !hasResume
                      ? t('analyze.addResume')
                      : t('analyze.addJobAd')
                  : t('analyze.run')}
        </Button>
      </div>
    </div>
  );
}
