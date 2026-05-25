import type { DocumentImportRequest } from '../../schemas/index.js';
import type { DocumentRecord } from '../../types/index.js';

export interface DocumentsContract {
  list(): Promise<DocumentRecord[]>;

  import(req: DocumentImportRequest): Promise<{ id: string; success: boolean }>;

  remove(id: string): Promise<void>;

  setDefault(id: string): Promise<void>;

  exportDocument(req: {
    text: string;
    format: 'docx' | 'pdf' | 'txt';
    documentType: 'resume' | 'cover-letter';
    templateId: 'classic' | 'modern' | 'executive';
    meta?: {
      candidateName?: string;
      jobTitle?: string;
      companyName?: string;
      targetLanguage?: string;
    };
  }): Promise<{ data: number[]; mimeType: string; filename: string }>;

  exportAndSave(req: {
    text: string;
    format: 'docx' | 'pdf' | 'txt';
    documentType: 'resume' | 'cover-letter';
    templateId: 'classic' | 'modern' | 'executive';
    meta?: {
      candidateName?: string;
      jobTitle?: string;
      companyName?: string;
      targetLanguage?: string;
    };
  }): Promise<string>;
}

export const DOCUMENTS_CHANNELS = {
  list: 'documents:list',
  import: 'documents:import',
  remove: 'documents:remove',
  exportDocument: 'documents:export_document',
  exportAndSave: 'documents:export_and_save',
} as const;
