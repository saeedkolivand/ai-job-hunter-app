import { buildFilename, exportDOCX, exportPDF, exportTXT, resolveMarket } from '@/lib/generate';
import type { useSessionStore } from '@/store/session-store';

type AiGenerateState = ReturnType<typeof useSessionStore.getState>['aiGenerate'];

/** The slice of the AI Generate session an export reads. */
export type ExportDocumentState = Pick<
  AiGenerateState,
  | 'activeOut'
  | 'resumeOut'
  | 'coverOut'
  | 'meta'
  | 'locale'
  | 'templateId'
  | 'atsMode'
  | 'accent'
  | 'letterLayoutId'
>;

/** Export the active document (résumé or cover letter) in `fmt`. No-op on empty text. */
export async function exportActiveDocument(
  {
    activeOut,
    resumeOut,
    coverOut,
    meta,
    locale,
    templateId,
    atsMode,
    accent,
    letterLayoutId,
  }: ExportDocumentState,
  fmt: 'pdf' | 'docx' | 'txt'
): Promise<void> {
  const text = activeOut === 'resume' ? resumeOut : coverOut;
  if (!text) return;
  const type = activeOut === 'resume' ? 'resume' : 'cover-letter';
  // The cover letter's exported layout (subject line, date placement, page)
  // must match the market its text was generated for, so resolve it the same
  // way generation does (manual override → job country → language → intl).
  // Résumé export keeps the user's chosen locale unchanged.
  const exportLocale =
    type === 'cover-letter'
      ? resolveMarket({
          jobCountry: meta?.jobCountry,
          targetLanguage: meta?.targetLanguage,
          override: locale,
        })
      : locale;
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
    await exportPDF(
      text,
      name,
      type,
      meta ?? undefined,
      templateId,
      atsMode,
      exportLocale,
      accent,
      letterLayoutId
    );
  }
  if (fmt === 'docx') {
    await exportDOCX(
      text,
      name,
      type,
      meta ?? undefined,
      templateId,
      atsMode,
      exportLocale,
      accent,
      letterLayoutId
    );
  }
  if (fmt === 'txt') {
    exportTXT(text, name);
  }
}
