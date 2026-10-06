import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import type { ExportFormat } from '@/components/generation/ExportPicker';
import { errorClass } from '@/lib/error-class';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  resolveMarket,
  type TemplateId,
} from '@/lib/generate';

type GenerationExportMeta = Parameters<typeof buildFilename>[0];

interface UseGenerationExportArgs {
  meta: GenerationExportMeta;
  resumeDraft: string;
  coverDraft: string;
}

/** Export picker state plus the per-document export action for one generation card. */
export function useGenerationExport({ meta, resumeDraft, coverDraft }: UseGenerationExportArgs) {
  const { t } = useTranslation();
  const notify = useNotification();
  const [format, setFormat] = useState<ExportFormat>('pdf');
  const [template, setTemplate] = useState<TemplateId>('classic');
  // Per-export document accent (6-hex) — undefined = the template's own palette.
  const [accent, setAccent] = useState<string | undefined>(undefined);
  const [exporting, setExporting] = useState<'resume' | 'cover' | null>(null);

  const doExport = async (type: 'resume' | 'cover') => {
    const text = type === 'resume' ? resumeDraft : coverDraft;
    if (!text) return;
    const docType = type === 'resume' ? 'resume' : 'cover-letter';
    const filename = buildFilename(meta, docType, format);
    // Cover-letter market — the backend's `complete_letter_text` synthesizes a
    // body-only letter's salutation/sign-off from this locale, defaulting to
    // "intl" (English) when omitted, regardless of the letter's real language.
    // Resolved the same way `AIGeneratePage`/`useTailorPipeline` do. This record
    // carries no structured job location/country (unlike `GenerationMeta`'s
    // optional `jobCountry`), so language is the whole signal here — the correct
    // floor per `resolveMarket`'s own country → language → intl priority.
    // Résumé export keeps `locale` unset, mirroring `AIGeneratePage.doExport`
    // (there `locale` is the user's own template-locale picker, not the job
    // market; this card has no such picker, so there's nothing to preserve).
    const coverLetterLocale =
      docType === 'cover-letter'
        ? resolveMarket({ targetLanguage: meta.targetLanguage })
        : undefined;
    setExporting(type);
    try {
      if (format === 'txt') {
        exportTXT(text, filename.replace('.txt', ''));
      } else {
        const exporter = format === 'pdf' ? exportPDF : exportDOCX;
        await exporter(
          text,
          filename.replace(`.${format}`, ''),
          docType,
          meta,
          template,
          false,
          coverLetterLocale,
          accent
        );
      }
    } catch (err) {
      console.error('[export] failed', {
        format,
        docType,
        // Never the raw message — it can embed a header URL. See `errorClass`.
        error: errorClass(err),
      });
      notify.error({
        message: err instanceof Error && err.message ? err.message : t('common.exportFailed'),
      });
    } finally {
      setExporting(null);
    }
  };

  return { format, setFormat, template, setTemplate, accent, setAccent, exporting, doExport };
}
