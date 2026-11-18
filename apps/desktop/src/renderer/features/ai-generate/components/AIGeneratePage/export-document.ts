import { type TrackResumeExport, useResumeExportPath } from '@/hooks/use-resume-export-path';
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

/** Export the active document (résumé or cover letter) in `fmt`. No-op on empty text.
 *  `track` records a résumé's saved path (see `useResumeExportPath`). */
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
  fmt: 'pdf' | 'docx' | 'txt',
  track: TrackResumeExport,
  /** Id of the saved generation this output belongs to — the backend records the export template on it. */
  generationId?: string
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
  await track(type === 'resume', async () => {
    if (fmt === 'txt') return exportTXT(text, name);
    const exporter = fmt === 'pdf' ? exportPDF : exportDOCX;
    return exporter(
      text,
      name,
      type,
      meta ?? undefined,
      templateId,
      atsMode,
      exportLocale,
      accent,
      letterLayoutId,
      generationId
    );
  });
}

/** Export action for the AI Generate page plus the saved résumé path that feeds the ATS panel. */
export function useActiveDocumentExport(
  state: ExportDocumentState,
  blocked: boolean,
  generationId?: string
) {
  const { path, track } = useResumeExportPath();
  const doExport = async (fmt: 'pdf' | 'docx' | 'txt') => {
    if (!blocked) await exportActiveDocument(state, fmt, track, generationId);
  };
  return { atsViewPath: path, doExport };
}
