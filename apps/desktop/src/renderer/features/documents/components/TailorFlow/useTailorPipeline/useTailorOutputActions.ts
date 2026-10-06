import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { errorClass } from '@/lib/error-class';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  type GenerationMeta,
  type LetterLayoutId,
  type resolveMarket,
  type TemplateId,
} from '@/lib/generate';
import { COPY_FEEDBACK_MS } from '@/lib/timings';

interface Params {
  output: string;
  activeOut: 'resume' | 'cover';
  meta: GenerationMeta | null;
  market: ReturnType<typeof resolveMarket>;
  templateId: TemplateId;
  atsMode: boolean;
  accent?: string;
  letterLayoutId?: LetterLayoutId;
}

/** Copy-to-clipboard and export of the document currently on screen. */
export function useTailorOutputActions({
  output,
  activeOut,
  meta,
  market,
  templateId,
  atsMode,
  accent,
  letterLayoutId,
}: Params) {
  const { t } = useTranslation();
  const notify = useNotification();
  const [copied, setCopied] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);

  const copy = async () => {
    if (!output) return;
    await navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), COPY_FEEDBACK_MS);
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
    try {
      if (fmt === 'txt') exportTXT(output, name);
      else {
        const exporter = fmt === 'pdf' ? exportPDF : exportDOCX;
        await exporter(
          output,
          name,
          docType,
          meta ?? undefined,
          templateId,
          atsMode,
          market,
          accent,
          letterLayoutId
        );
      }
    } catch (err) {
      console.error('[export] failed', {
        format: fmt,
        docType,
        error: errorClass(err),
      });
      notify.error({
        message: err instanceof Error && err.message ? err.message : t('common.exportFailed'),
      });
    }
  };

  return { copied, exportOpen, setExportOpen, copy, exportAs };
}
