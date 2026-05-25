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
  ): Promise<{ data: number[]; mimeType: string; filename: string }>;

  exportAndSave(req: BaseExportRequest): Promise<string>;
}

export const DOCUMENTS_CHANNELS = {
  list: 'documents:list',
  import: 'documents:import',
  remove: 'documents:remove',
  exportDocument: 'documents:export_document',
  exportAndSave: 'documents:export_and_save',
} as const;
