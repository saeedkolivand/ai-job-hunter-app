import type { DocumentImportRequest } from '../../schemas/index.js';
import type { DocumentRecord } from '../../types/index.js';

export type TemplateId =
  | 'classic'
  | 'modern'
  | 'executive'
  | 'editorial-serif'
  | 'swiss-minimal'
  | 'two-column'
  | 'mono-technical'
  | 'refined-executive'
  | 'academic';

interface ExportMeta {
  candidateName?: string;
  jobTitle?: string;
  companyName?: string;
  targetLanguage?: string;
}

interface BaseExportRequest {
  text: string;
  format: 'docx' | 'pdf' | 'txt';
  documentType: 'resume' | 'cover-letter';
  templateId: TemplateId;
  meta?: ExportMeta;
  /** Linearize two-column layouts for ATS parsers. Only affects two-column template. */
  atsMode?: boolean;
}

export type ExportIssueSeverity = 'critical' | 'warning';

/** A single problem found while re-reading an exported document. */
export interface ExportIssue {
  severity: ExportIssueSeverity;
  /** Stable machine code (e.g. `section_order`, `missing_section`). */
  code: string;
  /** Plain-language explanation for the user. */
  message: string;
}

/**
 * Pre-export validation report. Present for PDF/DOCX, absent for TXT. The
 * backend auto-fixes a two-column layout that doesn't survive extraction and
 * blocks the export only when a critical issue survives, so `ok` is `false`
 * only on a hard failure the user must address.
 */
export interface ExportReport {
  ok: boolean;
  /** Whether the returned bytes were rendered in ATS (single-column) mode. */
  atsMode: boolean;
  issues: ExportIssue[];
  /** Human-readable description of each auto-fix that was applied. */
  fixed: string[];
}

export interface CoverLetterExportRequest {
  templateId: TemplateId;
  /** Recipient first/last name — used for salutation. */
  recipientName?: string;
  /** Honorific: "Dr.", "Prof.", "Ms.", etc. — prepended to salutation. */
  recipientTitle?: string;
  recipientCompany?: string;
  /** Multi-line OK — rendered as recipient block. */
  recipientAddress?: string;
  /** Overrides the template's default closing phrase. */
  closingPhrase?: string;
  /** User's professional title — shown in NameAndTitle / ScriptStyle signatures. */
  signatureTitle?: string;
  /** Overrides the app locale for salutation and closing phrase resolution. */
  locale?: 'en' | 'de';
}

export interface DocumentsContract {
  list(): Promise<DocumentRecord[]>;

  import(req: DocumentImportRequest): Promise<{ id: string; success: boolean }>;

  remove(id: string): Promise<void>;

  setDefault(id: string): Promise<void>;

  exportDocument(
    req: BaseExportRequest
  ): Promise<{ data: number[]; mimeType: string; filename: string; report?: ExportReport }>;

  exportAndSave(req: BaseExportRequest): Promise<string>;
}

export const DOCUMENTS_CHANNELS = {
  list: 'documents:list',
  import: 'documents:import',
  remove: 'documents:remove',
  exportDocument: 'documents:export_document',
  exportAndSave: 'documents:export_and_save',
} as const;
