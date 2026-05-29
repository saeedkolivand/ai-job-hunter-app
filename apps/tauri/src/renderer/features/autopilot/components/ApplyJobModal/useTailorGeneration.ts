import { useRef, useState } from 'react';

import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  extractMetadata,
  generateCoverLetter,
  generateResume,
  type GenerationMeta,
  type GenerationMode,
  type TemplateId,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

export type TailorTarget = 'resume' | 'cover' | 'both';

const TEMPLATE: TemplateId = 'modern';
const MODE: GenerationMode = 'ats';

interface Params {
  jobDesc: string;
  model: string;
  canUse: boolean;
  hasDesc: boolean;
}

/** Owns the analyze → resume → cover-letter generation flow for ApplyJobModal. */
export function useTailorGeneration({ jobDesc, model, canUse, hasDesc }: Params) {
  const { t } = useTranslation();

  const [generating, setGenerating] = useState(false);
  const [phase, setPhase] = useState<'idle' | 'analyzing' | 'resume' | 'cover'>('idle');
  const [resumeOut, setResumeOut] = useState('');
  const [coverOut, setCoverOut] = useState('');
  const [activeOut, setActiveOut] = useState<'resume' | 'cover'>('cover');
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [meta, setMeta] = useState<GenerationMeta | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const abortRef = useRef<AbortController | null>(null);

  const output = activeOut === 'resume' ? resumeOut : coverOut;

  const abort = () => abortRef.current?.abort();

  const generate = async (resume: string, target: TailorTarget) => {
    if (!canUse || !hasDesc || generating || !resume.trim()) return;
    setError(null);
    setGenerating(true);
    setPhase('analyzing');
    setResumeOut('');
    setCoverOut('');
    const controller = new AbortController();
    abortRef.current = controller;
    try {
      const detected = await extractMetadata(resume, jobDesc, model);
      setMeta(detected);
      if (target === 'resume' || target === 'both') {
        setActiveOut('resume');
        setPhase('resume');
        const r = await generateResume(
          resume,
          jobDesc,
          detected,
          MODE,
          model,
          (tok) => setResumeOut((p) => p + tok),
          'en',
          controller.signal
        );
        setResumeOut(r);
      }
      if (target === 'cover' || target === 'both') {
        setActiveOut('cover');
        setPhase('cover');
        const c = await generateCoverLetter(
          resume,
          jobDesc,
          detected,
          MODE,
          model,
          (tok) => setCoverOut((p) => p + tok),
          'en',
          controller.signal
        );
        setCoverOut(c);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('autopilot.apply.failed'));
    } finally {
      setGenerating(false);
      setPhase('idle');
      abortRef.current = null;
    }
  };

  const phaseLabel =
    phase === 'analyzing'
      ? t('autopilot.apply.analyzing')
      : phase === 'resume'
        ? t('autopilot.apply.writingResume')
        : phase === 'cover'
          ? t('autopilot.apply.writingCover')
          : '';

  const copy = async () => {
    if (!output) return;
    await navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const exportAs = async (fmt: 'pdf' | 'docx' | 'txt') => {
    setExportOpen(false);
    if (!output) return;
    const docType = activeOut === 'resume' ? 'resume' : 'cover-letter';
    const fileMeta: GenerationMeta = meta ?? {
      candidateName: '',
      jobTitle: '',
      companyName: '',
      resumeLanguage: 'en',
      jobAdLanguage: 'en',
      mismatch: false,
      targetLanguage: 'en',
      topRequirements: [],
    };
    const name = buildFilename(fileMeta, docType, fmt);
    if (fmt === 'pdf') await exportPDF(output, name, docType, meta ?? undefined, TEMPLATE, false);
    else if (fmt === 'docx')
      await exportDOCX(output, name, docType, meta ?? undefined, TEMPLATE, false);
    else exportTXT(output, name);
  };

  return {
    generating,
    phase,
    phaseLabel,
    resumeOut,
    coverOut,
    activeOut,
    setActiveOut,
    output,
    error,
    copied,
    exportOpen,
    setExportOpen,
    generate,
    abort,
    copy,
    exportAs,
  };
}
